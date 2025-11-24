use num_bigint::BigUint;
use num_bigint::Sign::{Minus, NoSign, Plus};
use num_bigint::{BigInt, ToBigInt};

use std::cmp::Ordering::{Equal, Greater, Less};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::iter::repeat;
use std::ops::Neg;
use std::{f32, f64};

use num_integer::Integer;
use num_traits::{
    pow, Euclid, FromBytes, FromPrimitive, Num, One, Pow, Signed, ToBytes, ToPrimitive, Zero,
};

mod consts;
use crate::consts::*;

#[macro_use]
mod macros;

#[test]
fn test_from_bytes_be() {
    fn check(s: &str, result: &str) {
        assert_eq!(
            BigInt::from_bytes_be(Plus, s.as_bytes()),
            BigInt::parse_bytes(result.as_bytes(), 10).unwrap()
        );
    }
    check("A", "65");
    check("AA", "16705");
    check("AB", "16706");
    check("Hello world!", "22405534230753963835153736737");
    assert_eq!(BigInt::from_bytes_be(Plus, &[]), BigInt::zero());
    assert_eq!(BigInt::from_bytes_be(Minus, &[]), BigInt::zero());
}

#[test]
fn test_to_bytes_be() {
    fn check(s: &str, result: &str) {
        let b = BigInt::parse_bytes(result.as_bytes(), 10).unwrap();
        let (sign, v) = b.to_bytes_be();
        assert_eq!((Plus, s.as_bytes()), (sign, &*v));
    }
    check("A", "65");
    check("AA", "16705");
    check("AB", "16706");
    check("Hello world!", "22405534230753963835153736737");
    let b: BigInt = Zero::zero();
    assert_eq!(b.to_bytes_be(), (NoSign, vec![0]));

    // Test with leading/trailing zero bytes and a full BigDigit of value 0
    let b = BigInt::from_str_radix("00010000000000000200", 16).unwrap();
    assert_eq!(b.to_bytes_be(), (Plus, vec![1, 0, 0, 0, 0, 0, 0, 2, 0]));
}

#[test]
fn test_from_bytes_le() {
    fn check(s: &str, result: &str) {
        assert_eq!(
            BigInt::from_bytes_le(Plus, s.as_bytes()),
            BigInt::parse_bytes(result.as_bytes(), 10).unwrap()
        );
    }
    check("A", "65");
    check("AA", "16705");
    check("BA", "16706");
    check("!dlrow olleH", "22405534230753963835153736737");
    assert_eq!(BigInt::from_bytes_le(Plus, &[]), BigInt::zero());
    assert_eq!(BigInt::from_bytes_le(Minus, &[]), BigInt::zero());
}

