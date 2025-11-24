//! This file contains benchmarks for the ops traits implemented by HashSet.
//! Each test is intended to have a defined larger and smaller set,
//! but using a larger size for the "small" set works just as well.
//!
//! Each assigning test is done in the configuration that is faster. Cheating, I know.
//! The exception to this is Sub, because there the result differs. So I made two benchmarks for Sub.

#![feature(test)]

extern crate test;

use hashbrown::HashSet;
use test::Bencher;

/// The number of items to generate for the larger of the sets.
const LARGE_SET_SIZE: usize = 1000;

/// The number of items to generate for the smaller of the sets.
const SMALL_SET_SIZE: usize = 100;

/// The number of keys present in both sets.
const OVERLAP: usize =
    [LARGE_SET_SIZE, SMALL_SET_SIZE][(LARGE_SET_SIZE < SMALL_SET_SIZE) as usize] / 2;

/// Creates a set containing end - start unique string elements.
fn create_set(start: usize, end: usize) -> HashSet<String> {
    (start..end).map(|nr| format!("key{}", nr)).collect()
}

#[bench]
fn set_ops_bit_or(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| &large_set | &small_set)
}

#[bench]
fn set_ops_bit_and(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| &large_set & &small_set)
}

#[bench]
fn set_ops_bit_xor(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| &large_set ^ &small_set)
}

#[bench]
fn set_ops_sub_large_small(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| &large_set - &small_set)
}

#[bench]
fn set_ops_sub_small_large(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| &small_set - &large_set)
}

#[bench]
fn set_ops_bit_or_assign(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| {
        let mut set = large_set.clone();
        set |= &small_set;
        set
    });
}

#[bench]
fn set_ops_bit_and_assign(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| {
        let mut set = small_set.clone();
        set &= &large_set;
        set
    });
}

#[bench]
fn set_ops_bit_xor_assign(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| {
        let mut set = large_set.clone();
        set ^= &small_set;
        set
    });
}

#[bench]
fn set_ops_sub_assign_large_small(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| {
        let mut set = large_set.clone();
        set -= &small_set;
        set
    });
}

#[bench]
fn set_ops_sub_assign_small_large(b: &mut Bencher) {
    let large_set = create_set(0, LARGE_SET_SIZE);
    let small_set = create_set(
        LARGE_SET_SIZE - OVERLAP,
        LARGE_SET_SIZE + SMALL_SET_SIZE - OVERLAP,
    );
    b.iter(|| {
        let mut set = small_set.clone();
        set -= &large_set;
        set
    });
}
