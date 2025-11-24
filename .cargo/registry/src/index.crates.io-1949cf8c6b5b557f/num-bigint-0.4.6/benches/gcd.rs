#![feature(test)]
#![cfg(feature = "rand")]

extern crate test;

use num_bigint::{BigUint, RandBigInt};
use num_integer::Integer;
use num_traits::Zero;
use test::Bencher;

mod rng;
use rng::get_rng;

fn bench(b: &mut Bencher, bits: u64, gcd: fn(&BigUint, &BigUint) -> BigUint) {
    let mut rng = get_rng();
    let x = rng.gen_biguint(bits);
    let y = rng.gen_biguint(bits);

    assert_eq!(euclid(&x, &y), x.gcd(&y));

    b.iter(|| gcd(&x, &y));
}

fn euclid(x: &BigUint, y: &BigUint) -> BigUint {
    // Use Euclid's algorithm
    let mut m = x.clone();
    let mut n = y.clone();
    while !m.is_zero() {
        let temp = m;
        m = n % &temp;
        n = temp;
    }
    n
}

#[bench]
fn gcd_euclid_0064(b: &mut Bencher) {
    bench(b, 64, euclid);
}

#[bench]
fn gcd_euclid_0256(b: &mut Bencher) {
    bench(b, 256, euclid);
}

#[bench]
fn gcd_euclid_1024(b: &mut Bencher) {
    bench(b, 1024, euclid);
}

#[bench]
fn gcd_euclid_4096(b: &mut Bencher) {
    bench(b, 4096, euclid);
}

// Integer for BigUint now uses Stein for gcd

#[bench]
fn gcd_stein_0064(b: &mut Bencher) {
    bench(b, 64, BigUint::gcd);
}

#[bench]
fn gcd_stein_0256(b: &mut Bencher) {
    bench(b, 256, BigUint::gcd);
}

#[bench]
fn gcd_stein_1024(b: &mut Bencher) {
    bench(b, 1024, BigUint::gcd);
}

#[bench]
fn gcd_stein_4096(b: &mut Bencher) {
    bench(b, 4096, BigUint::gcd);
}
