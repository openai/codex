#![feature(test)]

extern crate test;

use crossbeam_epoch::{self as epoch, Owned};
use crossbeam_utils::thread::scope;
use test::Bencher;

#[bench]
fn single_alloc_defer_free(b: &mut Bencher) {
    b.iter(|| {
        let guard = &epoch::pin();
        let p = Owned::new(1).into_shared(guard);
        unsafe {
            guard.defer_destroy(p);
        }
    });
}

#[bench]
fn single_defer(b: &mut Bencher) {
    b.iter(|| {
        let guard = &epoch::pin();
        guard.defer(move || ());
    });
}

#[bench]
fn multi_alloc_defer_free(b: &mut Bencher) {
    const THREADS: usize = 16;
    const STEPS: usize = 10_000;

    b.iter(|| {
        scope(|s| {
            for _ in 0..THREADS {
                s.spawn(|_| {
                    for _ in 0..STEPS {
                        let guard = &epoch::pin();
                        let p = Owned::new(1).into_shared(guard);
                        unsafe {
                            guard.defer_destroy(p);
                        }
                    }
                });
            }
        })
        .unwrap();
    });
}

#[bench]
fn multi_defer(b: &mut Bencher) {
    const THREADS: usize = 16;
    const STEPS: usize = 10_000;

    b.iter(|| {
        scope(|s| {
            for _ in 0..THREADS {
                s.spawn(|_| {
                    for _ in 0..STEPS {
                        let guard = &epoch::pin();
                        guard.defer(move || ());
                    }
                });
            }
        })
        .unwrap();
    });
}