#[test]
fn test_to_bytes_le() {
    fn check(s: &str, result: &str) {
        let b = BigInt::parse_bytes(result.as_bytes(), 10).unwrap();
        let (sign, v) = b.to_bytes_le();
        assert_eq!((Plus, s.as_bytes()), (sign, &*v));
    }
    check("A", "65");
    check("AA", "16705");
    check("BA", "16706");
    check("!dlrow olleH", "22405534230753963835153736737");
    let b: BigInt = Zero::zero();
    assert_eq!(b.to_bytes_le(), (NoSign, vec![0]));

    // Test with leading/trailing zero bytes and a full BigDigit of value 0
    let b = BigInt::from_str_radix("00010000000000000200", 16).unwrap();
    assert_eq!(b.to_bytes_le(), (Plus, vec![0, 2, 0, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn test_to_signed_bytes_le() {
    fn check(s: &str, result: Vec<u8>) {
        let b = BigInt::parse_bytes(s.as_bytes(), 10).unwrap();
        assert_eq!(b.to_signed_bytes_le(), result);
        assert_eq!(<BigInt as ToBytes>::to_le_bytes(&b), result);
    }

    check("0", vec![0]);
    check("32767", vec![0xff, 0x7f]);
    check("-1", vec![0xff]);
    check("16777216", vec![0, 0, 0, 1]);
    check("-100", vec![156]);
    check("-8388608", vec![0, 0, 0x80]);
    check("-192", vec![0x40, 0xff]);
    check("128", vec![0x80, 0])
}

#[test]
fn test_from_signed_bytes_le() {
    fn check(s: &[u8], result: &str) {
        let b = BigInt::parse_bytes(result.as_bytes(), 10).unwrap();
        assert_eq!(BigInt::from_signed_bytes_le(s), b);
        assert_eq!(<BigInt as FromBytes>::from_le_bytes(s), b);
    }

    check(&[], "0");
    check(&[0], "0");
    check(&[0; 10], "0");
    check(&[0xff, 0x7f], "32767");
    check(&[0xff], "-1");
    check(&[0, 0, 0, 1], "16777216");
    check(&[156], "-100");
    check(&[0, 0, 0x80], "-8388608");
    check(&[0xff; 10], "-1");
    check(&[0x40, 0xff], "-192");
}

#[test]
fn test_to_signed_bytes_be() {
    fn check(s: &str, result: Vec<u8>) {
        let b = BigInt::parse_bytes(s.as_bytes(), 10).unwrap();
        assert_eq!(b.to_signed_bytes_be(), result);
        assert_eq!(<BigInt as ToBytes>::to_be_bytes(&b), result);
    }

    check("0", vec![0]);
    check("32767", vec![0x7f, 0xff]);
    check("-1", vec![255]);
    check("16777216", vec![1, 0, 0, 0]);
    check("-100", vec![156]);
    check("-8388608", vec![128, 0, 0]);
    check("-192", vec![0xff, 0x40]);
    check("128", vec![0, 0x80]);
}

#[test]
fn test_from_signed_bytes_be() {
    fn check(s: &[u8], result: &str) {
        let b = BigInt::parse_bytes(result.as_bytes(), 10).unwrap();
        assert_eq!(BigInt::from_signed_bytes_be(s), b);
        assert_eq!(<BigInt as FromBytes>::from_be_bytes(s), b);
    }

    check(&[], "0");
    check(&[0], "0");
    check(&[0; 10], "0");
    check(&[127, 255], "32767");
    check(&[255], "-1");
    check(&[1, 0, 0, 0], "16777216");
    check(&[156], "-100");
    check(&[128, 0, 0], "-8388608");
    check(&[255; 10], "-1");
    check(&[0xff, 0x40], "-192");
}

#[test]
fn test_signed_bytes_be_round_trip() {
    for i in -0x1FFFF..0x20000 {
        let n = BigInt::from(i);
        assert_eq!(n, BigInt::from_signed_bytes_be(&n.to_signed_bytes_be()));
    }
}

#[test]
fn test_signed_bytes_le_round_trip() {
    for i in -0x1FFFF..0x20000 {
        let n = BigInt::from(i);
        assert_eq!(n, BigInt::from_signed_bytes_le(&n.to_signed_bytes_le()));
    }
}

#[test]
fn test_cmp() {
    let vs: [&[u32]; 4] = [&[2_u32], &[1, 1], &[2, 1], &[1, 1, 1]];
    let mut nums = Vec::new();
    for s in vs.iter().rev() {
        nums.push(BigInt::from_slice(Minus, *s));
    }
    nums.push(Zero::zero());
    nums.extend(vs.iter().map(|s| BigInt::from_slice(Plus, *s)));

    for (i, ni) in nums.iter().enumerate() {
        for (j0, nj) in nums[i..].iter().enumerate() {
            let j = i + j0;
            if i == j {
                assert_eq!(ni.cmp(nj), Equal);
                assert_eq!(nj.cmp(ni), Equal);
                assert_eq!(ni, nj);
                assert!(!(ni != nj));
                assert!(ni <= nj);
                assert!(ni >= nj);
                assert!(!(ni < nj));
                assert!(!(ni > nj));
            } else {
                assert_eq!(ni.cmp(nj), Less);
                assert_eq!(nj.cmp(ni), Greater);

                assert!(!(ni == nj));
                assert!(ni != nj);

                assert!(ni <= nj);
                assert!(!(ni >= nj));
                assert!(ni < nj);
                assert!(!(ni > nj));

                assert!(!(nj <= ni));
                assert!(nj >= ni);
                assert!(!(nj < ni));
                assert!(nj > ni);
            }
        }
    }
}

fn hash<T: Hash>(x: &T) -> u64 {
    let mut hasher = <RandomState as BuildHasher>::Hasher::new();
    x.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn test_hash() {
    let a = BigInt::new(NoSign, vec![]);
    let b = BigInt::new(NoSign, vec![0]);
    let c = BigInt::new(Plus, vec![1]);
    let d = BigInt::new(Plus, vec![1, 0, 0, 0, 0, 0]);
    let e = BigInt::new(Plus, vec![0, 0, 0, 0, 0, 1]);
    let f = BigInt::new(Minus, vec![1]);
    assert!(hash(&a) == hash(&b));
    assert!(hash(&b) != hash(&c));
    assert!(hash(&c) == hash(&d));
    assert!(hash(&d) != hash(&e));
    assert!(hash(&c) != hash(&f));
}

#[test]
fn test_convert_i64() {
    fn check(b1: BigInt, i: i64) {
        let b2: BigInt = FromPrimitive::from_i64(i).unwrap();
        assert!(b1 == b2);
        assert!(b1.to_i64().unwrap() == i);
    }

    check(Zero::zero(), 0);
    check(One::one(), 1);
    check(i64::MIN.to_bigint().unwrap(), i64::MIN);
    check(i64::MAX.to_bigint().unwrap(), i64::MAX);

    assert_eq!((i64::MAX as u64 + 1).to_bigint().unwrap().to_i64(), None);

    assert_eq!(
        BigInt::from_biguint(Plus, BigUint::new(vec![1, 2, 3, 4, 5])).to_i64(),
        None
    );

    assert_eq!(
        BigInt::from_biguint(Minus, BigUint::new(vec![1, 0, 0, 1 << 31])).to_i64(),
        None
    );

    assert_eq!(
        BigInt::from_biguint(Minus, BigUint::new(vec![1, 2, 3, 4, 5])).to_i64(),
        None
    );
}

#[test]
fn test_convert_i128() {
    fn check(b1: BigInt, i: i128) {
        let b2: BigInt = FromPrimitive::from_i128(i).unwrap();
        assert!(b1 == b2);
        assert!(b1.to_i128().unwrap() == i);
    }

    check(Zero::zero(), 0);
    check(One::one(), 1);
    check(i128::MIN.to_bigint().unwrap(), i128::MIN);
    check(i128::MAX.to_bigint().unwrap(), i128::MAX);

    assert_eq!((i128::MAX as u128 + 1).to_bigint().unwrap().to_i128(), None);

    assert_eq!(
        BigInt::from_biguint(Plus, BigUint::new(vec![1, 2, 3, 4, 5])).to_i128(),
        None
    );

    assert_eq!(
        BigInt::from_biguint(Minus, BigUint::new(vec![1, 0, 0, 1 << 31])).to_i128(),
        None
    );

    assert_eq!(
        BigInt::from_biguint(Minus, BigUint::new(vec![1, 2, 3, 4, 5])).to_i128(),
        None
    );
}

#[test]
fn test_convert_u64() {
    fn check(b1: BigInt, u: u64) {
        let b2: BigInt = FromPrimitive::from_u64(u).unwrap();
        assert!(b1 == b2);
        assert!(b1.to_u64().unwrap() == u);
    }

    check(Zero::zero(), 0);
    check(One::one(), 1);
    check(u64::MIN.to_bigint().unwrap(), u64::MIN);
    check(u64::MAX.to_bigint().unwrap(), u64::MAX);

    assert_eq!(
        BigInt::from_biguint(Plus, BigUint::new(vec![1, 2, 3, 4, 5])).to_u64(),
        None
    );

    let max_value: BigUint = FromPrimitive::from_u64(u64::MAX).unwrap();
    assert_eq!(BigInt::from_biguint(Minus, max_value).to_u64(), None);
    assert_eq!(
        BigInt::from_biguint(Minus, BigUint::new(vec![1, 2, 3, 4, 5])).to_u64(),
        None
    );
}

#[test]
fn test_convert_u128() {
    fn check(b1: BigInt, u: u128) {
        let b2: BigInt = FromPrimitive::from_u128(u).unwrap();
        assert!(b1 == b2);
        assert!(b1.to_u128().unwrap() == u);
    }

    check(Zero::zero(), 0);
    check(One::one(), 1);
    check(u128::MIN.to_bigint().unwrap(), u128::MIN);
    check(u128::MAX.to_bigint().unwrap(), u128::MAX);

    assert_eq!(
        BigInt::from_biguint(Plus, BigUint::new(vec![1, 2, 3, 4, 5])).to_u128(),
        None
    );

    let max_value: BigUint = FromPrimitive::from_u128(u128::MAX).unwrap();
    assert_eq!(BigInt::from_biguint(Minus, max_value).to_u128(), None);
    assert_eq!(
        BigInt::from_biguint(Minus, BigUint::new(vec![1, 2, 3, 4, 5])).to_u128(),
        None
    );
}

#[test]
#[allow(clippy::float_cmp)]
fn test_convert_f32() {
    fn check(b1: &BigInt, f: f32) {
        let b2 = BigInt::from_f32(f).unwrap();
        assert_eq!(b1, &b2);
        assert_eq!(b1.to_f32().unwrap(), f);
        let neg_b1 = -b1;
        let neg_b2 = BigInt::from_f32(-f).unwrap();
        assert_eq!(neg_b1, neg_b2);
        assert_eq!(neg_b1.to_f32().unwrap(), -f);
    }

    check(&BigInt::zero(), 0.0);
    check(&BigInt::one(), 1.0);
    check(&BigInt::from(u16::MAX), pow(2.0_f32, 16) - 1.0);
    check(&BigInt::from(1u64 << 32), pow(2.0_f32, 32));
    check(&BigInt::from_slice(Plus, &[0, 0, 1]), pow(2.0_f32, 64));
    check(
        &((BigInt::one() << 100) + (BigInt::one() << 123)),
        pow(2.0_f32, 100) + pow(2.0_f32, 123),
    );
    check(&(BigInt::one() << 127), pow(2.0_f32, 127));
    check(&(BigInt::from((1u64 << 24) - 1) << (128 - 24)), f32::MAX);

    // keeping all 24 digits with the bits at different offsets to the BigDigits
    let x: u32 = 0b00000000101111011111011011011101;
    let mut f = x as f32;
    let mut b = BigInt::from(x);
    for _ in 0..64 {
        check(&b, f);
        f *= 2.0;
        b <<= 1;
    }

    // this number when rounded to f64 then f32 isn't the same as when rounded straight to f32
    let mut n: i64 = 0b0000000000111111111111111111111111011111111111111111111111111111;
    assert!((n as f64) as f32 != n as f32);
    assert_eq!(BigInt::from(n).to_f32(), Some(n as f32));
    n = -n;
    assert!((n as f64) as f32 != n as f32);
    assert_eq!(BigInt::from(n).to_f32(), Some(n as f32));

    // test rounding up with the bits at different offsets to the BigDigits
    let mut f = ((1u64 << 25) - 1) as f32;
    let mut b = BigInt::from(1u64 << 25);
    for _ in 0..64 {
        assert_eq!(b.to_f32(), Some(f));
        f *= 2.0;
        b <<= 1;
    }

    // test correct ties-to-even rounding
    let weird: i128 = (1i128 << 100) + (1i128 << (100 - f32::MANTISSA_DIGITS));
    assert_ne!(weird as f32, (weird + 1) as f32);

    assert_eq!(BigInt::from(weird).to_f32(), Some(weird as f32));
    assert_eq!(BigInt::from(weird + 1).to_f32(), Some((weird + 1) as f32));

    // rounding
    assert_eq!(
        BigInt::from_f32(-f32::consts::PI),
        Some(BigInt::from(-3i32))
    );
    assert_eq!(BigInt::from_f32(-f32::consts::E), Some(BigInt::from(-2i32)));
    assert_eq!(BigInt::from_f32(-0.99999), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f32(-0.5), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f32(-0.0), Some(BigInt::zero()));
    assert_eq!(
        BigInt::from_f32(f32::MIN_POSITIVE / 2.0),
        Some(BigInt::zero())
    );
    assert_eq!(BigInt::from_f32(f32::MIN_POSITIVE), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f32(0.5), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f32(0.99999), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f32(f32::consts::E), Some(BigInt::from(2u32)));
    assert_eq!(BigInt::from_f32(f32::consts::PI), Some(BigInt::from(3u32)));

    // special float values
    assert_eq!(BigInt::from_f32(f32::NAN), None);
    assert_eq!(BigInt::from_f32(f32::INFINITY), None);
    assert_eq!(BigInt::from_f32(f32::NEG_INFINITY), None);

    // largest BigInt that will round to a finite f32 value
    let big_num = (BigInt::one() << 128u8) - 1u8 - (BigInt::one() << (128u8 - 25));
    assert_eq!(big_num.to_f32(), Some(f32::MAX));
    assert_eq!((&big_num + 1u8).to_f32(), Some(f32::INFINITY));
    assert_eq!((-&big_num).to_f32(), Some(f32::MIN));
    assert_eq!(((-&big_num) - 1u8).to_f32(), Some(f32::NEG_INFINITY));

    assert_eq!(
        ((BigInt::one() << 128u8) - 1u8).to_f32(),
        Some(f32::INFINITY)
    );
    assert_eq!((BigInt::one() << 128u8).to_f32(), Some(f32::INFINITY));
    assert_eq!(
        (-((BigInt::one() << 128u8) - 1u8)).to_f32(),
        Some(f32::NEG_INFINITY)
    );
    assert_eq!(
        (-(BigInt::one() << 128u8)).to_f32(),
        Some(f32::NEG_INFINITY)
    );
}

#[test]
#[allow(clippy::float_cmp)]
fn test_convert_f64() {
    fn check(b1: &BigInt, f: f64) {
        let b2 = BigInt::from_f64(f).unwrap();
        assert_eq!(b1, &b2);
        assert_eq!(b1.to_f64().unwrap(), f);
        let neg_b1 = -b1;
        let neg_b2 = BigInt::from_f64(-f).unwrap();
        assert_eq!(neg_b1, neg_b2);
        assert_eq!(neg_b1.to_f64().unwrap(), -f);
    }

    check(&BigInt::zero(), 0.0);
    check(&BigInt::one(), 1.0);
    check(&BigInt::from(u32::MAX), pow(2.0_f64, 32) - 1.0);
    check(&BigInt::from(1u64 << 32), pow(2.0_f64, 32));
    check(&BigInt::from_slice(Plus, &[0, 0, 1]), pow(2.0_f64, 64));
    check(
        &((BigInt::one() << 100) + (BigInt::one() << 152)),
        pow(2.0_f64, 100) + pow(2.0_f64, 152),
    );
    check(&(BigInt::one() << 1023), pow(2.0_f64, 1023));
    check(&(BigInt::from((1u64 << 53) - 1) << (1024 - 53)), f64::MAX);

    // keeping all 53 digits with the bits at different offsets to the BigDigits
    let x: u64 = 0b0000000000011110111110110111111101110111101111011111011011011101;
    let mut f = x as f64;
    let mut b = BigInt::from(x);
    for _ in 0..128 {
        check(&b, f);
        f *= 2.0;
        b <<= 1;
    }

    // test rounding up with the bits at different offsets to the BigDigits
    let mut f = ((1u64 << 54) - 1) as f64;
    let mut b = BigInt::from(1u64 << 54);
    for _ in 0..128 {
        assert_eq!(b.to_f64(), Some(f));
        f *= 2.0;
        b <<= 1;
    }

    // test correct ties-to-even rounding
    let weird: i128 = (1i128 << 100) + (1i128 << (100 - f64::MANTISSA_DIGITS));
    assert_ne!(weird as f64, (weird + 1) as f64);

    assert_eq!(BigInt::from(weird).to_f64(), Some(weird as f64));
    assert_eq!(BigInt::from(weird + 1).to_f64(), Some((weird + 1) as f64));

    // rounding
    assert_eq!(
        BigInt::from_f64(-f64::consts::PI),
        Some(BigInt::from(-3i32))
    );
    assert_eq!(BigInt::from_f64(-f64::consts::E), Some(BigInt::from(-2i32)));
    assert_eq!(BigInt::from_f64(-0.99999), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f64(-0.5), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f64(-0.0), Some(BigInt::zero()));
    assert_eq!(
        BigInt::from_f64(f64::MIN_POSITIVE / 2.0),
        Some(BigInt::zero())
    );
    assert_eq!(BigInt::from_f64(f64::MIN_POSITIVE), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f64(0.5), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f64(0.99999), Some(BigInt::zero()));
    assert_eq!(BigInt::from_f64(f64::consts::E), Some(BigInt::from(2u32)));
    assert_eq!(BigInt::from_f64(f64::consts::PI), Some(BigInt::from(3u32)));

    // special float values
    assert_eq!(BigInt::from_f64(f64::NAN), None);
    assert_eq!(BigInt::from_f64(f64::INFINITY), None);
    assert_eq!(BigInt::from_f64(f64::NEG_INFINITY), None);

    // largest BigInt that will round to a finite f64 value
    let big_num = (BigInt::one() << 1024u16) - 1u8 - (BigInt::one() << (1024u16 - 54));
    assert_eq!(big_num.to_f64(), Some(f64::MAX));
    assert_eq!((&big_num + 1u8).to_f64(), Some(f64::INFINITY));
    assert_eq!((-&big_num).to_f64(), Some(f64::MIN));
    assert_eq!(((-&big_num) - 1u8).to_f64(), Some(f64::NEG_INFINITY));

    assert_eq!(
        ((BigInt::one() << 1024u16) - 1u8).to_f64(),
        Some(f64::INFINITY)
    );
    assert_eq!((BigInt::one() << 1024u16).to_f64(), Some(f64::INFINITY));
    assert_eq!(
        (-((BigInt::one() << 1024u16) - 1u8)).to_f64(),
        Some(f64::NEG_INFINITY)
    );
    assert_eq!(
        (-(BigInt::one() << 1024u16)).to_f64(),
        Some(f64::NEG_INFINITY)
    );
}

#[test]
fn test_convert_to_biguint() {
    fn check(n: BigInt, ans_1: BigUint) {
        assert_eq!(n.to_biguint().unwrap(), ans_1);
        assert_eq!(n.to_biguint().unwrap().to_bigint().unwrap(), n);
    }
    let zero: BigInt = Zero::zero();
    let unsigned_zero: BigUint = Zero::zero();
    let positive = BigInt::from_biguint(Plus, BigUint::new(vec![1, 2, 3]));
    let negative = -&positive;

    check(zero, unsigned_zero);
    check(positive, BigUint::new(vec![1, 2, 3]));

    assert_eq!(negative.to_biguint(), None);
}

#[test]
fn test_convert_from_uint() {
    macro_rules! check {
        ($ty:ident, $max:expr) => {
            assert_eq!(BigInt::from($ty::zero()), BigInt::zero());
            assert_eq!(BigInt::from($ty::one()), BigInt::one());
            assert_eq!(BigInt::from($ty::MAX - $ty::one()), $max - BigInt::one());
            assert_eq!(BigInt::from($ty::MAX), $max);
        };
    }

    check!(u8, BigInt::from_slice(Plus, &[u8::MAX as u32]));
    check!(u16, BigInt::from_slice(Plus, &[u16::MAX as u32]));
    check!(u32, BigInt::from_slice(Plus, &[u32::MAX]));
    check!(u64, BigInt::from_slice(Plus, &[u32::MAX, u32::MAX]));
    check!(
        u128,
        BigInt::from_slice(Plus, &[u32::MAX, u32::MAX, u32::MAX, u32::MAX])
    );
    check!(usize, BigInt::from(usize::MAX as u64));
}

#[test]
fn test_convert_from_int() {
    macro_rules! check {
        ($ty:ident, $min:expr, $max:expr) => {
            assert_eq!(BigInt::from($ty::MIN), $min);
            assert_eq!(BigInt::from($ty::MIN + $ty::one()), $min + BigInt::one());
            assert_eq!(BigInt::from(-$ty::one()), -BigInt::one());
            assert_eq!(BigInt::from($ty::zero()), BigInt::zero());
            assert_eq!(BigInt::from($ty::one()), BigInt::one());
            assert_eq!(BigInt::from($ty::MAX - $ty::one()), $max - BigInt::one());
            assert_eq!(BigInt::from($ty::MAX), $max);
        };
    }

    check!(
        i8,
        BigInt::from_slice(Minus, &[1 << 7]),
        BigInt::from_slice(Plus, &[i8::MAX as u32])
    );
    check!(
        i16,
        BigInt::from_slice(Minus, &[1 << 15]),
        BigInt::from_slice(Plus, &[i16::MAX as u32])
    );
    check!(
        i32,
        BigInt::from_slice(Minus, &[1 << 31]),
        BigInt::from_slice(Plus, &[i32::MAX as u32])
    );
    check!(
        i64,
        BigInt::from_slice(Minus, &[0, 1 << 31]),
        BigInt::from_slice(Plus, &[u32::MAX, i32::MAX as u32])
    );
    check!(
        i128,
        BigInt::from_slice(Minus, &[0, 0, 0, 1 << 31]),
        BigInt::from_slice(Plus, &[u32::MAX, u32::MAX, u32::MAX, i32::MAX as u32])
    );
    check!(
        isize,
        BigInt::from(isize::MIN as i64),
        BigInt::from(isize::MAX as i64)
    );
}

#[test]
fn test_convert_from_biguint() {
    assert_eq!(BigInt::from(BigUint::zero()), BigInt::zero());
    assert_eq!(BigInt::from(BigUint::one()), BigInt::one());
    assert_eq!(
        BigInt::from(BigUint::from_slice(&[1, 2, 3])),
        BigInt::from_slice(Plus, &[1, 2, 3])
    );
}

#[test]
fn test_add() {
    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let (na, nb, nc) = (-&a, -&b, -&c);

        assert_op!(a + b == c);
        assert_op!(b + a == c);
        assert_op!(c + na == b);
        assert_op!(c + nb == a);
        assert_op!(a + nc == nb);
        assert_op!(b + nc == na);
        assert_op!(na + nb == nc);
        assert_op!(a + na == BigInt::zero());

        assert_assign_op!(a += b == c);
        assert_assign_op!(b += a == c);
        assert_assign_op!(c += na == b);
        assert_assign_op!(c += nb == a);
        assert_assign_op!(a += nc == nb);
        assert_assign_op!(b += nc == na);
        assert_assign_op!(na += nb == nc);
        assert_assign_op!(a += na == BigInt::zero());
    }
}

#[test]
fn test_sub() {
    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let (na, nb, nc) = (-&a, -&b, -&c);

        assert_op!(c - a == b);
        assert_op!(c - b == a);
        assert_op!(nb - a == nc);
        assert_op!(na - b == nc);
        assert_op!(b - na == c);
        assert_op!(a - nb == c);
        assert_op!(nc - na == nb);
        assert_op!(a - a == BigInt::zero());

        assert_assign_op!(c -= a == b);
        assert_assign_op!(c -= b == a);
        assert_assign_op!(nb -= a == nc);
        assert_assign_op!(na -= b == nc);
        assert_assign_op!(b -= na == c);
        assert_assign_op!(a -= nb == c);
        assert_assign_op!(nc -= na == nb);
        assert_assign_op!(a -= a == BigInt::zero());
    }
}

#[test]
fn test_mul() {
    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let (na, nb, nc) = (-&a, -&b, -&c);

        assert_op!(a * b == c);
        assert_op!(b * a == c);
        assert_op!(na * nb == c);

        assert_op!(na * b == nc);
        assert_op!(nb * a == nc);

        assert_assign_op!(a *= b == c);
        assert_assign_op!(b *= a == c);
        assert_assign_op!(na *= nb == c);

        assert_assign_op!(na *= b == nc);
        assert_assign_op!(nb *= a == nc);
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let d = BigInt::from_slice(Plus, d_vec);

        assert!(a == &b * &c + &d);
        assert!(a == &c * &b + &d);
    }
}

#[test]
fn test_div_mod_floor() {
    fn check_sub(a: &BigInt, b: &BigInt, ans_d: &BigInt, ans_m: &BigInt) {
        let (d, m) = a.div_mod_floor(b);
        assert_eq!(d, a.div_floor(b));
        assert_eq!(m, a.mod_floor(b));
        if !m.is_zero() {
            assert_eq!(m.sign(), b.sign());
        }
        assert!(m.abs() <= b.abs());
        assert!(*a == b * &d + &m);
        assert!(d == *ans_d);
        assert!(m == *ans_m);
    }

    fn check(a: &BigInt, b: &BigInt, d: &BigInt, m: &BigInt) {
        if m.is_zero() {
            check_sub(a, b, d, m);
            check_sub(a, &b.neg(), &d.neg(), m);
            check_sub(&a.neg(), b, &d.neg(), m);
            check_sub(&a.neg(), &b.neg(), d, m);
        } else {
            let one: BigInt = One::one();
            check_sub(a, b, d, m);
            check_sub(a, &b.neg(), &(d.neg() - &one), &(m - b));
            check_sub(&a.neg(), b, &(d.neg() - &one), &(b - m));
            check_sub(&a.neg(), &b.neg(), d, &m.neg());
        }
    }

    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);

        if !a.is_zero() {
            check(&c, &a, &b, &Zero::zero());
        }
        if !b.is_zero() {
            check(&c, &b, &a, &Zero::zero());
        }
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let d = BigInt::from_slice(Plus, d_vec);

        if !b.is_zero() {
            check(&a, &b, &c, &d);
        }
    }
}

