#![feature(test)]
#![cfg(feature = "rand")]

extern crate test;

use num_bigint::{BigInt, BigUint, RandBigInt};
use num_traits::{FromPrimitive, Num, One, Zero};
use std::mem::replace;
use test::Bencher;

mod rng;
use rng::get_rng;

fn multiply_bench(b: &mut Bencher, xbits: u64, ybits: u64) {
    let mut rng = get_rng();
    let x = rng.gen_bigint(xbits);
    let y = rng.gen_bigint(ybits);

    b.iter(|| &x * &y);
}

fn divide_bench(b: &mut Bencher, xbits: u64, ybits: u64) {
    let mut rng = get_rng();
    let x = rng.gen_bigint(xbits);
    let y = rng.gen_bigint(ybits);

    b.iter(|| &x / &y);
}

fn remainder_bench(b: &mut Bencher, xbits: u64, ybits: u64) {
    let mut rng = get_rng();
    let x = rng.gen_bigint(xbits);
    let y = rng.gen_bigint(ybits);

    b.iter(|| &x % &y);
}

fn factorial(n: usize) -> BigUint {
    let mut f: BigUint = One::one();
    for i in 1..=n {
        let bu: BigUint = FromPrimitive::from_usize(i).unwrap();
        f *= bu;
    }
    f
}

/// Compute Fibonacci numbers
fn fib(n: usize) -> BigUint {
    let mut f0: BigUint = Zero::zero();
    let mut f1: BigUint = One::one();
    for _ in 0..n {
        let f2 = f0 + &f1;
        f0 = replace(&mut f1, f2);
    }
    f0
}

/// Compute Fibonacci numbers with two ops per iteration
/// (add and subtract, like issue #200)
fn fib2(n: usize) -> BigUint {
    let mut f0: BigUint = Zero::zero();
    let mut f1: BigUint = One::one();
    for _ in 0..n {
        f1 += &f0;
        f0 = &f1 - f0;
    }
    f0
}

#[bench]
fn multiply_0(b: &mut Bencher) {
    multiply_bench(b, 1 << 8, 1 << 8);
}

#[bench]
fn multiply_1(b: &mut Bencher) {
    multiply_bench(b, 1 << 8, 1 << 16);
}

#[bench]
fn multiply_2(b: &mut Bencher) {
    multiply_bench(b, 1 << 16, 1 << 16);
}

#[bench]
fn multiply_3(b: &mut Bencher) {
    multiply_bench(b, 1 << 16, 1 << 17);
}

#[bench]
fn multiply_4(b: &mut Bencher) {
    multiply_bench(b, 1 << 12, 1 << 13);
}

#[bench]
fn multiply_5(b: &mut Bencher) {
    multiply_bench(b, 1 << 12, 1 << 14);
}

#[bench]
fn divide_0(b: &mut Bencher) {
    divide_bench(b, 1 << 8, 1 << 6);
}

#[bench]
fn divide_1(b: &mut Bencher) {
    divide_bench(b, 1 << 12, 1 << 8);
}

#[bench]
fn divide_2(b: &mut Bencher) {
    divide_bench(b, 1 << 16, 1 << 12);
}

#[bench]
fn divide_big_little(b: &mut Bencher) {
    divide_bench(b, 1 << 16, 1 << 4);
}

#[bench]
fn remainder_0(b: &mut Bencher) {
    remainder_bench(b, 1 << 8, 1 << 6);
}

#[bench]
fn remainder_1(b: &mut Bencher) {
    remainder_bench(b, 1 << 12, 1 << 8);
}

#[bench]
fn remainder_2(b: &mut Bencher) {
    remainder_bench(b, 1 << 16, 1 << 12);
}

#[bench]
fn remainder_big_little(b: &mut Bencher) {
    remainder_bench(b, 1 << 16, 1 << 4);
}

#[bench]
fn factorial_100(b: &mut Bencher) {
    b.iter(|| factorial(100));
}

#[bench]
fn fib_100(b: &mut Bencher) {
    b.iter(|| fib(100));
}

#[bench]
fn fib_1000(b: &mut Bencher) {
    b.iter(|| fib(1000));
}

#[bench]
fn fib_10000(b: &mut Bencher) {
    b.iter(|| fib(10000));
}

#[bench]
fn fib2_100(b: &mut Bencher) {
    b.iter(|| fib2(100));
}

