// -*- mode: rust; -*-
//
// This file is part of ocelot.
// Copyright © 2019 Galois, Inc.
// See LICENSE for licensing information.

use ocelot::oprf::{KkrtReceiver, KkrtSender};
use ocelot::oprf::{Receiver, Sender};
use scuttlebutt::comm::{TrackReader, TrackWriter};
use scuttlebutt::{AesRng, Block};
use std::io::{BufReader, BufWriter};
use std::os::unix::net::UnixStream;
use std::time::SystemTime;

fn rand_block_vec(size: usize) -> Vec<Block> {
    (0..size).map(|_| rand::random::<Block>()).collect()
}

fn _test_oprf(n: usize) {
    let selections = rand_block_vec(n);
    let (sender, receiver) = UnixStream::pair().unwrap();
    let total = SystemTime::now();
    let handle = std::thread::spawn(move || {
        let mut rng = AesRng::new();
        let mut reader = TrackReader::new(BufReader::new(sender.try_clone().unwrap()));
        let mut writer = TrackWriter::new(BufWriter::new(sender));
        let start = SystemTime::now();
        let mut oprf = KkrtSender::init(&mut reader, &mut writer, &mut rng).unwrap();
        println!(
            "Sender init time: {} ms",
            start.elapsed().unwrap().as_millis()
        );
        let start = SystemTime::now();
        let _ = oprf.send(&mut reader, &mut writer, n, &mut rng).unwrap();
        println!(
            "[{}] Send time: {} ms",
            n,
            start.elapsed().unwrap().as_millis()
        );
        println!(
            "Sender communication (read): {:.2} Mb",
            reader.kilobits() / 1000.0
        );
        println!(
            "Sender communication (write): {:.2} Mb",
            writer.kilobits() / 1000.0
        );
    });
    let mut rng = AesRng::new();
    let mut reader = TrackReader::new(BufReader::new(receiver.try_clone().unwrap()));
    let mut writer = TrackWriter::new(BufWriter::new(receiver));
    let start = SystemTime::now();
    let mut oprf = KkrtReceiver::init(&mut reader, &mut writer, &mut rng).unwrap();
    println!(
        "Receiver init time: {} ms",
        start.elapsed().unwrap().as_millis()
    );
    let start = SystemTime::now();
    let _ = oprf
        .receive(&mut reader, &mut writer, &selections, &mut rng)
        .unwrap();
    println!(
        "[{}] Receiver time: {} ms",
        n,
        start.elapsed().unwrap().as_millis()
    );
    handle.join().unwrap();
    println!(
        "Receiver communication (read): {:.2} Mb",
        reader.kilobits() / 1000.0
    );
    println!(
        "Receiver communication (write): {:.2} Mb",
        writer.kilobits() / 1000.0
    );
    println!("Total time: {} ms", total.elapsed().unwrap().as_millis());
}

fn main() {
    _test_oprf(1 << 20);
}