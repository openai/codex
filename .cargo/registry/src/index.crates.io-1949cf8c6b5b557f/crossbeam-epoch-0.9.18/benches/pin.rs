#![feature(test)]

extern crate test;

use crossbeam_epoch as epoch;
use crossbeam_utils::thread::scope;
use test::Bencher;

#[bench]
fn single_pin(b: &mut Bencher) {
    b.iter(epoch::pin);
}

#[bench]
fn multi_pin(b: &mut Bencher) {
    const THREADS: usize = 16;
    const STEPS: usize = 100_000;

    b.iter(|| {
        scope(|s| {
            for _ in 0..THREADS {
                s.spawn(|_| {
                    for _ in 0..STEPS {
                        epoch::pin();
                    }
                });
            }
        })
        .unwrap();
    });
}
