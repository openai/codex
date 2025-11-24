#![feature(test)]

extern crate test;

use test::Bencher;
use tiny_keccak::{keccakf, Keccak, Hasher};

#[bench]
fn bench_keccak_256_input_4096_bytes(b: &mut Bencher) {
    let data = [254u8; 4096];
    b.bytes = data.len() as u64;

    b.iter(|| {
        let mut res: [u8; 32] = [0; 32];
        let mut keccak = Keccak::v256();
        keccak.update(&data);
        keccak.finalize(&mut res);
    });
}

#[bench]
fn keccakf_u64(b: &mut Bencher) {
    const WORDS: usize = 25;
    b.bytes = (WORDS * 8) as u64;

    b.iter(|| {
        let mut data = [0u64; WORDS];
        keccakf(&mut data);
    });
}

#[bench]
fn bench_keccak256(b: &mut Bencher) {
    let data = [0u8; 32];
    b.bytes = data.len() as u64;

    b.iter(|| {
        let mut res: [u8; 32] = [0; 32];
        let mut keccak = Keccak::v256();
        keccak.update(&data);
        keccak.finalize(&mut res);
    });
}
