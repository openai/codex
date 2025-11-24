#![feature(test)]

extern crate test;
extern crate fixedbitset;
use test::Bencher;
use fixedbitset::{FixedBitSet};
use std::mem::size_of;

#[inline]
fn iter_ones_using_contains<F: FnMut(usize)>(fb: &FixedBitSet, f: &mut F) {
    for bit in 0 .. fb.len() {
       if fb.contains(bit) {
           f(bit);
       }
    }
}

#[inline]
fn iter_ones_using_slice_directly<F: FnMut(usize)>(fb: &FixedBitSet, f: &mut F) {
    for (block_idx, &block) in fb.as_slice().iter().enumerate() {
        let mut bit_pos = block_idx * size_of::<u32>() * 8;
        let mut block: u32 = block;

        while block != 0 {
            if (block & 1) == 1 {
                f(bit_pos);
            }
            block = block >> 1;
            bit_pos += 1;
        }
    }
}

#[bench]
fn bench_iter_ones_using_contains_all_zeros(b: &mut Bencher) {
    const N: usize = 1_000_000;
    let fb = FixedBitSet::with_capacity(N);

    b.iter(|| {
        let mut count = 0;
        iter_ones_using_contains(&fb, &mut |_bit| count += 1);
        count
    });
}

#[bench]
fn bench_iter_ones_using_contains_all_ones(b: &mut Bencher) {
    const N: usize = 1_000_000;
    let mut fb = FixedBitSet::with_capacity(N);
    fb.insert_range(..);

    b.iter(|| {
        let mut count = 0;
        iter_ones_using_contains(&fb, &mut |_bit| count += 1);
        count
    });
}

#[bench]
fn bench_iter_ones_using_slice_directly_all_zero(b: &mut Bencher) {
    const N: usize = 1_000_000;
    let fb = FixedBitSet::with_capacity(N);

    b.iter(|| {
       let mut count = 0;
       iter_ones_using_slice_directly(&fb, &mut |_bit| count += 1);
       count
    });
}

#[bench]
fn bench_iter_ones_using_slice_directly_all_ones(b: &mut Bencher) {
    const N: usize = 1_000_000;
    let mut fb = FixedBitSet::with_capacity(N);
    fb.insert_range(..);

    b.iter(|| {
       let mut count = 0;
       iter_ones_using_slice_directly(&fb, &mut |_bit| count += 1);
       count
    });
}

#[bench]
fn bench_iter_ones_all_zeros(b: &mut Bencher) {
    const N: usize = 1_000_000;
    let fb = FixedBitSet::with_capacity(N);

    b.iter(|| {
        let mut count = 0;
        for _ in fb.ones() {
            count += 1;
        }
        count
    });
}

#[bench]
fn bench_iter_ones_all_ones(b: &mut Bencher) {
    const N: usize = 1_000_000;
    let mut fb = FixedBitSet::with_capacity(N);
    fb.insert_range(..);

    b.iter(|| {
        let mut count = 0;
        for _ in fb.ones() {
            count += 1;
        }
        count
    });
}

#[bench]
fn bench_insert_range(b: &mut Bencher) {
    const N: usize = 1_000_000;
    let mut fb = FixedBitSet::with_capacity(N);

    b.iter(|| {
        fb.insert_range(..)
    });
}

#[bench]
fn bench_insert_range_using_loop(b: &mut Bencher) {
    const N: usize = 1_000_000;
    let mut fb = FixedBitSet::with_capacity(N);

    b.iter(|| {
        for i in 0..N {
            fb.insert(i);
        }
    });
}