#[test]
fn test_div_rem() {
    fn check_sub(a: &BigInt, b: &BigInt, ans_q: &BigInt, ans_r: &BigInt) {
        let (q, r) = a.div_rem(b);
        if !r.is_zero() {
            assert_eq!(r.sign(), a.sign());
        }
        assert!(r.abs() <= b.abs());
        assert!(*a == b * &q + &r);
        assert!(q == *ans_q);
        assert!(r == *ans_r);

        let (a, b, ans_q, ans_r) = (a.clone(), b.clone(), ans_q.clone(), ans_r.clone());
        assert_op!(a / b == ans_q);
        assert_op!(a % b == ans_r);
        assert_assign_op!(a /= b == ans_q);
        assert_assign_op!(a %= b == ans_r);
    }

    fn check(a: &BigInt, b: &BigInt, q: &BigInt, r: &BigInt) {
        check_sub(a, b, q, r);
        check_sub(a, &b.neg(), &q.neg(), r);
        check_sub(&a.neg(), b, &q.neg(), &r.neg());
        check_sub(&a.neg(), &b.neg(), q, &r.neg());
    }
    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);

        if !a.is_zero() {
            check(&c, &a, &b, &Zero::zero());
        }
        if !b.is_zero() {
            check(&c, &b, &a, &Zero::zero());
        }
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let d = BigInt::from_slice(Plus, d_vec);

        if !b.is_zero() {
            check(&a, &b, &c, &d);
        }
    }
}

