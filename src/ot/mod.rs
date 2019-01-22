mod chou_orlandi;
mod dummy;
mod iknp;
mod naor_pinkas;

pub use chou_orlandi::ChouOrlandiOT;
pub use dummy::DummyOT;
pub use iknp::IknpOT;
pub use naor_pinkas::NaorPinkasOT;

use crate::util::*;
use aes::Aes128;
use bitvec::BitVec;
use block_modes::block_padding::Pkcs7;
use block_modes::{BlockMode, Ecb};
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
use failure::Error;
use std::io::Error as IOError;
use std::io::{ErrorKind, Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};

pub trait ObliviousTransfer<T: Read + Write> {
    fn new(stream: Arc<Mutex<T>>) -> Self;
    fn send(&mut self, values: &[(BitVec, BitVec)]) -> Result<(), Error>;
    fn receive(&mut self, inputs: &[u16], nbits: usize) -> Result<Vec<BitVec>, Error>;
}

struct Stream<T: Read + Write> {
    stream: Arc<Mutex<T>>,
}

impl<T: Read + Write> Stream<T> {
    pub fn new(stream: Arc<Mutex<T>>) -> Self {
        Self { stream }
    }
    #[inline(always)]
    fn stream(&mut self) -> MutexGuard<T> {
        self.stream.lock().unwrap()
    }
    #[inline(always)]
    fn write_pt(&mut self, pt: &RistrettoPoint) -> Result<usize, Error> {
        self.stream()
            .write(pt.compress().as_bytes())
            .map_err(Error::from)
    }
    #[inline(always)]
    fn read_pt(&mut self) -> Result<RistrettoPoint, Error> {
        let mut data = [0; 32];
        self.stream().read_exact(&mut data)?;
        let pt = match CompressedRistretto::from_slice(&data).decompress() {
            Some(pt) => pt,
            None => {
                return Err(Error::from(IOError::new(
                    ErrorKind::InvalidData,
                    "Unable to decompress point",
                )));
            }
        };
        Ok(pt)
    }
    #[inline(always)]
    fn write_bool(&mut self, b: bool) -> Result<usize, Error> {
        self.stream().write(&[b as u8]).map_err(Error::from)
    }
    #[inline(always)]
    fn read_bool(&mut self) -> Result<bool, Error> {
        let mut data = [0; 1];
        self.stream().read_exact(&mut data)?;
        Ok(if data[0] == 0 { false } else { true })
    }
    #[inline(always)]
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        self.stream().write(bytes).map_err(Error::from)
    }
    #[inline(always)]
    fn read_bytes(&mut self, nbytes: usize) -> Result<Vec<u8>, Error> {
        let mut bytes = vec![0; nbytes];
        self.stream().read_exact(&mut bytes)?;
        Ok(bytes)
    }
    #[inline(always)]
    fn write_bitvec(&mut self, bytes: &BitVec) -> Result<usize, Error> {
        self.stream()
            .write(&(bitvec_to_vec(bytes)))
            .map_err(Error::from)
    }
    #[inline(always)]
    fn read_bitvec(&mut self, nbits: usize) -> Result<BitVec, Error> {
        let mut bytes = vec![0; nbits / 8];
        self.stream().read_exact(&mut bytes)?;
        Ok(BitVec::from(bytes))
    }
    #[inline(always)]
    fn write_u128(&mut self, data: &u128) -> Result<usize, Error> {
        self.stream()
            .write(&data.to_ne_bytes())
            .map_err(Error::from)
    }
    #[inline(always)]
    fn read_u128(&mut self) -> Result<u128, Error> {
        let mut data = [0; 16];
        self.stream().read_exact(&mut data)?;
        Ok(u128::from_ne_bytes(data))
    }
}

#[inline(always)]
fn hash_pt(pt: &RistrettoPoint, length: usize) -> Vec<u8> {
    // Hash a point `pt` by compute `E(pt, 0)`
    let result = pt.compress();
    let bytes = result.as_bytes();
    let m = vec![0; length];
    encrypt(&bytes[0..16], &m).unwrap().drain(16..).collect()
}
#[inline(always)]
fn xor(a: &[u8], b: &[u8]) -> Vec<u8> {
    assert_eq!(
        a.len(),
        b.len(),
        "lengths not equal: {} ≠ {}",
        a.len(),
        b.len()
    );
    a.iter().zip(b.iter()).map(|(a, b)| a ^ b).collect()
}

type Cipher = Ecb<Aes128, Pkcs7>;

#[inline(always)]
fn encrypt(k: &[u8], m: &[u8]) -> Result<Vec<u8>, Error> {
    let cipher = match Cipher::new_var(k, Default::default()) {
        Ok(c) => c,
        Err(_) => {
            return Err(Error::from(IOError::new(
                ErrorKind::InvalidInput,
                "initializing AES128-ECB failed",
            )));
        }
    };
    let mut m = m.to_vec();
    let ct = cipher.encrypt_vec(&mut m);
    Ok(ct.to_vec())
}
#[inline(always)]
fn decrypt(k: &[u8], c: &[u8]) -> Result<Vec<u8>, Error> {
    let cipher = match Cipher::new_var(k, Default::default()) {
        Ok(c) => c,
        Err(_) => {
            return Err(Error::from(IOError::new(
                ErrorKind::InvalidInput,
                "initializing AES128-ECB failed",
            )));
        }
    };
    let mut c = c.to_vec();
    let m = match cipher.decrypt_vec(&mut c) {
        Ok(m) => m,
        Err(_) => {
            return Err(Error::from(IOError::new(
                ErrorKind::InvalidInput,
                "decrypting AES128-ECB failed",
            )));
        }
    };
    Ok(m.to_vec())
}