#[bench]
fn fib2_1000(b: &mut Bencher) {
    b.iter(|| fib2(1000));
}

#[bench]
fn fib2_10000(b: &mut Bencher) {
    b.iter(|| fib2(10000));
}

#[bench]
fn fac_to_string(b: &mut Bencher) {
    let fac = factorial(100);
    b.iter(|| fac.to_string());
}

#[bench]
fn fib_to_string(b: &mut Bencher) {
    let fib = fib(100);
    b.iter(|| fib.to_string());
}

fn to_str_radix_bench(b: &mut Bencher, radix: u32, bits: u64) {
    let mut rng = get_rng();
    let x = rng.gen_bigint(bits);
    b.iter(|| x.to_str_radix(radix));
}

#[bench]
fn to_str_radix_02(b: &mut Bencher) {
    to_str_radix_bench(b, 2, 1009);
}

#[bench]
fn to_str_radix_08(b: &mut Bencher) {
    to_str_radix_bench(b, 8, 1009);
}

#[bench]
fn to_str_radix_10(b: &mut Bencher) {
    to_str_radix_bench(b, 10, 1009);
}

#[bench]
fn to_str_radix_10_2(b: &mut Bencher) {
    to_str_radix_bench(b, 10, 10009);
}

#[bench]
fn to_str_radix_16(b: &mut Bencher) {
    to_str_radix_bench(b, 16, 1009);
}

#[bench]
fn to_str_radix_36(b: &mut Bencher) {
    to_str_radix_bench(b, 36, 1009);
}

fn from_str_radix_bench(b: &mut Bencher, radix: u32) {
    let mut rng = get_rng();
    let x = rng.gen_bigint(1009);
    let s = x.to_str_radix(radix);
    assert_eq!(x, BigInt::from_str_radix(&s, radix).unwrap());
    b.iter(|| BigInt::from_str_radix(&s, radix));
}

#[bench]
fn from_str_radix_02(b: &mut Bencher) {
    from_str_radix_bench(b, 2);
}

#[bench]
fn from_str_radix_08(b: &mut Bencher) {
    from_str_radix_bench(b, 8);
}

#[bench]
fn from_str_radix_10(b: &mut Bencher) {
    from_str_radix_bench(b, 10);
}

#[bench]
fn from_str_radix_16(b: &mut Bencher) {
    from_str_radix_bench(b, 16);
}

#[bench]
fn from_str_radix_36(b: &mut Bencher) {
    from_str_radix_bench(b, 36);
}

fn rand_bench(b: &mut Bencher, bits: u64) {
    let mut rng = get_rng();

    b.iter(|| rng.gen_bigint(bits));
}

#[bench]
fn rand_64(b: &mut Bencher) {
    rand_bench(b, 1 << 6);
}

#[bench]
fn rand_256(b: &mut Bencher) {
    rand_bench(b, 1 << 8);
}

#[bench]
fn rand_1009(b: &mut Bencher) {
    rand_bench(b, 1009);
}

#[bench]
fn rand_2048(b: &mut Bencher) {
    rand_bench(b, 1 << 11);
}

#[bench]
fn rand_4096(b: &mut Bencher) {
    rand_bench(b, 1 << 12);
}

#[bench]
fn rand_8192(b: &mut Bencher) {
    rand_bench(b, 1 << 13);
}

#[bench]
fn rand_65536(b: &mut Bencher) {
    rand_bench(b, 1 << 16);
}

#[bench]
fn rand_131072(b: &mut Bencher) {
    rand_bench(b, 1 << 17);
}

#[bench]
fn shl(b: &mut Bencher) {
    let n = BigUint::one() << 1000u32;
    let mut m = n.clone();
    b.iter(|| {
        m.clone_from(&n);
        for i in 0..50 {
            m <<= i;
        }
    })
}

#[bench]
fn shr(b: &mut Bencher) {
    let n = BigUint::one() << 2000u32;
    let mut m = n.clone();
    b.iter(|| {
        m.clone_from(&n);
        for i in 0..50 {
            m >>= i;
        }
    })
}

#[bench]
fn hash(b: &mut Bencher) {
    use std::collections::HashSet;
    let mut rng = get_rng();
    let v: Vec<BigInt> = (1000..2000).map(|bits| rng.gen_bigint(bits)).collect();
    b.iter(|| {
        let h: HashSet<&BigInt> = v.iter().collect();
        assert_eq!(h.len(), v.len());
    });
}