#[test]
fn test_div_ceil() {
    fn check_sub(a: &BigInt, b: &BigInt, ans_d: &BigInt) {
        assert_eq!(a.div_ceil(b), *ans_d);
    }

    fn check(a: &BigInt, b: &BigInt, d: &BigInt, m: &BigInt) {
        if m.is_zero() {
            check_sub(a, b, d);
            check_sub(a, &b.neg(), &d.neg());
            check_sub(&a.neg(), b, &d.neg());
            check_sub(&a.neg(), &b.neg(), d);
        } else {
            check_sub(a, b, &(d + 1));
            check_sub(a, &b.neg(), &d.neg());
            check_sub(&a.neg(), b, &d.neg());
            check_sub(&a.neg(), &b.neg(), &(d + 1));
        }
    }

    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);

        if !a.is_zero() {
            check(&c, &a, &b, &Zero::zero());
        }
        if !b.is_zero() {
            check(&c, &b, &a, &Zero::zero());
        }
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let d = BigInt::from_slice(Plus, d_vec);

        if !b.is_zero() {
            check(&a, &b, &c, &d);
        }
    }
}

#[test]
fn test_div_rem_euclid() {
    fn check_sub(a: &BigInt, b: &BigInt, ans_d: &BigInt, ans_m: &BigInt) {
        eprintln!("{} {} {} {}", a, b, ans_d, ans_m);
        assert_eq!(a.div_euclid(b), *ans_d);
        assert_eq!(a.rem_euclid(b), *ans_m);
        assert!(*ans_m >= BigInt::zero());
        assert!(*ans_m < b.abs());
    }

    fn check(a: &BigInt, b: &BigInt, d: &BigInt, m: &BigInt) {
        if m.is_zero() {
            check_sub(a, b, d, m);
            check_sub(a, &b.neg(), &d.neg(), m);
            check_sub(&a.neg(), b, &d.neg(), m);
            check_sub(&a.neg(), &b.neg(), d, m);
        } else {
            let one: BigInt = One::one();
            check_sub(a, b, d, m);
            check_sub(a, &b.neg(), &d.neg(), m);
            check_sub(&a.neg(), b, &(d + &one).neg(), &(b - m));
            check_sub(&a.neg(), &b.neg(), &(d + &one), &(b.abs() - m));
        }
    }

    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);

        if !a.is_zero() {
            check(&c, &a, &b, &Zero::zero());
        }
        if !b.is_zero() {
            check(&c, &b, &a, &Zero::zero());
        }
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let d = BigInt::from_slice(Plus, d_vec);

        if !b.is_zero() {
            check(&a, &b, &c, &d);
        }
    }
}

