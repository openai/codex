use alloc::vec::Vec;
use core::mem;
use core::ops::Shl;
use num_traits::One;

use crate::big_digit::{self, BigDigit, DoubleBigDigit};
use crate::biguint::BigUint;

struct MontyReducer {
    n0inv: BigDigit,
}

// k0 = -m**-1 mod 2**BITS. Algorithm from: Dumas, J.G. "On Newton–Raphson
// Iteration for Multiplicative Inverses Modulo Prime Powers".
fn inv_mod_alt(b: BigDigit) -> BigDigit {
    assert_ne!(b & 1, 0);

    let mut k0 = BigDigit::wrapping_sub(2, b);
    let mut t = b - 1;
    let mut i = 1;
    while i < big_digit::BITS {
        t = t.wrapping_mul(t);
        k0 = k0.wrapping_mul(t + 1);

        i <<= 1;
    }
    debug_assert_eq!(k0.wrapping_mul(b), 1);
    k0.wrapping_neg()
}

impl MontyReducer {
    fn new(n: &BigUint) -> Self {
        let n0inv = inv_mod_alt(n.data[0]);
        MontyReducer { n0inv }
    }
}

/// Computes z mod m = x * y * 2 ** (-n*_W) mod m
/// assuming k = -1/m mod 2**_W
/// See Gueron, "Efficient Software Implementations of Modular Exponentiation".
/// <https://eprint.iacr.org/2011/239.pdf>
/// In the terminology of that paper, this is an "Almost Montgomery Multiplication":
/// x and y are required to satisfy 0 <= z < 2**(n*_W) and then the result
/// z is guaranteed to satisfy 0 <= z < 2**(n*_W), but it may not be < m.
#[allow(clippy::many_single_char_names)]
fn montgomery(x: &BigUint, y: &BigUint, m: &BigUint, k: BigDigit, n: usize) -> BigUint {
    // This code assumes x, y, m are all the same length, n.
    // (required by addMulVVW and the for loop).
    // It also assumes that x, y are already reduced mod m,
    // or else the result will not be properly reduced.
    assert!(
        x.data.len() == n && y.data.len() == n && m.data.len() == n,
        "{:?} {:?} {:?} {}",
        x,
        y,
        m,
        n
    );

    let mut z = BigUint::ZERO;
    z.data.resize(n * 2, 0);

    let mut c: BigDigit = 0;
    for i in 0..n {
        let c2 = add_mul_vvw(&mut z.data[i..n + i], &x.data, y.data[i]);
        let t = z.data[i].wrapping_mul(k);
        let c3 = add_mul_vvw(&mut z.data[i..n + i], &m.data, t);
        let cx = c.wrapping_add(c2);
        let cy = cx.wrapping_add(c3);
        z.data[n + i] = cy;
        if cx < c2 || cy < c3 {
            c = 1;
        } else {
            c = 0;
        }
    }

    if c == 0 {
        z.data = z.data[n..].to_vec();
    } else {
        {
            let (first, second) = z.data.split_at_mut(n);
            sub_vv(first, second, &m.data);
        }
        z.data = z.data[..n].to_vec();
    }

    z
}

#[inline(always)]
fn add_mul_vvw(z: &mut [BigDigit], x: &[BigDigit], y: BigDigit) -> BigDigit {
    let mut c = 0;
    for (zi, xi) in z.iter_mut().zip(x.iter()) {
        let (z1, z0) = mul_add_www(*xi, y, *zi);
        let (c_, zi_) = add_ww(z0, c, 0);
        *zi = zi_;
        c = c_ + z1;
    }

    c
}

/// The resulting carry c is either 0 or 1.
#[inline(always)]
fn sub_vv(z: &mut [BigDigit], x: &[BigDigit], y: &[BigDigit]) -> BigDigit {
    let mut c = 0;
    for (i, (xi, yi)) in x.iter().zip(y.iter()).enumerate().take(z.len()) {
        let zi = xi.wrapping_sub(*yi).wrapping_sub(c);
        z[i] = zi;
        // see "Hacker's Delight", section 2-12 (overflow detection)
        c = ((yi & !xi) | ((yi | !xi) & zi)) >> (big_digit::BITS - 1)
    }

    c
}

