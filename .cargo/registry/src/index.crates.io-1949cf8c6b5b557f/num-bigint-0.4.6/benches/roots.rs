#![feature(test)]
#![cfg(feature = "rand")]

extern crate test;

use num_bigint::{BigUint, RandBigInt};
use test::Bencher;

mod rng;
use rng::get_rng;

// The `big64` cases demonstrate the speed of cases where the value
// can be converted to a `u64` primitive for faster calculation.
//
// The `big1k` cases demonstrate those that can convert to `f64` for
// a better initial guess of the actual value.
//
// The `big2k` and `big4k` cases are too big for `f64`, and use a simpler guess.

fn check(x: &BigUint, n: u32) {
    let root = x.nth_root(n);
    if n == 2 {
        assert_eq!(root, x.sqrt())
    } else if n == 3 {
        assert_eq!(root, x.cbrt())
    }

    let lo = root.pow(n);
    assert!(lo <= *x);
    assert_eq!(lo.nth_root(n), root);
    assert_eq!((&lo - 1u32).nth_root(n), &root - 1u32);

    let hi = (&root + 1u32).pow(n);
    assert!(hi > *x);
    assert_eq!(hi.nth_root(n), &root + 1u32);
    assert_eq!((&hi - 1u32).nth_root(n), root);
}

fn bench_sqrt(b: &mut Bencher, bits: u64) {
    let x = get_rng().gen_biguint(bits);
    eprintln!("bench_sqrt({})", x);

    check(&x, 2);
    b.iter(|| x.sqrt());
}

#[bench]
fn big64_sqrt(b: &mut Bencher) {
    bench_sqrt(b, 64);
}

#[bench]
fn big1k_sqrt(b: &mut Bencher) {
    bench_sqrt(b, 1024);
}

#[bench]
fn big2k_sqrt(b: &mut Bencher) {
    bench_sqrt(b, 2048);
}

#[bench]
fn big4k_sqrt(b: &mut Bencher) {
    bench_sqrt(b, 4096);
}

fn bench_cbrt(b: &mut Bencher, bits: u64) {
    let x = get_rng().gen_biguint(bits);
    eprintln!("bench_cbrt({})", x);

    check(&x, 3);
    b.iter(|| x.cbrt());
}

#[bench]
fn big64_cbrt(b: &mut Bencher) {
    bench_cbrt(b, 64);
}

#[bench]
fn big1k_cbrt(b: &mut Bencher) {
    bench_cbrt(b, 1024);
}

#[bench]
fn big2k_cbrt(b: &mut Bencher) {
    bench_cbrt(b, 2048);
}

#[bench]
fn big4k_cbrt(b: &mut Bencher) {
    bench_cbrt(b, 4096);
}

fn bench_nth_root(b: &mut Bencher, bits: u64, n: u32) {
    let x = get_rng().gen_biguint(bits);
    eprintln!("bench_{}th_root({})", n, x);

    check(&x, n);
    b.iter(|| x.nth_root(n));
}

#[bench]
fn big64_nth_10(b: &mut Bencher) {
    bench_nth_root(b, 64, 10);
}

#[bench]
fn big1k_nth_10(b: &mut Bencher) {
    bench_nth_root(b, 1024, 10);
}

#[bench]
fn big1k_nth_100(b: &mut Bencher) {
    bench_nth_root(b, 1024, 100);
}

#[bench]
fn big1k_nth_1000(b: &mut Bencher) {
    bench_nth_root(b, 1024, 1000);
}

#[bench]
fn big1k_nth_10000(b: &mut Bencher) {
    bench_nth_root(b, 1024, 10000);
}

#[bench]
fn big2k_nth_10(b: &mut Bencher) {
    bench_nth_root(b, 2048, 10);
}

#[bench]
fn big2k_nth_100(b: &mut Bencher) {
    bench_nth_root(b, 2048, 100);
}

#[bench]
fn big2k_nth_1000(b: &mut Bencher) {
    bench_nth_root(b, 2048, 1000);
}

#[bench]
fn big2k_nth_10000(b: &mut Bencher) {
    bench_nth_root(b, 2048, 10000);
}

#[bench]
fn big4k_nth_10(b: &mut Bencher) {
    bench_nth_root(b, 4096, 10);
}

#[bench]
fn big4k_nth_100(b: &mut Bencher) {
    bench_nth_root(b, 4096, 100);
}

#[bench]
fn big4k_nth_1000(b: &mut Bencher) {
    bench_nth_root(b, 4096, 1000);
}

#[bench]
fn big4k_nth_10000(b: &mut Bencher) {
    bench_nth_root(b, 4096, 10000);
}
