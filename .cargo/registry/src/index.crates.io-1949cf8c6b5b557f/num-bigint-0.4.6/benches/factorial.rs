#![feature(test)]

extern crate test;

use num_bigint::BigUint;
use num_traits::One;
use std::ops::{Div, Mul};
use test::Bencher;

#[bench]
fn factorial_mul_biguint(b: &mut Bencher) {
    b.iter(|| {
        (1u32..1000)
            .map(BigUint::from)
            .fold(BigUint::one(), Mul::mul)
    });
}

#[bench]
fn factorial_mul_u32(b: &mut Bencher) {
    b.iter(|| (1u32..1000).fold(BigUint::one(), Mul::mul));
}

// The division test is inspired by this blog comparison:
// <https://tiehuis.github.io/big-integers-in-zig#division-test-single-limb>

#[bench]
fn factorial_div_biguint(b: &mut Bencher) {
    let n: BigUint = (1u32..1000).fold(BigUint::one(), Mul::mul);
    b.iter(|| {
        (1u32..1000)
            .rev()
            .map(BigUint::from)
            .fold(n.clone(), Div::div)
    });
}

#[bench]
fn factorial_div_u32(b: &mut Bencher) {
    let n: BigUint = (1u32..1000).fold(BigUint::one(), Mul::mul);
    b.iter(|| (1u32..1000).rev().fold(n.clone(), Div::div));
}
