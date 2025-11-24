// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(test)]

extern crate test;
extern crate rand;
extern crate rand_xorshift;
extern crate bit_vec;

use test::{Bencher, black_box};
use rand::{Rng, RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;
use bit_vec::BitVec;

const HUGE_BENCH_BITS : usize = 1 << 20;
const BENCH_BITS : usize = 1 << 14;
const U32_BITS: usize = 32;

fn small_rng() -> XorShiftRng {
    XorShiftRng::from_entropy()
}

#[bench]
fn bench_usize_small(b: &mut Bencher) {
    let mut r = small_rng();
    let mut bit_vec = 0 as usize;
    b.iter(|| {
        for _ in 0..100 {
            bit_vec |= 1 << ((r.next_u32() as usize) % U32_BITS);
        }
        black_box(&bit_vec);
    });
}

#[bench]
fn bench_bit_set_big_fixed(b: &mut Bencher) {
    let mut r = small_rng();
    let mut bit_vec = BitVec::from_elem(BENCH_BITS, false);
    b.iter(|| {
        for _ in 0..100 {
            bit_vec.set((r.next_u32() as usize) % BENCH_BITS, true);
        }
        black_box(&bit_vec);
    });
}

#[bench]
fn bench_bit_set_big_variable(b: &mut Bencher) {
    let mut r = small_rng();
    let mut bit_vec = BitVec::from_elem(BENCH_BITS, false);
    b.iter(|| {
        for _ in 0..100 {
            bit_vec.set((r.next_u32() as usize) % BENCH_BITS, r.gen());
        }
        black_box(&bit_vec);
    });
}

#[bench]
fn bench_bit_set_small(b: &mut Bencher) {
    let mut r = small_rng();
    let mut bit_vec = BitVec::from_elem(U32_BITS, false);
    b.iter(|| {
        for _ in 0..100 {
            bit_vec.set((r.next_u32() as usize) % U32_BITS, true);
        }
        black_box(&bit_vec);
    });
}

#[bench]
fn bench_bit_vec_big_or(b: &mut Bencher) {
    let mut b1 = BitVec::from_elem(BENCH_BITS, false);
    let b2 = BitVec::from_elem(BENCH_BITS, false);
    b.iter(|| {
        b1.or(&b2)
    })
}

#[bench]
fn bench_bit_vec_big_xnor(b: &mut Bencher) {
    let mut b1 = BitVec::from_elem(BENCH_BITS, false);
    let b2 = BitVec::from_elem(BENCH_BITS, false);
    b.iter(|| {
        b1.xnor(&b2)
    })
}

#[bench]
fn bench_bit_vec_big_negate_xor(b: &mut Bencher) {
    let mut b1 = BitVec::from_elem(BENCH_BITS, false);
    let b2 = BitVec::from_elem(BENCH_BITS, false);
    b.iter(|| {
        let res = b1.xor(&b2);
        b1.negate();
        res
    })
}

#[bench]
fn bench_bit_vec_huge_xnor(b: &mut Bencher) {
    let mut b1 = BitVec::from_elem(HUGE_BENCH_BITS, false);
    let b2 = BitVec::from_elem(HUGE_BENCH_BITS, false);
    b.iter(|| {
        b1.xnor(&b2)
    })
}

#[bench]
fn bench_bit_vec_huge_negate_xor(b: &mut Bencher) {
    let mut b1 = BitVec::from_elem(HUGE_BENCH_BITS, false);
    let b2 = BitVec::from_elem(HUGE_BENCH_BITS, false);
    b.iter(|| {
        let res = b1.xor(&b2);
        b1.negate();
        res
    })
}

#[bench]
fn bench_bit_vec_small_iter(b: &mut Bencher) {
    let bit_vec = BitVec::from_elem(U32_BITS, false);
    b.iter(|| {
        let mut sum = 0;
        for _ in 0..10 {
            for pres in &bit_vec {
                sum += pres as usize;
            }
        }
        sum
    })
}

#[bench]
fn bench_bit_vec_big_iter(b: &mut Bencher) {
    let bit_vec = BitVec::from_elem(BENCH_BITS, false);
    b.iter(|| {
        let mut sum = 0;
        for pres in &bit_vec {
            sum += pres as usize;
        }
        sum
    })
}

#[bench]
fn bench_from_elem(b: &mut Bencher) {
    let cap = black_box(BENCH_BITS);
    let bit = black_box(true);
    b.iter(|| {
        // create a BitVec and popcount it
        BitVec::from_elem(cap, bit).blocks()
            .fold(0, |acc, b| acc + b.count_ones())
    });
    b.bytes = cap as u64 / 8;
}

#[bench]
fn bench_erathostenes(b: &mut test::Bencher) {
    let mut primes = vec![];
    b.iter(|| {
        primes.clear();
        let mut sieve = BitVec::from_elem(1 << 16, true);
        black_box(&mut sieve);
        let mut i = 2;
        while i < sieve.len() {
            if sieve[i] == true {
                primes.push(i);
            }
            let mut j = i;
            while j < sieve.len() {
                sieve.set(j, false);
                j += i;
            }
            i += 1;
        }
        black_box(&mut sieve);
    });
}

#[bench]
fn bench_erathostenes_set_all(b: &mut test::Bencher) {
    let mut primes = vec![];
    let mut sieve = BitVec::from_elem(1 << 16, true);
    b.iter(|| {
        primes.clear();
        black_box(&mut sieve);
        sieve.set_all();
        black_box(&mut sieve);
        let mut i = 2;
        while i < sieve.len() {
            if sieve[i] == true {
                primes.push(i);
            }
            let mut j = i;
            while j < sieve.len() {
                sieve.set(j, false);
                j += i;
            }
            i += 1;
        }
        black_box(&mut sieve);
    });
}