#[test]
fn test_checked_add() {
    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);

        assert!(a.checked_add(&b).unwrap() == c);
        assert!(b.checked_add(&a).unwrap() == c);
        assert!(c.checked_add(&(-&a)).unwrap() == b);
        assert!(c.checked_add(&(-&b)).unwrap() == a);
        assert!(a.checked_add(&(-&c)).unwrap() == (-&b));
        assert!(b.checked_add(&(-&c)).unwrap() == (-&a));
        assert!((-&a).checked_add(&(-&b)).unwrap() == (-&c));
        assert!(a.checked_add(&(-&a)).unwrap() == BigInt::zero());
    }
}

#[test]
fn test_checked_sub() {
    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);

        assert!(c.checked_sub(&a).unwrap() == b);
        assert!(c.checked_sub(&b).unwrap() == a);
        assert!((-&b).checked_sub(&a).unwrap() == (-&c));
        assert!((-&a).checked_sub(&b).unwrap() == (-&c));
        assert!(b.checked_sub(&(-&a)).unwrap() == c);
        assert!(a.checked_sub(&(-&b)).unwrap() == c);
        assert!((-&c).checked_sub(&(-&a)).unwrap() == (-&b));
        assert!(a.checked_sub(&a).unwrap() == BigInt::zero());
    }
}