/// z1<<_W + z0 = x+y+c, with c == 0 or 1
#[inline(always)]
fn add_ww(x: BigDigit, y: BigDigit, c: BigDigit) -> (BigDigit, BigDigit) {
    let yc = y.wrapping_add(c);
    let z0 = x.wrapping_add(yc);
    let z1 = if z0 < x || yc < y { 1 } else { 0 };

    (z1, z0)
}

/// z1 << _W + z0 = x * y + c
#[inline(always)]
fn mul_add_www(x: BigDigit, y: BigDigit, c: BigDigit) -> (BigDigit, BigDigit) {
    let z = x as DoubleBigDigit * y as DoubleBigDigit + c as DoubleBigDigit;
    ((z >> big_digit::BITS) as BigDigit, z as BigDigit)
}

/// Calculates x ** y mod m using a fixed, 4-bit window.
#[allow(clippy::many_single_char_names)]
pub(super) fn monty_modpow(x: &BigUint, y: &BigUint, m: &BigUint) -> BigUint {
    assert!(m.data[0] & 1 == 1);
    let mr = MontyReducer::new(m);
    let num_words = m.data.len();

    let mut x = x.clone();

    // We want the lengths of x and m to be equal.
    // It is OK if x >= m as long as len(x) == len(m).
    if x.data.len() > num_words {
        x %= m;
        // Note: now len(x) <= numWords, not guaranteed ==.
    }
    if x.data.len() < num_words {
        x.data.resize(num_words, 0);
    }

    // rr = 2**(2*_W*len(m)) mod m
    let mut rr = BigUint::one();
    rr = (rr.shl(2 * num_words as u64 * u64::from(big_digit::BITS))) % m;
    if rr.data.len() < num_words {
        rr.data.resize(num_words, 0);
    }
    // one = 1, with equal length to that of m
    let mut one = BigUint::one();
    one.data.resize(num_words, 0);

    let n = 4;
    // powers[i] contains x^i
    let mut powers = Vec::with_capacity(1 << n);
    powers.push(montgomery(&one, &rr, m, mr.n0inv, num_words));
    powers.push(montgomery(&x, &rr, m, mr.n0inv, num_words));
    for i in 2..1 << n {
        let r = montgomery(&powers[i - 1], &powers[1], m, mr.n0inv, num_words);
        powers.push(r);
    }

    // initialize z = 1 (Montgomery 1)
    let mut z = powers[0].clone();
    z.data.resize(num_words, 0);
    let mut zz = BigUint::ZERO;
    zz.data.resize(num_words, 0);

    // same windowed exponent, but with Montgomery multiplications
    for i in (0..y.data.len()).rev() {
        let mut yi = y.data[i];
        let mut j = 0;
        while j < big_digit::BITS {
            if i != y.data.len() - 1 || j != 0 {
                zz = montgomery(&z, &z, m, mr.n0inv, num_words);
                z = montgomery(&zz, &zz, m, mr.n0inv, num_words);
                zz = montgomery(&z, &z, m, mr.n0inv, num_words);
                z = montgomery(&zz, &zz, m, mr.n0inv, num_words);
            }
            zz = montgomery(
                &z,
                &powers[(yi >> (big_digit::BITS - n)) as usize],
                m,
                mr.n0inv,
                num_words,
            );
            mem::swap(&mut z, &mut zz);
            yi <<= n;
            j += n;
        }
    }

    // convert to regular number
    zz = montgomery(&z, &one, m, mr.n0inv, num_words);

    zz.normalize();
    // One last reduction, just in case.
    // See golang.org/issue/13907.
    if zz >= *m {
        // Common case is m has high bit set; in that case,
        // since zz is the same length as m, there can be just
        // one multiple of m to remove. Just subtract.
        // We think that the subtract should be sufficient in general,
        // so do that unconditionally, but double-check,
        // in case our beliefs are wrong.
        // The div is not expected to be reached.
        zz -= m;
        if zz >= *m {
            zz %= m;
        }
    }

    zz.normalize();
    zz
}
