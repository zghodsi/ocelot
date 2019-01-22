use super::{ObliviousTransfer, Stream};
use crate::util;
use bitvec::BitVec;
use curve25519_dalek::constants;
use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::scalar::Scalar;
use failure::Error;
use rand::rngs::ThreadRng;
use std::io::{ErrorKind, Read, Write};
use std::sync::{Arc, Mutex};

pub struct ChouOrlandiOT<T: Read + Write> {
    stream: Stream<T>,
    rng: ThreadRng,
}

const P: RistrettoPoint = constants::RISTRETTO_BASEPOINT_POINT;
const KEYSIZE: usize = 16;

impl<T: Read + Write> ChouOrlandiOT<T> {
    fn _send(&mut self, values: &(BitVec, BitVec)) -> Result<(), Error> {
        assert_eq!(
            values.0.len(),
            values.1.len(),
            "input values of unequal
        length: {} ≠ {}",
            values.0.len(),
            values.1.len()
        );
        let a = Scalar::random(&mut self.rng);
        let a_ = P * a;
        self.stream.write_pt(&a_)?;
        let b_ = self.stream.read_pt()?;
        let k0 = super::hash_pt(&(b_ * a), KEYSIZE);
        let k1 = super::hash_pt(&((b_ - a_) * a), KEYSIZE);
        let c0 = super::encrypt(&k0, &util::bitvec_to_vec(&values.0))?;
        let c1 = super::encrypt(&k1, &util::bitvec_to_vec(&values.1))?;
        self.stream.write_bytes(&c0)?;
        self.stream.write_bytes(&c1)?;
        Ok(())
    }

    fn _receive(&mut self, input: u16, nbits: usize) -> Result<BitVec, Error> {
        let input = match input {
            0 => false,
            1 => true,
            _ => {
                return Err(Error::from(std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "Input must be zero or one",
                )));
            }
        };
        let nbytes = nbits / 8;
        let b = Scalar::random(&mut self.rng);
        let a_ = self.stream.read_pt()?;
        let b_ = match input {
            // XXX: Timing attack!
            false => P * b,
            true => a_ + P * b,
        };
        self.stream.write_pt(&b_)?;
        let kr = super::hash_pt(&(a_ * b), KEYSIZE);
        let c_0 = self.stream.read_bytes(nbytes + KEYSIZE)?;
        let c_1 = self.stream.read_bytes(nbytes + KEYSIZE)?;
        let c = if input { &c_1 } else { &c_0 };
        let m = super::decrypt(&kr, &c)?;
        Ok(BitVec::from(m))
    }
}

impl<T: Read + Write> ObliviousTransfer<T> for ChouOrlandiOT<T> {
    fn new(stream: Arc<Mutex<T>>) -> Self {
        let stream = Stream::new(stream);
        let rng = rand::thread_rng();
        Self { stream, rng }
    }

    fn send(&mut self, values: &[(BitVec, BitVec)]) -> Result<(), Error> {
        for inputs in values.iter() {
            self._send(inputs)?;
        }
        Ok(())
    }

    fn receive(&mut self, inputs: &[u16], nbits: usize) -> Result<Vec<BitVec>, Error> {
        let mut outputs = Vec::with_capacity(inputs.len());
        for input in inputs.iter() {
            let output = self._receive(*input, nbits)?;
            outputs.push(output);
        }
        Ok(outputs)
    }
}

#[cfg(test)]
mod tests {
    extern crate test;
    use super::*;
    use std::os::unix::net::UnixStream;
    use test::Bencher;

    const N: usize = 32;

    fn rand_u8_vec(size: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(size);
        for _ in 0..size {
            v.push(rand::random::<u8>());
        }
        v
    }

    #[test]
    fn test() {
        let m0 = rand_u8_vec(N);
        let m1 = rand_u8_vec(N);
        let m0 = BitVec::from(m0.to_vec());
        let m1 = BitVec::from(m1.to_vec());
        let m0_ = m0.clone();
        let m1_ = m1.clone();
        let b = rand::random::<bool>();
        let (sender, receiver) = match UnixStream::pair() {
            Ok((s1, s2)) => (Arc::new(Mutex::new(s1)), Arc::new(Mutex::new(s2))),
            Err(e) => {
                eprintln!("Couldn't create pair of sockets: {:?}", e);
                return;
            }
        };
        let handler = std::thread::spawn(move || {
            let mut ot = ChouOrlandiOT::new(sender);
            ot.send(&[(m0, m1)]).unwrap();
        });
        let mut ot = ChouOrlandiOT::new(receiver);
        let results = ot.receive(&[b as u16], N * 8).unwrap();
        assert_eq!(results[0], if b { m1_ } else { m0_ });
        handler.join().unwrap();
    }

    #[bench]
    fn bench(b: &mut Bencher) {
        b.iter(|| test())
    }
}