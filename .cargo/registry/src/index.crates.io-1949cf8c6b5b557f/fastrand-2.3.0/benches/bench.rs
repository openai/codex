#![feature(test)]

extern crate test;

use rand::prelude::*;
use test::Bencher;
use wyhash::WyRng;

#[bench]
fn shuffle_wyhash(b: &mut Bencher) {
    let mut rng = WyRng::from_rng(thread_rng()).unwrap();
    let mut x = (0..100).collect::<Vec<usize>>();
    b.iter(|| {
        x.shuffle(&mut rng);
        x[0]
    })
}

#[bench]
fn shuffle_fastrand(b: &mut Bencher) {
    let mut rng = fastrand::Rng::new();
    let mut x = (0..100).collect::<Vec<usize>>();
    b.iter(|| {
        rng.shuffle(&mut x);
        x[0]
    })
}

#[bench]
fn u8_wyhash(b: &mut Bencher) {
    let mut rng = WyRng::from_rng(thread_rng()).unwrap();
    b.iter(|| {
        let mut sum = 0u8;
        for _ in 0..10_000 {
            sum = sum.wrapping_add(rng.gen::<u8>());
        }
        sum
    })
}

#[bench]
fn u8_fastrand(b: &mut Bencher) {
    let mut rng = fastrand::Rng::new();
    b.iter(|| {
        let mut sum = 0u8;
        for _ in 0..10_000 {
            sum = sum.wrapping_add(rng.u8(..));
        }
        sum
    })
}

#[bench]
fn u32_wyhash(b: &mut Bencher) {
    let mut rng = WyRng::from_rng(thread_rng()).unwrap();
    b.iter(|| {
        let mut sum = 0u32;
        for _ in 0..10_000 {
            sum = sum.wrapping_add(rng.gen::<u32>());
        }
        sum
    })
}

#[bench]
fn u32_fastrand(b: &mut Bencher) {
    let mut rng = fastrand::Rng::new();
    b.iter(|| {
        let mut sum = 0u32;
        for _ in 0..10_000 {
            sum = sum.wrapping_add(rng.u32(..));
        }
        sum
    })
}

#[bench]
fn fill(b: &mut Bencher) {
    let mut rng = fastrand::Rng::new();
    b.iter(|| {
        // Pick a size that isn't divisible by 8.
        let mut bytes = [0u8; 367];
        rng.fill(&mut bytes);
        bytes
    })
}

#[bench]
fn fill_naive(b: &mut Bencher) {
    let mut rng = fastrand::Rng::new();
    b.iter(|| {
        let mut bytes = [0u8; 367];
        for item in &mut bytes {
            *item = rng.u8(..);
        }
        bytes
    })
}