#[bench]
fn pow_bench(b: &mut Bencher) {
    b.iter(|| {
        let upper = 100_u32;
        let mut i_big = BigUint::from(1u32);
        for _i in 2..=upper {
            i_big += 1u32;
            for j in 2..=upper {
                i_big.pow(j);
            }
        }
    });
}

#[bench]
fn pow_bench_bigexp(b: &mut Bencher) {
    use num_traits::Pow;

    b.iter(|| {
        let upper = 100_u32;
        let mut i_big = BigUint::from(1u32);
        for _i in 2..=upper {
            i_big += 1u32;
            let mut j_big = BigUint::from(1u32);
            for _j in 2..=upper {
                j_big += 1u32;
                Pow::pow(&i_big, &j_big);
            }
        }
    });
}

#[bench]
fn pow_bench_1e1000(b: &mut Bencher) {
    b.iter(|| BigUint::from(10u32).pow(1_000));
}

#[bench]
fn pow_bench_1e10000(b: &mut Bencher) {
    b.iter(|| BigUint::from(10u32).pow(10_000));
}

#[bench]
fn pow_bench_1e100000(b: &mut Bencher) {
    b.iter(|| BigUint::from(10u32).pow(100_000));
}

/// This modulus is the prime from the 2048-bit MODP DH group:
/// https://tools.ietf.org/html/rfc3526#section-3
const RFC3526_2048BIT_MODP_GROUP: &str = "\
                                          FFFFFFFF_FFFFFFFF_C90FDAA2_2168C234_C4C6628B_80DC1CD1\
                                          29024E08_8A67CC74_020BBEA6_3B139B22_514A0879_8E3404DD\
                                          EF9519B3_CD3A431B_302B0A6D_F25F1437_4FE1356D_6D51C245\
                                          E485B576_625E7EC6_F44C42E9_A637ED6B_0BFF5CB6_F406B7ED\
                                          EE386BFB_5A899FA5_AE9F2411_7C4B1FE6_49286651_ECE45B3D\
                                          C2007CB8_A163BF05_98DA4836_1C55D39A_69163FA8_FD24CF5F\
                                          83655D23_DCA3AD96_1C62F356_208552BB_9ED52907_7096966D\
                                          670C354E_4ABC9804_F1746C08_CA18217C_32905E46_2E36CE3B\
                                          E39E772C_180E8603_9B2783A2_EC07A28F_B5C55DF0_6F4C52C9\
                                          DE2BCBF6_95581718_3995497C_EA956AE5_15D22618_98FA0510\
                                          15728E5A_8AACAA68_FFFFFFFF_FFFFFFFF";

#[bench]
fn modpow(b: &mut Bencher) {
    let mut rng = get_rng();
    let base = rng.gen_biguint(2048);
    let e = rng.gen_biguint(2048);
    let m = BigUint::from_str_radix(RFC3526_2048BIT_MODP_GROUP, 16).unwrap();

    b.iter(|| base.modpow(&e, &m));
}

#[bench]
fn modpow_even(b: &mut Bencher) {
    let mut rng = get_rng();
    let base = rng.gen_biguint(2048);
    let e = rng.gen_biguint(2048);
    // Make the modulus even, so monty (base-2^32) doesn't apply.
    let m = BigUint::from_str_radix(RFC3526_2048BIT_MODP_GROUP, 16).unwrap() - 1u32;

    b.iter(|| base.modpow(&e, &m));
}

#[bench]
fn to_u32_digits(b: &mut Bencher) {
    let mut rng = get_rng();
    let n = rng.gen_biguint(2048);

    b.iter(|| n.to_u32_digits());
}

#[bench]
fn iter_u32_digits(b: &mut Bencher) {
    let mut rng = get_rng();
    let n = rng.gen_biguint(2048);

    b.iter(|| n.iter_u32_digits().max());
}

#[bench]
fn to_u64_digits(b: &mut Bencher) {
    let mut rng = get_rng();
    let n = rng.gen_biguint(2048);

    b.iter(|| n.to_u64_digits());
}

#[bench]
fn iter_u64_digits(b: &mut Bencher) {
    let mut rng = get_rng();
    let n = rng.gen_biguint(2048);

    b.iter(|| n.iter_u64_digits().max());
}