#[test]
fn test_checked_mul() {
    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);

        assert!(a.checked_mul(&b).unwrap() == c);
        assert!(b.checked_mul(&a).unwrap() == c);

        assert!((-&a).checked_mul(&b).unwrap() == -&c);
        assert!((-&b).checked_mul(&a).unwrap() == -&c);
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);
        let d = BigInt::from_slice(Plus, d_vec);

        assert!(a == b.checked_mul(&c).unwrap() + &d);
        assert!(a == c.checked_mul(&b).unwrap() + &d);
    }
}
#[test]
fn test_checked_div() {
    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigInt::from_slice(Plus, a_vec);
        let b = BigInt::from_slice(Plus, b_vec);
        let c = BigInt::from_slice(Plus, c_vec);

        if !a.is_zero() {
            assert!(c.checked_div(&a).unwrap() == b);
            assert!((-&c).checked_div(&(-&a)).unwrap() == b);
            assert!((-&c).checked_div(&a).unwrap() == -&b);
        }
        if !b.is_zero() {
            assert!(c.checked_div(&b).unwrap() == a);
            assert!((-&c).checked_div(&(-&b)).unwrap() == a);
            assert!((-&c).checked_div(&b).unwrap() == -&a);
        }

        assert!(c.checked_div(&Zero::zero()).is_none());
        assert!((-&c).checked_div(&Zero::zero()).is_none());
    }
}

#[test]
fn test_gcd() {
    fn check(a: isize, b: isize, c: isize) {
        let big_a: BigInt = FromPrimitive::from_isize(a).unwrap();
        let big_b: BigInt = FromPrimitive::from_isize(b).unwrap();
        let big_c: BigInt = FromPrimitive::from_isize(c).unwrap();

        assert_eq!(big_a.gcd(&big_b), big_c);
        assert_eq!(big_a.extended_gcd(&big_b).gcd, big_c);
        assert_eq!(big_a.gcd_lcm(&big_b).0, big_c);
        assert_eq!(big_a.extended_gcd_lcm(&big_b).0.gcd, big_c);
    }

    check(10, 2, 2);
    check(10, 3, 1);
    check(0, 3, 3);
    check(3, 3, 3);
    check(56, 42, 14);
    check(3, -3, 3);
    check(-6, 3, 3);
    check(-4, -2, 2);
}

#[test]
fn test_lcm() {
    fn check(a: isize, b: isize, c: isize) {
        let big_a: BigInt = FromPrimitive::from_isize(a).unwrap();
        let big_b: BigInt = FromPrimitive::from_isize(b).unwrap();
        let big_c: BigInt = FromPrimitive::from_isize(c).unwrap();

        assert_eq!(big_a.lcm(&big_b), big_c);
        assert_eq!(big_a.gcd_lcm(&big_b).1, big_c);
        assert_eq!(big_a.extended_gcd_lcm(&big_b).1, big_c);
    }

    check(0, 0, 0);
    check(1, 0, 0);
    check(0, 1, 0);
    check(1, 1, 1);
    check(-1, 1, 1);
    check(1, -1, 1);
    check(-1, -1, 1);
    check(8, 9, 72);
    check(11, 5, 55);
}

#[test]
fn test_is_multiple_of() {
    assert!(BigInt::from(0).is_multiple_of(&BigInt::from(0)));
    assert!(BigInt::from(6).is_multiple_of(&BigInt::from(6)));
    assert!(BigInt::from(6).is_multiple_of(&BigInt::from(3)));
    assert!(BigInt::from(6).is_multiple_of(&BigInt::from(1)));

    assert!(!BigInt::from(42).is_multiple_of(&BigInt::from(5)));
    assert!(!BigInt::from(5).is_multiple_of(&BigInt::from(3)));
    assert!(!BigInt::from(42).is_multiple_of(&BigInt::from(0)));
}

#[test]
fn test_next_multiple_of() {
    assert_eq!(
        BigInt::from(16).next_multiple_of(&BigInt::from(8)),
        BigInt::from(16)
    );
    assert_eq!(
        BigInt::from(23).next_multiple_of(&BigInt::from(8)),
        BigInt::from(24)
    );
    assert_eq!(
        BigInt::from(16).next_multiple_of(&BigInt::from(-8)),
        BigInt::from(16)
    );
    assert_eq!(
        BigInt::from(23).next_multiple_of(&BigInt::from(-8)),
        BigInt::from(16)
    );
    assert_eq!(
        BigInt::from(-16).next_multiple_of(&BigInt::from(8)),
        BigInt::from(-16)
    );
    assert_eq!(
        BigInt::from(-23).next_multiple_of(&BigInt::from(8)),
        BigInt::from(-16)
    );
    assert_eq!(
        BigInt::from(-16).next_multiple_of(&BigInt::from(-8)),
        BigInt::from(-16)
    );
    assert_eq!(
        BigInt::from(-23).next_multiple_of(&BigInt::from(-8)),
        BigInt::from(-24)
    );
}

#[test]
fn test_prev_multiple_of() {
    assert_eq!(
        BigInt::from(16).prev_multiple_of(&BigInt::from(8)),
        BigInt::from(16)
    );
    assert_eq!(
        BigInt::from(23).prev_multiple_of(&BigInt::from(8)),
        BigInt::from(16)
    );
    assert_eq!(
        BigInt::from(16).prev_multiple_of(&BigInt::from(-8)),
        BigInt::from(16)
    );
    assert_eq!(
        BigInt::from(23).prev_multiple_of(&BigInt::from(-8)),
        BigInt::from(24)
    );
    assert_eq!(
        BigInt::from(-16).prev_multiple_of(&BigInt::from(8)),
        BigInt::from(-16)
    );
    assert_eq!(
        BigInt::from(-23).prev_multiple_of(&BigInt::from(8)),
        BigInt::from(-24)
    );
    assert_eq!(
        BigInt::from(-16).prev_multiple_of(&BigInt::from(-8)),
        BigInt::from(-16)
    );
    assert_eq!(
        BigInt::from(-23).prev_multiple_of(&BigInt::from(-8)),
        BigInt::from(-16)
    );
}

#[test]
fn test_abs_sub() {
    let zero: BigInt = Zero::zero();
    let one: BigInt = One::one();
    assert_eq!((-&one).abs_sub(&one), zero);
    let one: BigInt = One::one();
    let zero: BigInt = Zero::zero();
    assert_eq!(one.abs_sub(&one), zero);
    let one: BigInt = One::one();
    let zero: BigInt = Zero::zero();
    assert_eq!(one.abs_sub(&zero), one);
    let one: BigInt = One::one();
    let two: BigInt = FromPrimitive::from_isize(2).unwrap();
    assert_eq!(one.abs_sub(&-&one), two);
}

#[test]
fn test_from_str_radix() {
    fn check(s: &str, ans: Option<isize>) {
        let ans = ans.map(|n| {
            let x: BigInt = FromPrimitive::from_isize(n).unwrap();
            x
        });
        assert_eq!(BigInt::from_str_radix(s, 10).ok(), ans);
    }
    check("10", Some(10));
    check("1", Some(1));
    check("0", Some(0));
    check("-1", Some(-1));
    check("-10", Some(-10));
    check("+10", Some(10));
    check("--7", None);
    check("++5", None);
    check("+-9", None);
    check("-+3", None);
    check("Z", None);
    check("_", None);

    // issue 10522, this hit an edge case that caused it to
    // attempt to allocate a vector of size (-1u) == huge.
    let x: BigInt = format!("1{}", repeat("0").take(36).collect::<String>())
        .parse()
        .unwrap();
    let _y = x.to_string();
}

#[test]
fn test_lower_hex() {
    let a = BigInt::parse_bytes(b"A", 16).unwrap();
    let hello = BigInt::parse_bytes(b"-22405534230753963835153736737", 10).unwrap();

    assert_eq!(format!("{:x}", a), "a");
    assert_eq!(format!("{:x}", hello), "-48656c6c6f20776f726c6421");
    assert_eq!(format!("{:♥>+#8x}", a), "♥♥♥♥+0xa");
}

#[test]
fn test_upper_hex() {
    let a = BigInt::parse_bytes(b"A", 16).unwrap();
    let hello = BigInt::parse_bytes(b"-22405534230753963835153736737", 10).unwrap();

    assert_eq!(format!("{:X}", a), "A");
    assert_eq!(format!("{:X}", hello), "-48656C6C6F20776F726C6421");
    assert_eq!(format!("{:♥>+#8X}", a), "♥♥♥♥+0xA");
}

#[test]
fn test_binary() {
    let a = BigInt::parse_bytes(b"A", 16).unwrap();
    let hello = BigInt::parse_bytes(b"-224055342307539", 10).unwrap();

    assert_eq!(format!("{:b}", a), "1010");
    assert_eq!(
        format!("{:b}", hello),
        "-110010111100011011110011000101101001100011010011"
    );
    assert_eq!(format!("{:♥>+#8b}", a), "♥+0b1010");
}

#[test]
fn test_octal() {
    let a = BigInt::parse_bytes(b"A", 16).unwrap();
    let hello = BigInt::parse_bytes(b"-22405534230753963835153736737", 10).unwrap();

    assert_eq!(format!("{:o}", a), "12");
    assert_eq!(format!("{:o}", hello), "-22062554330674403566756233062041");
    assert_eq!(format!("{:♥>+#8o}", a), "♥♥♥+0o12");
}

#[test]
fn test_display() {
    let a = BigInt::parse_bytes(b"A", 16).unwrap();
    let hello = BigInt::parse_bytes(b"-22405534230753963835153736737", 10).unwrap();

    assert_eq!(format!("{}", a), "10");
    assert_eq!(format!("{}", hello), "-22405534230753963835153736737");
    assert_eq!(format!("{:♥>+#8}", a), "♥♥♥♥♥+10");
}

#[test]
fn test_neg() {
    assert!(-BigInt::new(Plus, vec![1, 1, 1]) == BigInt::new(Minus, vec![1, 1, 1]));
    assert!(-BigInt::new(Minus, vec![1, 1, 1]) == BigInt::new(Plus, vec![1, 1, 1]));
    let zero: BigInt = Zero::zero();
    assert_eq!(-&zero, zero);
}

#[test]
fn test_negative_shr() {
    assert_eq!(BigInt::from(-1) >> 1, BigInt::from(-1));
    assert_eq!(BigInt::from(-2) >> 1, BigInt::from(-1));
    assert_eq!(BigInt::from(-3) >> 1, BigInt::from(-2));
    assert_eq!(BigInt::from(-3) >> 2, BigInt::from(-1));
}

#[test]
fn test_iter_sum() {
    let result: BigInt = FromPrimitive::from_isize(-1234567).unwrap();
    let data: Vec<BigInt> = vec![
        FromPrimitive::from_i32(-1000000).unwrap(),
        FromPrimitive::from_i32(-200000).unwrap(),
        FromPrimitive::from_i32(-30000).unwrap(),
        FromPrimitive::from_i32(-4000).unwrap(),
        FromPrimitive::from_i32(-500).unwrap(),
        FromPrimitive::from_i32(-60).unwrap(),
        FromPrimitive::from_i32(-7).unwrap(),
    ];

    assert_eq!(result, data.iter().sum::<BigInt>());
    assert_eq!(result, data.into_iter().sum::<BigInt>());
}

#[test]
fn test_iter_product() {
    let data: Vec<BigInt> = vec![
        FromPrimitive::from_i32(1001).unwrap(),
        FromPrimitive::from_i32(-1002).unwrap(),
        FromPrimitive::from_i32(1003).unwrap(),
        FromPrimitive::from_i32(-1004).unwrap(),
        FromPrimitive::from_i32(1005).unwrap(),
    ];
    let result = data.get(0).unwrap()
        * data.get(1).unwrap()
        * data.get(2).unwrap()
        * data.get(3).unwrap()
        * data.get(4).unwrap();

    assert_eq!(result, data.iter().product::<BigInt>());
    assert_eq!(result, data.into_iter().product::<BigInt>());
}

#[test]
fn test_iter_sum_generic() {
    let result: BigInt = FromPrimitive::from_isize(-1234567).unwrap();
    let data = vec![-1000000, -200000, -30000, -4000, -500, -60, -7];

    assert_eq!(result, data.iter().sum::<BigInt>());
    assert_eq!(result, data.into_iter().sum::<BigInt>());
}

#[test]
fn test_iter_product_generic() {
    let data = vec![1001, -1002, 1003, -1004, 1005];
    let result = data[0].to_bigint().unwrap()
        * data[1].to_bigint().unwrap()
        * data[2].to_bigint().unwrap()
        * data[3].to_bigint().unwrap()
        * data[4].to_bigint().unwrap();

    assert_eq!(result, data.iter().product::<BigInt>());
    assert_eq!(result, data.into_iter().product::<BigInt>());
}

#[test]
fn test_pow() {
    let one = BigInt::from(1i32);
    let two = BigInt::from(2i32);
    let four = BigInt::from(4i32);
    let eight = BigInt::from(8i32);
    let minus_two = BigInt::from(-2i32);
    macro_rules! check {
        ($t:ty) => {
            assert_eq!(Pow::pow(&two, 0 as $t), one);
            assert_eq!(Pow::pow(&two, 1 as $t), two);
            assert_eq!(Pow::pow(&two, 2 as $t), four);
            assert_eq!(Pow::pow(&two, 3 as $t), eight);
            assert_eq!(Pow::pow(&two, &(3 as $t)), eight);
            assert_eq!(Pow::pow(&minus_two, 0 as $t), one, "-2^0");
            assert_eq!(Pow::pow(&minus_two, 1 as $t), minus_two, "-2^1");
            assert_eq!(Pow::pow(&minus_two, 2 as $t), four, "-2^2");
            assert_eq!(Pow::pow(&minus_two, 3 as $t), -&eight, "-2^3");
        };
    }
    check!(u8);
    check!(u16);
    check!(u32);
    check!(u64);
    check!(usize);

    let pow_1e10000 = BigInt::from(10u32).pow(10_000_u32);
    let manual_1e10000 = repeat(10u32).take(10_000).product::<BigInt>();
    assert!(manual_1e10000 == pow_1e10000);
}

#[test]
fn test_bit() {
    // 12 = (1100)_2
    assert!(!BigInt::from(0b1100u8).bit(0));
    assert!(!BigInt::from(0b1100u8).bit(1));
    assert!(BigInt::from(0b1100u8).bit(2));
    assert!(BigInt::from(0b1100u8).bit(3));
    assert!(!BigInt::from(0b1100u8).bit(4));
    assert!(!BigInt::from(0b1100u8).bit(200));
    assert!(!BigInt::from(0b1100u8).bit(u64::MAX));
    // -12 = (...110100)_2
    assert!(!BigInt::from(-12i8).bit(0));
    assert!(!BigInt::from(-12i8).bit(1));
    assert!(BigInt::from(-12i8).bit(2));
    assert!(!BigInt::from(-12i8).bit(3));
    assert!(BigInt::from(-12i8).bit(4));
    assert!(BigInt::from(-12i8).bit(200));
    assert!(BigInt::from(-12i8).bit(u64::MAX));
}

#[test]
fn test_set_bit() {
    let mut x: BigInt;

    // zero
    x = BigInt::zero();
    x.set_bit(200, true);
    assert_eq!(x, BigInt::one() << 200);
    x = BigInt::zero();
    x.set_bit(200, false);
    assert_eq!(x, BigInt::zero());

    // positive numbers
    x = BigInt::from_biguint(Plus, BigUint::one() << 200);
    x.set_bit(10, true);
    x.set_bit(200, false);
    assert_eq!(x, BigInt::one() << 10);
    x.set_bit(10, false);
    x.set_bit(5, false);
    assert_eq!(x, BigInt::zero());

    // negative numbers
    x = BigInt::from(-12i8);
    x.set_bit(200, true);
    assert_eq!(x, BigInt::from(-12i8));
    x.set_bit(200, false);
    assert_eq!(
        x,
        BigInt::from_biguint(Minus, BigUint::from(12u8) | (BigUint::one() << 200))
    );
    x.set_bit(6, false);
    assert_eq!(
        x,
        BigInt::from_biguint(Minus, BigUint::from(76u8) | (BigUint::one() << 200))
    );
    x.set_bit(6, true);
    assert_eq!(
        x,
        BigInt::from_biguint(Minus, BigUint::from(12u8) | (BigUint::one() << 200))
    );
    x.set_bit(200, true);
    assert_eq!(x, BigInt::from(-12i8));

    x = BigInt::from_biguint(Minus, BigUint::one() << 30);
    x.set_bit(10, true);
    assert_eq!(
        x,
        BigInt::from_biguint(Minus, (BigUint::one() << 30) - (BigUint::one() << 10))
    );

    x = BigInt::from_biguint(Minus, BigUint::one() << 200);
    x.set_bit(40, true);
    assert_eq!(
        x,
        BigInt::from_biguint(Minus, (BigUint::one() << 200) - (BigUint::one() << 40))
    );

    x = BigInt::from_biguint(Minus, (BigUint::one() << 200) | (BigUint::one() << 100));
    x.set_bit(100, false);
    assert_eq!(
        x,
        BigInt::from_biguint(Minus, (BigUint::one() << 200) | (BigUint::one() << 101))
    );

    x = BigInt::from_biguint(Minus, (BigUint::one() << 63) | (BigUint::one() << 62));
    x.set_bit(62, false);
    assert_eq!(x, BigInt::from_biguint(Minus, BigUint::one() << 64));

    x = BigInt::from_biguint(Minus, (BigUint::one() << 200) - BigUint::one());
    x.set_bit(0, false);
    assert_eq!(x, BigInt::from_biguint(Minus, BigUint::one() << 200));
}
