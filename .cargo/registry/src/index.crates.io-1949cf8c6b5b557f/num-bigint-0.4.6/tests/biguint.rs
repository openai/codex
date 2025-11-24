use num_bigint::Sign::Plus;
use num_bigint::{BigInt, ToBigInt};
use num_bigint::{BigUint, ToBigUint};
use num_integer::Integer;

use std::cmp::Ordering::{Equal, Greater, Less};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::iter::repeat;
use std::str::FromStr;
use std::{f32, f64};

use num_traits::{
    pow, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Euclid, FromBytes, FromPrimitive, Num,
    One, Pow, ToBytes, ToPrimitive, Zero,
};

mod consts;
use crate::consts::*;

#[macro_use]
mod macros;

#[test]
fn test_from_bytes_be() {
    fn check(s: &str, result: &str) {
        let b = BigUint::parse_bytes(result.as_bytes(), 10).unwrap();
        assert_eq!(BigUint::from_bytes_be(s.as_bytes()), b);
        assert_eq!(<BigUint as FromBytes>::from_be_bytes(s.as_bytes()), b);
    }
    check("A", "65");
    check("AA", "16705");
    check("AB", "16706");
    check("Hello world!", "22405534230753963835153736737");
    assert_eq!(BigUint::from_bytes_be(&[]), BigUint::zero());
}

#[test]
fn test_to_bytes_be() {
    fn check(s: &str, result: &str) {
        let b = BigUint::parse_bytes(result.as_bytes(), 10).unwrap();
        assert_eq!(b.to_bytes_be(), s.as_bytes());
        assert_eq!(<BigUint as ToBytes>::to_be_bytes(&b), s.as_bytes());
    }
    check("A", "65");
    check("AA", "16705");
    check("AB", "16706");
    check("Hello world!", "22405534230753963835153736737");
    let b: BigUint = Zero::zero();
    assert_eq!(b.to_bytes_be(), [0]);

    // Test with leading/trailing zero bytes and a full BigDigit of value 0
    let b = BigUint::from_str_radix("00010000000000000200", 16).unwrap();
    assert_eq!(b.to_bytes_be(), [1, 0, 0, 0, 0, 0, 0, 2, 0]);
}

#[test]
fn test_from_bytes_le() {
    fn check(s: &str, result: &str) {
        let b = BigUint::parse_bytes(result.as_bytes(), 10).unwrap();
        assert_eq!(BigUint::from_bytes_le(s.as_bytes()), b);
        assert_eq!(<BigUint as FromBytes>::from_le_bytes(s.as_bytes()), b);
    }
    check("A", "65");
    check("AA", "16705");
    check("BA", "16706");
    check("!dlrow olleH", "22405534230753963835153736737");
    assert_eq!(BigUint::from_bytes_le(&[]), BigUint::zero());
}

#[test]
fn test_to_bytes_le() {
    fn check(s: &str, result: &str) {
        let b = BigUint::parse_bytes(result.as_bytes(), 10).unwrap();
        assert_eq!(b.to_bytes_le(), s.as_bytes());
        assert_eq!(<BigUint as ToBytes>::to_le_bytes(&b), s.as_bytes());
    }
    check("A", "65");
    check("AA", "16705");
    check("BA", "16706");
    check("!dlrow olleH", "22405534230753963835153736737");
    let b: BigUint = Zero::zero();
    assert_eq!(b.to_bytes_le(), [0]);

    // Test with leading/trailing zero bytes and a full BigDigit of value 0
    let b = BigUint::from_str_radix("00010000000000000200", 16).unwrap();
    assert_eq!(b.to_bytes_le(), [0, 2, 0, 0, 0, 0, 0, 0, 1]);
}

#[test]
fn test_cmp() {
    let data: [&[_]; 7] = [&[], &[1], &[2], &[!0], &[0, 1], &[2, 1], &[1, 1, 1]];
    let data: Vec<BigUint> = data.iter().map(|v| BigUint::from_slice(*v)).collect();
    for (i, ni) in data.iter().enumerate() {
        for (j0, nj) in data[i..].iter().enumerate() {
            let j = j0 + i;
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
    use crate::hash;

    let a = BigUint::new(vec![]);
    let b = BigUint::new(vec![0]);
    let c = BigUint::new(vec![1]);
    let d = BigUint::new(vec![1, 0, 0, 0, 0, 0]);
    let e = BigUint::new(vec![0, 0, 0, 0, 0, 1]);
    assert!(hash(&a) == hash(&b));
    assert!(hash(&b) != hash(&c));
    assert!(hash(&c) == hash(&d));
    assert!(hash(&d) != hash(&e));
}

// LEFT, RIGHT, AND, OR, XOR
const BIT_TESTS: &[(&[u32], &[u32], &[u32], &[u32], &[u32])] = &[
    (&[], &[], &[], &[], &[]),
    (&[1, 0, 1], &[1, 1], &[1], &[1, 1, 1], &[0, 1, 1]),
    (&[1, 0, 1], &[0, 1, 1], &[0, 0, 1], &[1, 1, 1], &[1, 1]),
    (
        &[268, 482, 17],
        &[964, 54],
        &[260, 34],
        &[972, 502, 17],
        &[712, 468, 17],
    ),
];

#[test]
fn test_bitand() {
    for elm in BIT_TESTS {
        let (a_vec, b_vec, c_vec, _, _) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert_op!(a & b == c);
        assert_op!(b & a == c);
        assert_assign_op!(a &= b == c);
        assert_assign_op!(b &= a == c);
    }
}

#[test]
fn test_bitor() {
    for elm in BIT_TESTS {
        let (a_vec, b_vec, _, c_vec, _) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert_op!(a | b == c);
        assert_op!(b | a == c);
        assert_assign_op!(a |= b == c);
        assert_assign_op!(b |= a == c);
    }
}

#[test]
fn test_bitxor() {
    for elm in BIT_TESTS {
        let (a_vec, b_vec, _, _, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert_op!(a ^ b == c);
        assert_op!(b ^ a == c);
        assert_op!(a ^ c == b);
        assert_op!(c ^ a == b);
        assert_op!(b ^ c == a);
        assert_op!(c ^ b == a);
        assert_assign_op!(a ^= b == c);
        assert_assign_op!(b ^= a == c);
        assert_assign_op!(a ^= c == b);
        assert_assign_op!(c ^= a == b);
        assert_assign_op!(b ^= c == a);
        assert_assign_op!(c ^= b == a);
    }
}

#[test]
fn test_shl() {
    fn check(s: &str, shift: usize, ans: &str) {
        let opt_biguint = BigUint::from_str_radix(s, 16).ok();
        let mut bu_assign = opt_biguint.unwrap();
        let bu = (bu_assign.clone() << shift).to_str_radix(16);
        assert_eq!(bu, ans);
        bu_assign <<= shift;
        assert_eq!(bu_assign.to_str_radix(16), ans);
    }

    check("0", 3, "0");
    check("1", 3, "8");

    check(
        "1\
         0000\
         0000\
         0000\
         0001\
         0000\
         0000\
         0000\
         0001",
        3,
        "8\
         0000\
         0000\
         0000\
         0008\
         0000\
         0000\
         0000\
         0008",
    );
    check(
        "1\
         0000\
         0001\
         0000\
         0001",
        2,
        "4\
         0000\
         0004\
         0000\
         0004",
    );
    check(
        "1\
         0001\
         0001",
        1,
        "2\
         0002\
         0002",
    );

    check(
        "\
         4000\
         0000\
         0000\
         0000",
        3,
        "2\
         0000\
         0000\
         0000\
         0000",
    );
    check(
        "4000\
         0000",
        2,
        "1\
         0000\
         0000",
    );
    check(
        "4000",
        2,
        "1\
         0000",
    );

    check(
        "4000\
         0000\
         0000\
         0000",
        67,
        "2\
         0000\
         0000\
         0000\
         0000\
         0000\
         0000\
         0000\
         0000",
    );
    check(
        "4000\
         0000",
        35,
        "2\
         0000\
         0000\
         0000\
         0000",
    );
    check(
        "4000",
        19,
        "2\
         0000\
         0000",
    );

    check(
        "fedc\
         ba98\
         7654\
         3210\
         fedc\
         ba98\
         7654\
         3210",
        4,
        "f\
         edcb\
         a987\
         6543\
         210f\
         edcb\
         a987\
         6543\
         2100",
    );
    check(
        "88887777666655554444333322221111",
        16,
        "888877776666555544443333222211110000",
    );
}

#[test]
fn test_shr() {
    fn check(s: &str, shift: usize, ans: &str) {
        let opt_biguint = BigUint::from_str_radix(s, 16).ok();
        let mut bu_assign = opt_biguint.unwrap();
        let bu = (bu_assign.clone() >> shift).to_str_radix(16);
        assert_eq!(bu, ans);
        bu_assign >>= shift;
        assert_eq!(bu_assign.to_str_radix(16), ans);
    }

    check("0", 3, "0");
    check("f", 3, "1");

    check(
        "1\
         0000\
         0000\
         0000\
         0001\
         0000\
         0000\
         0000\
         0001",
        3,
        "2000\
         0000\
         0000\
         0000\
         2000\
         0000\
         0000\
         0000",
    );
    check(
        "1\
         0000\
         0001\
         0000\
         0001",
        2,
        "4000\
         0000\
         4000\
         0000",
    );
    check(
        "1\
         0001\
         0001",
        1,
        "8000\
         8000",
    );

    check(
        "2\
         0000\
         0000\
         0000\
         0001\
         0000\
         0000\
         0000\
         0001",
        67,
        "4000\
         0000\
         0000\
         0000",
    );
    check(
        "2\
         0000\
         0001\
         0000\
         0001",
        35,
        "4000\
         0000",
    );
    check(
        "2\
         0001\
         0001",
        19,
        "4000",
    );

    check(
        "1\
         0000\
         0000\
         0000\
         0000",
        1,
        "8000\
         0000\
         0000\
         0000",
    );
    check(
        "1\
         0000\
         0000",
        1,
        "8000\
         0000",
    );
    check(
        "1\
         0000",
        1,
        "8000",
    );
    check(
        "f\
         edcb\
         a987\
         6543\
         210f\
         edcb\
         a987\
         6543\
         2100",
        4,
        "fedc\
         ba98\
         7654\
         3210\
         fedc\
         ba98\
         7654\
         3210",
    );

    check(
        "888877776666555544443333222211110000",
        16,
        "88887777666655554444333322221111",
    );
}

// `DoubleBigDigit` size dependent
#[test]
fn test_convert_i64() {
    fn check(b1: BigUint, i: i64) {
        let b2: BigUint = FromPrimitive::from_i64(i).unwrap();
        assert_eq!(b1, b2);
        assert_eq!(b1.to_i64().unwrap(), i);
    }

    check(Zero::zero(), 0);
    check(One::one(), 1);
    check(i64::MAX.to_biguint().unwrap(), i64::MAX);

    check(BigUint::new(vec![]), 0);
    check(BigUint::new(vec![1]), 1);
    check(BigUint::new(vec![N1]), (1 << 32) - 1);
    check(BigUint::new(vec![0, 1]), 1 << 32);
    check(BigUint::new(vec![N1, N1 >> 1]), i64::MAX);

    assert_eq!(i64::MIN.to_biguint(), None);
    assert_eq!(BigUint::new(vec![N1, N1]).to_i64(), None);
    assert_eq!(BigUint::new(vec![0, 0, 1]).to_i64(), None);
    assert_eq!(BigUint::new(vec![N1, N1, N1]).to_i64(), None);
}

#[test]
fn test_convert_i128() {
    fn check(b1: BigUint, i: i128) {
        let b2: BigUint = FromPrimitive::from_i128(i).unwrap();
        assert_eq!(b1, b2);
        assert_eq!(b1.to_i128().unwrap(), i);
    }

    check(Zero::zero(), 0);
    check(One::one(), 1);
    check(i128::MAX.to_biguint().unwrap(), i128::MAX);

    check(BigUint::new(vec![]), 0);
    check(BigUint::new(vec![1]), 1);
    check(BigUint::new(vec![N1]), (1 << 32) - 1);
    check(BigUint::new(vec![0, 1]), 1 << 32);
    check(BigUint::new(vec![N1, N1, N1, N1 >> 1]), i128::MAX);

    assert_eq!(i128::MIN.to_biguint(), None);
    assert_eq!(BigUint::new(vec![N1, N1, N1, N1]).to_i128(), None);
    assert_eq!(BigUint::new(vec![0, 0, 0, 0, 1]).to_i128(), None);
    assert_eq!(BigUint::new(vec![N1, N1, N1, N1, N1]).to_i128(), None);
}

// `DoubleBigDigit` size dependent
#[test]
fn test_convert_u64() {
    fn check(b1: BigUint, u: u64) {
        let b2: BigUint = FromPrimitive::from_u64(u).unwrap();
        assert_eq!(b1, b2);
        assert_eq!(b1.to_u64().unwrap(), u);
    }

    check(Zero::zero(), 0);
    check(One::one(), 1);
    check(u64::MIN.to_biguint().unwrap(), u64::MIN);
    check(u64::MAX.to_biguint().unwrap(), u64::MAX);

    check(BigUint::new(vec![]), 0);
    check(BigUint::new(vec![1]), 1);
    check(BigUint::new(vec![N1]), (1 << 32) - 1);
    check(BigUint::new(vec![0, 1]), 1 << 32);
    check(BigUint::new(vec![N1, N1]), u64::MAX);

    assert_eq!(BigUint::new(vec![0, 0, 1]).to_u64(), None);
    assert_eq!(BigUint::new(vec![N1, N1, N1]).to_u64(), None);
}

#[test]
fn test_convert_u128() {
    fn check(b1: BigUint, u: u128) {
        let b2: BigUint = FromPrimitive::from_u128(u).unwrap();
        assert_eq!(b1, b2);
        assert_eq!(b1.to_u128().unwrap(), u);
    }

    check(Zero::zero(), 0);
    check(One::one(), 1);
    check(u128::MIN.to_biguint().unwrap(), u128::MIN);
    check(u128::MAX.to_biguint().unwrap(), u128::MAX);

    check(BigUint::new(vec![]), 0);
    check(BigUint::new(vec![1]), 1);
    check(BigUint::new(vec![N1]), (1 << 32) - 1);
    check(BigUint::new(vec![0, 1]), 1 << 32);
    check(BigUint::new(vec![N1, N1, N1, N1]), u128::MAX);

    assert_eq!(BigUint::new(vec![0, 0, 0, 0, 1]).to_u128(), None);
    assert_eq!(BigUint::new(vec![N1, N1, N1, N1, N1]).to_u128(), None);
}

#[test]
#[allow(clippy::float_cmp)]
fn test_convert_f32() {
    fn check(b1: &BigUint, f: f32) {
        let b2 = BigUint::from_f32(f).unwrap();
        assert_eq!(b1, &b2);
        assert_eq!(b1.to_f32().unwrap(), f);
    }

    check(&BigUint::zero(), 0.0);
    check(&BigUint::one(), 1.0);
    check(&BigUint::from(u16::MAX), pow(2.0_f32, 16) - 1.0);
    check(&BigUint::from(1u64 << 32), pow(2.0_f32, 32));
    check(&BigUint::from_slice(&[0, 0, 1]), pow(2.0_f32, 64));
    check(
        &((BigUint::one() << 100) + (BigUint::one() << 123)),
        pow(2.0_f32, 100) + pow(2.0_f32, 123),
    );
    check(&(BigUint::one() << 127), pow(2.0_f32, 127));
    check(&(BigUint::from((1u64 << 24) - 1) << (128 - 24)), f32::MAX);

    // keeping all 24 digits with the bits at different offsets to the BigDigits
    let x: u32 = 0b00000000101111011111011011011101;
    let mut f = x as f32;
    let mut b = BigUint::from(x);
    for _ in 0..64 {
        check(&b, f);
        f *= 2.0;
        b <<= 1;
    }

    // this number when rounded to f64 then f32 isn't the same as when rounded straight to f32
    let n: u64 = 0b0000000000111111111111111111111111011111111111111111111111111111;
    assert!((n as f64) as f32 != n as f32);
    assert_eq!(BigUint::from(n).to_f32(), Some(n as f32));

    // test rounding up with the bits at different offsets to the BigDigits
    let mut f = ((1u64 << 25) - 1) as f32;
    let mut b = BigUint::from(1u64 << 25);
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
    assert_eq!(BigUint::from_f32(-1.0), None);
    assert_eq!(BigUint::from_f32(-0.99999), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f32(-0.5), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f32(-0.0), Some(BigUint::zero()));
    assert_eq!(
        BigUint::from_f32(f32::MIN_POSITIVE / 2.0),
        Some(BigUint::zero())
    );
    assert_eq!(BigUint::from_f32(f32::MIN_POSITIVE), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f32(0.5), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f32(0.99999), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f32(f32::consts::E), Some(BigUint::from(2u32)));
    assert_eq!(
        BigUint::from_f32(f32::consts::PI),
        Some(BigUint::from(3u32))
    );

    // special float values
    assert_eq!(BigUint::from_f32(f32::NAN), None);
    assert_eq!(BigUint::from_f32(f32::INFINITY), None);
    assert_eq!(BigUint::from_f32(f32::NEG_INFINITY), None);
    assert_eq!(BigUint::from_f32(f32::MIN), None);

    // largest BigUint that will round to a finite f32 value
    let big_num = (BigUint::one() << 128u8) - 1u8 - (BigUint::one() << (128u8 - 25));
    assert_eq!(big_num.to_f32(), Some(f32::MAX));
    assert_eq!((big_num + 1u8).to_f32(), Some(f32::INFINITY));

    assert_eq!(
        ((BigUint::one() << 128u8) - 1u8).to_f32(),
        Some(f32::INFINITY)
    );
    assert_eq!((BigUint::one() << 128u8).to_f32(), Some(f32::INFINITY));
}

#[test]
#[allow(clippy::float_cmp)]
fn test_convert_f64() {
    fn check(b1: &BigUint, f: f64) {
        let b2 = BigUint::from_f64(f).unwrap();
        assert_eq!(b1, &b2);
        assert_eq!(b1.to_f64().unwrap(), f);
    }

    check(&BigUint::zero(), 0.0);
    check(&BigUint::one(), 1.0);
    check(&BigUint::from(u32::MAX), pow(2.0_f64, 32) - 1.0);
    check(&BigUint::from(1u64 << 32), pow(2.0_f64, 32));
    check(&BigUint::from_slice(&[0, 0, 1]), pow(2.0_f64, 64));
    check(
        &((BigUint::one() << 100) + (BigUint::one() << 152)),
        pow(2.0_f64, 100) + pow(2.0_f64, 152),
    );
    check(&(BigUint::one() << 1023), pow(2.0_f64, 1023));
    check(&(BigUint::from((1u64 << 53) - 1) << (1024 - 53)), f64::MAX);

    // keeping all 53 digits with the bits at different offsets to the BigDigits
    let x: u64 = 0b0000000000011110111110110111111101110111101111011111011011011101;
    let mut f = x as f64;
    let mut b = BigUint::from(x);
    for _ in 0..128 {
        check(&b, f);
        f *= 2.0;
        b <<= 1;
    }

    // test rounding up with the bits at different offsets to the BigDigits
    let mut f = ((1u64 << 54) - 1) as f64;
    let mut b = BigUint::from(1u64 << 54);
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
    assert_eq!(BigUint::from_f64(-1.0), None);
    assert_eq!(BigUint::from_f64(-0.99999), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f64(-0.5), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f64(-0.0), Some(BigUint::zero()));
    assert_eq!(
        BigUint::from_f64(f64::MIN_POSITIVE / 2.0),
        Some(BigUint::zero())
    );
    assert_eq!(BigUint::from_f64(f64::MIN_POSITIVE), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f64(0.5), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f64(0.99999), Some(BigUint::zero()));
    assert_eq!(BigUint::from_f64(f64::consts::E), Some(BigUint::from(2u32)));
    assert_eq!(
        BigUint::from_f64(f64::consts::PI),
        Some(BigUint::from(3u32))
    );

    // special float values
    assert_eq!(BigUint::from_f64(f64::NAN), None);
    assert_eq!(BigUint::from_f64(f64::INFINITY), None);
    assert_eq!(BigUint::from_f64(f64::NEG_INFINITY), None);
    assert_eq!(BigUint::from_f64(f64::MIN), None);

    // largest BigUint that will round to a finite f64 value
    let big_num = (BigUint::one() << 1024u16) - 1u8 - (BigUint::one() << (1024u16 - 54));
    assert_eq!(big_num.to_f64(), Some(f64::MAX));
    assert_eq!((big_num + 1u8).to_f64(), Some(f64::INFINITY));

    assert_eq!(
        ((BigUint::one() << 1024u16) - 1u8).to_f64(),
        Some(f64::INFINITY)
    );
    assert_eq!((BigUint::one() << 1024u16).to_f64(), Some(f64::INFINITY));
}

#[test]
fn test_convert_to_bigint() {
    fn check(n: BigUint, ans: BigInt) {
        assert_eq!(n.to_bigint().unwrap(), ans);
        assert_eq!(n.to_bigint().unwrap().to_biguint().unwrap(), n);
    }
    check(Zero::zero(), Zero::zero());
    check(
        BigUint::new(vec![1, 2, 3]),
        BigInt::from_biguint(Plus, BigUint::new(vec![1, 2, 3])),
    );
}

#[test]
fn test_convert_from_uint() {
    macro_rules! check {
        ($ty:ident, $max:expr) => {
            assert_eq!(BigUint::from($ty::zero()), BigUint::zero());
            assert_eq!(BigUint::from($ty::one()), BigUint::one());
            assert_eq!(BigUint::from($ty::MAX - $ty::one()), $max - BigUint::one());
            assert_eq!(BigUint::from($ty::MAX), $max);
        };
    }

    check!(u8, BigUint::from_slice(&[u8::MAX as u32]));
    check!(u16, BigUint::from_slice(&[u16::MAX as u32]));
    check!(u32, BigUint::from_slice(&[u32::MAX]));
    check!(u64, BigUint::from_slice(&[u32::MAX, u32::MAX]));
    check!(
        u128,
        BigUint::from_slice(&[u32::MAX, u32::MAX, u32::MAX, u32::MAX])
    );
    check!(usize, BigUint::from(usize::MAX as u64));
}

#[test]
fn test_add() {
    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert_op!(a + b == c);
        assert_op!(b + a == c);
        assert_assign_op!(a += b == c);
        assert_assign_op!(b += a == c);
    }
}

#[test]
fn test_sub() {
    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert_op!(c - a == b);
        assert_op!(c - b == a);
        assert_assign_op!(c -= a == b);
        assert_assign_op!(c -= b == a);
    }
}

#[test]
#[should_panic]
fn test_sub_fail_on_underflow() {
    let (a, b): (BigUint, BigUint) = (Zero::zero(), One::one());
    let _ = a - b;
}

#[test]
fn test_mul() {
    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert_op!(a * b == c);
        assert_op!(b * a == c);
        assert_assign_op!(a *= b == c);
        assert_assign_op!(b *= a == c);
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);
        let d = BigUint::from_slice(d_vec);

        assert!(a == &b * &c + &d);
        assert!(a == &c * &b + &d);
    }
}

#[test]
fn test_div_rem() {
    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        if !a.is_zero() {
            assert_op!(c / a == b);
            assert_op!(c % a == BigUint::zero());
            assert_assign_op!(c /= a == b);
            assert_assign_op!(c %= a == BigUint::zero());
            assert_eq!(c.div_rem(&a), (b.clone(), BigUint::zero()));
        }
        if !b.is_zero() {
            assert_op!(c / b == a);
            assert_op!(c % b == BigUint::zero());
            assert_assign_op!(c /= b == a);
            assert_assign_op!(c %= b == BigUint::zero());
            assert_eq!(c.div_rem(&b), (a.clone(), BigUint::zero()));
        }
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);
        let d = BigUint::from_slice(d_vec);

        if !b.is_zero() {
            assert_op!(a / b == c);
            assert_op!(a % b == d);
            assert_assign_op!(a /= b == c);
            assert_assign_op!(a %= b == d);
            assert!(a.div_rem(&b) == (c, d));
        }
    }
}

#[test]
fn test_div_rem_big_multiple() {
    let a = BigUint::from(3u32).pow(100u32);
    let a2 = &a * &a;

    let (div, rem) = a2.div_rem(&a);
    assert_eq!(div, a);
    assert!(rem.is_zero());

    let (div, rem) = (&a2 - 1u32).div_rem(&a);
    assert_eq!(div, &a - 1u32);
    assert_eq!(rem, &a - 1u32);
}

#[test]
fn test_div_ceil() {
    fn check(a: &BigUint, b: &BigUint, d: &BigUint, m: &BigUint) {
        if m.is_zero() {
            assert_eq!(a.div_ceil(b), *d);
        } else {
            assert_eq!(a.div_ceil(b), d + 1u32);
        }
    }

    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        if !a.is_zero() {
            check(&c, &a, &b, &Zero::zero());
        }
        if !b.is_zero() {
            check(&c, &b, &a, &Zero::zero());
        }
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);
        let d = BigUint::from_slice(d_vec);

        if !b.is_zero() {
            check(&a, &b, &c, &d);
        }
    }
}

#[test]
fn test_div_rem_euclid() {
    fn check(a: &BigUint, b: &BigUint, d: &BigUint, m: &BigUint) {
        assert_eq!(a.div_euclid(b), *d);
        assert_eq!(a.rem_euclid(b), *m);
    }

    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        if !a.is_zero() {
            check(&c, &a, &b, &Zero::zero());
        }
        if !b.is_zero() {
            check(&c, &b, &a, &Zero::zero());
        }
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);
        let d = BigUint::from_slice(d_vec);

        if !b.is_zero() {
            check(&a, &b, &c, &d);
        }
    }
}

#[test]
fn test_checked_add() {
    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert!(a.checked_add(&b).unwrap() == c);
        assert!(b.checked_add(&a).unwrap() == c);
    }
}

#[test]
fn test_checked_sub() {
    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert!(c.checked_sub(&a).unwrap() == b);
        assert!(c.checked_sub(&b).unwrap() == a);

        if a > c {
            assert!(a.checked_sub(&c).is_none());
        }
        if b > c {
            assert!(b.checked_sub(&c).is_none());
        }
    }
}

#[test]
fn test_checked_mul() {
    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        assert!(a.checked_mul(&b).unwrap() == c);
        assert!(b.checked_mul(&a).unwrap() == c);
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);
        let d = BigUint::from_slice(d_vec);

        assert!(a == b.checked_mul(&c).unwrap() + &d);
        assert!(a == c.checked_mul(&b).unwrap() + &d);
    }
}

#[test]
fn test_mul_overflow() {
    // Test for issue #187 - overflow due to mac3 incorrectly sizing temporary
    let s = "5311379928167670986895882065524686273295931177270319231994441382\
             0040355986085224273916250223263671004753755210595137000079652876\
             0829212940754539968588340162273730474622005920097370111";
    let a: BigUint = s.parse().unwrap();
    let b = a.clone();
    let _ = a.checked_mul(&b);
}

#[test]
fn test_mul_overflow_2() {
    // Try a bunch of sizes that are right on the edge of multiplication length
    // overflow, where (x * x).data.len() == 2 * x.data.len() + 1.
    for i in 1u8..20 {
        let bits = 1u32 << i;
        let x = (BigUint::one() << bits) - 1u32;
        let x2 = (BigUint::one() << (2 * bits)) - &x - &x - 1u32;
        assert_eq!(&x * &x, x2);
    }
}

#[test]
fn test_checked_div() {
    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        if !a.is_zero() {
            assert!(c.checked_div(&a).unwrap() == b);
        }
        if !b.is_zero() {
            assert!(c.checked_div(&b).unwrap() == a);
        }

        assert!(c.checked_div(&Zero::zero()).is_none());
    }
}

#[test]
fn test_gcd() {
    fn check(a: usize, b: usize, c: usize) {
        let big_a: BigUint = FromPrimitive::from_usize(a).unwrap();
        let big_b: BigUint = FromPrimitive::from_usize(b).unwrap();
        let big_c: BigUint = FromPrimitive::from_usize(c).unwrap();

        assert_eq!(big_a.gcd(&big_b), big_c);
        assert_eq!(big_a.gcd_lcm(&big_b).0, big_c);
    }

    check(10, 2, 2);
    check(10, 3, 1);
    check(0, 3, 3);
    check(3, 3, 3);
    check(56, 42, 14);
}

#[test]
fn test_lcm() {
    fn check(a: usize, b: usize, c: usize) {
        let big_a: BigUint = FromPrimitive::from_usize(a).unwrap();
        let big_b: BigUint = FromPrimitive::from_usize(b).unwrap();
        let big_c: BigUint = FromPrimitive::from_usize(c).unwrap();

        assert_eq!(big_a.lcm(&big_b), big_c);
        assert_eq!(big_a.gcd_lcm(&big_b).1, big_c);
    }

    check(0, 0, 0);
    check(1, 0, 0);
    check(0, 1, 0);
    check(1, 1, 1);
    check(8, 9, 72);
    check(11, 5, 55);
    check(99, 17, 1683);
}

#[test]
fn test_is_multiple_of() {
    assert!(BigUint::from(0u32).is_multiple_of(&BigUint::from(0u32)));
    assert!(BigUint::from(6u32).is_multiple_of(&BigUint::from(6u32)));
    assert!(BigUint::from(6u32).is_multiple_of(&BigUint::from(3u32)));
    assert!(BigUint::from(6u32).is_multiple_of(&BigUint::from(1u32)));

    assert!(!BigUint::from(42u32).is_multiple_of(&BigUint::from(5u32)));
    assert!(!BigUint::from(5u32).is_multiple_of(&BigUint::from(3u32)));
    assert!(!BigUint::from(42u32).is_multiple_of(&BigUint::from(0u32)));
}

#[test]
fn test_next_multiple_of() {
    assert_eq!(
        BigUint::from(16u32).next_multiple_of(&BigUint::from(8u32)),
        BigUint::from(16u32)
    );
    assert_eq!(
        BigUint::from(23u32).next_multiple_of(&BigUint::from(8u32)),
        BigUint::from(24u32)
    );
}

#[test]
fn test_prev_multiple_of() {
    assert_eq!(
        BigUint::from(16u32).prev_multiple_of(&BigUint::from(8u32)),
        BigUint::from(16u32)
    );
    assert_eq!(
        BigUint::from(23u32).prev_multiple_of(&BigUint::from(8u32)),
        BigUint::from(16u32)
    );
}

#[test]
fn test_is_even() {
    let one: BigUint = FromStr::from_str("1").unwrap();
    let two: BigUint = FromStr::from_str("2").unwrap();
    let thousand: BigUint = FromStr::from_str("1000").unwrap();
    let big: BigUint = FromStr::from_str("1000000000000000000000").unwrap();
    let bigger: BigUint = FromStr::from_str("1000000000000000000001").unwrap();
    assert!(one.is_odd());
    assert!(two.is_even());
    assert!(thousand.is_even());
    assert!(big.is_even());
    assert!(bigger.is_odd());
    assert!((&one << 64u8).is_even());
    assert!(((&one << 64u8) + one).is_odd());
}

fn to_str_pairs() -> Vec<(BigUint, Vec<(u32, String)>)> {
    let bits = 32;
    vec![
        (
            Zero::zero(),
            vec![(2, "0".to_string()), (3, "0".to_string())],
        ),
        (
            BigUint::from_slice(&[0xff]),
            vec![
                (2, "11111111".to_string()),
                (3, "100110".to_string()),
                (4, "3333".to_string()),
                (5, "2010".to_string()),
                (6, "1103".to_string()),
                (7, "513".to_string()),
                (8, "377".to_string()),
                (9, "313".to_string()),
                (10, "255".to_string()),
                (11, "212".to_string()),
                (12, "193".to_string()),
                (13, "168".to_string()),
                (14, "143".to_string()),
                (15, "120".to_string()),
                (16, "ff".to_string()),
            ],
        ),
        (
            BigUint::from_slice(&[0xfff]),
            vec![
                (2, "111111111111".to_string()),
                (4, "333333".to_string()),
                (16, "fff".to_string()),
            ],
        ),
        (
            BigUint::from_slice(&[1, 2]),
            vec![
                (
                    2,
                    format!("10{}1", repeat("0").take(bits - 1).collect::<String>()),
                ),
                (
                    4,
                    format!("2{}1", repeat("0").take(bits / 2 - 1).collect::<String>()),
                ),
                (
                    10,
                    match bits {
                        64 => "36893488147419103233".to_string(),
                        32 => "8589934593".to_string(),
                        16 => "131073".to_string(),
                        _ => panic!(),
                    },
                ),
                (
                    16,
                    format!("2{}1", repeat("0").take(bits / 4 - 1).collect::<String>()),
                ),
            ],
        ),
        (
            BigUint::from_slice(&[1, 2, 3]),
            vec![
                (
                    2,
                    format!(
                        "11{}10{}1",
                        repeat("0").take(bits - 2).collect::<String>(),
                        repeat("0").take(bits - 1).collect::<String>()
                    ),
                ),
                (
                    4,
                    format!(
                        "3{}2{}1",
                        repeat("0").take(bits / 2 - 1).collect::<String>(),
                        repeat("0").take(bits / 2 - 1).collect::<String>()
                    ),
                ),
                (
                    8,
                    match bits {
                        64 => "14000000000000000000004000000000000000000001".to_string(),
                        32 => "6000000000100000000001".to_string(),
                        16 => "140000400001".to_string(),
                        _ => panic!(),
                    },
                ),
                (
                    10,
                    match bits {
                        64 => "1020847100762815390427017310442723737601".to_string(),
                        32 => "55340232229718589441".to_string(),
                        16 => "12885032961".to_string(),
                        _ => panic!(),
                    },
                ),
                (
                    16,
                    format!(
                        "3{}2{}1",
                        repeat("0").take(bits / 4 - 1).collect::<String>(),
                        repeat("0").take(bits / 4 - 1).collect::<String>()
                    ),
                ),
            ],
        ),
    ]
}

#[test]
fn test_to_str_radix() {
    let r = to_str_pairs();
    for num_pair in r.iter() {
        let &(ref n, ref rs) = num_pair;
        for str_pair in rs.iter() {
            let &(ref radix, ref str) = str_pair;
            assert_eq!(n.to_str_radix(*radix), *str);
        }
    }
}

#[test]
fn test_from_and_to_radix() {
    const GROUND_TRUTH: &[(&[u8], u32, &[u8])] = &[
        (b"0", 42, &[0]),
        (
            b"ffffeeffbb",
            2,
            &[
                1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1,
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            ],
        ),
        (
            b"ffffeeffbb",
            3,
            &[
                2, 2, 1, 1, 2, 1, 1, 2, 0, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 1, 0, 0, 2, 2, 0, 1,
            ],
        ),
        (
            b"ffffeeffbb",
            4,
            &[3, 2, 3, 2, 3, 3, 3, 3, 2, 3, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3],
        ),
        (
            b"ffffeeffbb",
            5,
            &[0, 4, 3, 3, 1, 4, 2, 4, 1, 4, 4, 2, 3, 0, 0, 1, 2, 1],
        ),
        (
            b"ffffeeffbb",
            6,
            &[5, 5, 4, 5, 5, 0, 0, 1, 2, 5, 3, 0, 1, 0, 2, 2],
        ),
        (
            b"ffffeeffbb",
            7,
            &[4, 2, 3, 6, 0, 1, 6, 1, 6, 2, 0, 3, 2, 4, 1],
        ),
        (
            b"ffffeeffbb",
            8,
            &[3, 7, 6, 7, 7, 5, 3, 7, 7, 7, 7, 7, 7, 1],
        ),
        (b"ffffeeffbb", 9, &[8, 4, 5, 7, 0, 0, 3, 2, 0, 3, 0, 8, 3]),
        (b"ffffeeffbb", 10, &[5, 9, 5, 3, 1, 5, 0, 1, 5, 9, 9, 0, 1]),
        (b"ffffeeffbb", 11, &[10, 7, 6, 5, 2, 0, 3, 3, 3, 4, 9, 3]),
        (b"ffffeeffbb", 12, &[11, 8, 5, 10, 1, 10, 3, 1, 1, 9, 5, 1]),
        (b"ffffeeffbb", 13, &[0, 5, 7, 4, 6, 5, 6, 11, 8, 12, 7]),
        (b"ffffeeffbb", 14, &[11, 4, 4, 11, 8, 4, 6, 0, 3, 11, 3]),
        (b"ffffeeffbb", 15, &[5, 11, 13, 2, 1, 10, 2, 0, 9, 13, 1]),
        (b"ffffeeffbb", 16, &[11, 11, 15, 15, 14, 14, 15, 15, 15, 15]),
        (b"ffffeeffbb", 17, &[0, 2, 14, 12, 2, 14, 8, 10, 4, 9]),
        (b"ffffeeffbb", 18, &[17, 15, 5, 13, 10, 16, 16, 13, 9, 5]),
        (b"ffffeeffbb", 19, &[14, 13, 2, 8, 9, 0, 1, 14, 7, 3]),
        (b"ffffeeffbb", 20, &[15, 19, 3, 14, 0, 17, 19, 18, 2, 2]),
        (b"ffffeeffbb", 21, &[11, 5, 4, 13, 5, 18, 9, 1, 8, 1]),
        (b"ffffeeffbb", 22, &[21, 3, 7, 21, 15, 12, 17, 0, 20]),
        (b"ffffeeffbb", 23, &[21, 21, 6, 9, 10, 7, 21, 0, 14]),
        (b"ffffeeffbb", 24, &[11, 10, 19, 14, 22, 11, 17, 23, 9]),
        (b"ffffeeffbb", 25, &[20, 18, 21, 22, 21, 14, 3, 5, 7]),
        (b"ffffeeffbb", 26, &[13, 15, 24, 11, 17, 6, 23, 6, 5]),
        (b"ffffeeffbb", 27, &[17, 16, 7, 0, 21, 0, 3, 24, 3]),
        (b"ffffeeffbb", 28, &[11, 16, 11, 15, 14, 18, 13, 25, 2]),
        (b"ffffeeffbb", 29, &[6, 8, 7, 19, 14, 13, 21, 5, 2]),
        (b"ffffeeffbb", 30, &[5, 13, 18, 11, 10, 7, 8, 20, 1]),
        (b"ffffeeffbb", 31, &[22, 26, 15, 19, 8, 27, 29, 8, 1]),
        (b"ffffeeffbb", 32, &[27, 29, 31, 29, 30, 31, 31, 31]),
        (b"ffffeeffbb", 33, &[32, 20, 27, 12, 1, 12, 26, 25]),
        (b"ffffeeffbb", 34, &[17, 9, 16, 33, 13, 25, 31, 20]),
        (b"ffffeeffbb", 35, &[25, 32, 2, 25, 11, 4, 3, 17]),
        (b"ffffeeffbb", 36, &[35, 34, 5, 6, 32, 3, 1, 14]),
        (b"ffffeeffbb", 37, &[16, 21, 18, 4, 33, 19, 21, 11]),
        (b"ffffeeffbb", 38, &[33, 25, 19, 29, 20, 6, 23, 9]),
        (b"ffffeeffbb", 39, &[26, 27, 29, 23, 16, 18, 0, 8]),
        (b"ffffeeffbb", 40, &[35, 39, 30, 11, 16, 17, 28, 6]),
        (b"ffffeeffbb", 41, &[36, 30, 9, 18, 12, 19, 26, 5]),
        (b"ffffeeffbb", 42, &[11, 34, 37, 27, 1, 13, 32, 4]),
        (b"ffffeeffbb", 43, &[3, 24, 11, 2, 10, 40, 1, 4]),
        (b"ffffeeffbb", 44, &[43, 12, 40, 32, 3, 23, 19, 3]),
        (b"ffffeeffbb", 45, &[35, 38, 44, 18, 22, 18, 42, 2]),
        (b"ffffeeffbb", 46, &[21, 45, 18, 41, 17, 2, 24, 2]),
        (b"ffffeeffbb", 47, &[37, 37, 11, 12, 6, 0, 8, 2]),
        (b"ffffeeffbb", 48, &[11, 41, 40, 43, 5, 43, 41, 1]),
        (b"ffffeeffbb", 49, &[18, 45, 7, 13, 20, 21, 30, 1]),
        (b"ffffeeffbb", 50, &[45, 21, 5, 34, 21, 18, 20, 1]),
        (b"ffffeeffbb", 51, &[17, 6, 26, 22, 38, 24, 11, 1]),
        (b"ffffeeffbb", 52, &[39, 33, 38, 30, 46, 31, 3, 1]),
        (b"ffffeeffbb", 53, &[31, 7, 44, 23, 9, 32, 49]),
        (b"ffffeeffbb", 54, &[17, 35, 8, 37, 31, 18, 44]),
        (b"ffffeeffbb", 55, &[10, 52, 9, 48, 36, 39, 39]),
        (b"ffffeeffbb", 56, &[11, 50, 51, 22, 25, 36, 35]),
        (b"ffffeeffbb", 57, &[14, 55, 12, 43, 20, 3, 32]),
        (b"ffffeeffbb", 58, &[35, 18, 45, 56, 9, 51, 28]),
        (b"ffffeeffbb", 59, &[51, 28, 20, 26, 55, 3, 26]),
        (b"ffffeeffbb", 60, &[35, 6, 27, 46, 58, 33, 23]),
        (b"ffffeeffbb", 61, &[58, 7, 6, 54, 49, 20, 21]),
        (b"ffffeeffbb", 62, &[53, 59, 3, 14, 10, 22, 19]),
        (b"ffffeeffbb", 63, &[53, 50, 23, 4, 56, 36, 17]),
        (b"ffffeeffbb", 64, &[59, 62, 47, 59, 63, 63, 15]),
        (b"ffffeeffbb", 65, &[0, 53, 39, 4, 40, 37, 14]),
        (b"ffffeeffbb", 66, &[65, 59, 39, 1, 64, 19, 13]),
        (b"ffffeeffbb", 67, &[35, 14, 19, 16, 25, 10, 12]),
        (b"ffffeeffbb", 68, &[51, 38, 63, 50, 15, 8, 11]),
        (b"ffffeeffbb", 69, &[44, 45, 18, 58, 68, 12, 10]),
        (b"ffffeeffbb", 70, &[25, 51, 0, 60, 13, 24, 9]),
        (b"ffffeeffbb", 71, &[54, 30, 9, 65, 28, 41, 8]),
        (b"ffffeeffbb", 72, &[35, 35, 55, 54, 17, 64, 7]),
        (b"ffffeeffbb", 73, &[34, 4, 48, 40, 27, 19, 7]),
        (b"ffffeeffbb", 74, &[53, 47, 4, 56, 36, 51, 6]),
        (b"ffffeeffbb", 75, &[20, 56, 10, 72, 24, 13, 6]),
        (b"ffffeeffbb", 76, &[71, 31, 52, 60, 48, 53, 5]),
        (b"ffffeeffbb", 77, &[32, 73, 14, 63, 15, 21, 5]),
        (b"ffffeeffbb", 78, &[65, 13, 17, 32, 64, 68, 4]),
        (b"ffffeeffbb", 79, &[37, 56, 2, 56, 25, 41, 4]),
        (b"ffffeeffbb", 80, &[75, 59, 37, 41, 43, 15, 4]),
        (b"ffffeeffbb", 81, &[44, 68, 0, 21, 27, 72, 3]),
        (b"ffffeeffbb", 82, &[77, 35, 2, 74, 46, 50, 3]),
        (b"ffffeeffbb", 83, &[52, 51, 19, 76, 10, 30, 3]),
        (b"ffffeeffbb", 84, &[11, 80, 19, 19, 76, 10, 3]),
        (b"ffffeeffbb", 85, &[0, 82, 20, 14, 68, 77, 2]),
        (b"ffffeeffbb", 86, &[3, 12, 78, 37, 62, 61, 2]),
        (b"ffffeeffbb", 87, &[35, 12, 20, 8, 52, 46, 2]),
        (b"ffffeeffbb", 88, &[43, 6, 54, 42, 30, 32, 2]),
        (b"ffffeeffbb", 89, &[49, 52, 85, 21, 80, 18, 2]),
        (b"ffffeeffbb", 90, &[35, 64, 78, 24, 18, 6, 2]),
        (b"ffffeeffbb", 91, &[39, 17, 83, 63, 17, 85, 1]),
        (b"ffffeeffbb", 92, &[67, 22, 85, 79, 75, 74, 1]),
        (b"ffffeeffbb", 93, &[53, 60, 39, 29, 4, 65, 1]),
        (b"ffffeeffbb", 94, &[37, 89, 2, 72, 76, 55, 1]),
        (b"ffffeeffbb", 95, &[90, 74, 89, 9, 9, 47, 1]),
        (b"ffffeeffbb", 96, &[59, 20, 46, 35, 81, 38, 1]),
        (b"ffffeeffbb", 97, &[94, 87, 60, 71, 3, 31, 1]),
        (b"ffffeeffbb", 98, &[67, 22, 63, 50, 62, 23, 1]),
        (b"ffffeeffbb", 99, &[98, 6, 69, 12, 61, 16, 1]),
        (b"ffffeeffbb", 100, &[95, 35, 51, 10, 95, 9, 1]),
        (b"ffffeeffbb", 101, &[87, 27, 7, 8, 62, 3, 1]),
        (b"ffffeeffbb", 102, &[17, 3, 32, 79, 59, 99]),
        (b"ffffeeffbb", 103, &[30, 22, 90, 0, 87, 94]),
        (b"ffffeeffbb", 104, &[91, 68, 87, 68, 38, 90]),
        (b"ffffeeffbb", 105, &[95, 80, 54, 73, 15, 86]),
        (b"ffffeeffbb", 106, &[31, 30, 24, 16, 17, 82]),
        (b"ffffeeffbb", 107, &[51, 50, 10, 12, 42, 78]),
        (b"ffffeeffbb", 108, &[71, 71, 96, 78, 89, 74]),
        (b"ffffeeffbb", 109, &[33, 18, 93, 22, 50, 71]),
        (b"ffffeeffbb", 110, &[65, 53, 57, 88, 29, 68]),
        (b"ffffeeffbb", 111, &[53, 93, 67, 90, 27, 65]),
        (b"ffffeeffbb", 112, &[11, 109, 96, 65, 43, 62]),
        (b"ffffeeffbb", 113, &[27, 23, 106, 56, 76, 59]),
        (b"ffffeeffbb", 114, &[71, 84, 31, 112, 11, 57]),
        (b"ffffeeffbb", 115, &[90, 22, 1, 56, 76, 54]),
        (b"ffffeeffbb", 116, &[35, 38, 98, 57, 40, 52]),
        (b"ffffeeffbb", 117, &[26, 113, 115, 62, 17, 50]),
        (b"ffffeeffbb", 118, &[51, 14, 5, 18, 7, 48]),
        (b"ffffeeffbb", 119, &[102, 31, 110, 108, 8, 46]),
        (b"ffffeeffbb", 120, &[35, 93, 96, 50, 22, 44]),
        (b"ffffeeffbb", 121, &[87, 61, 2, 36, 47, 42]),
        (b"ffffeeffbb", 122, &[119, 64, 1, 22, 83, 40]),
        (b"ffffeeffbb", 123, &[77, 119, 32, 90, 6, 39]),
        (b"ffffeeffbb", 124, &[115, 122, 31, 79, 62, 37]),
        (b"ffffeeffbb", 125, &[95, 108, 47, 74, 3, 36]),
        (b"ffffeeffbb", 126, &[53, 25, 116, 39, 78, 34]),
        (b"ffffeeffbb", 127, &[22, 23, 125, 67, 35, 33]),
        (b"ffffeeffbb", 128, &[59, 127, 59, 127, 127, 31]),
        (b"ffffeeffbb", 129, &[89, 36, 1, 59, 100, 30]),
        (b"ffffeeffbb", 130, &[65, 91, 123, 89, 79, 29]),
        (b"ffffeeffbb", 131, &[58, 72, 39, 63, 65, 28]),
        (b"ffffeeffbb", 132, &[131, 62, 92, 82, 57, 27]),
        (b"ffffeeffbb", 133, &[109, 31, 51, 123, 55, 26]),
        (b"ffffeeffbb", 134, &[35, 74, 21, 27, 60, 25]),
        (b"ffffeeffbb", 135, &[125, 132, 49, 37, 70, 24]),
        (b"ffffeeffbb", 136, &[51, 121, 117, 133, 85, 23]),
        (b"ffffeeffbb", 137, &[113, 60, 135, 22, 107, 22]),
        (b"ffffeeffbb", 138, &[113, 91, 73, 93, 133, 21]),
        (b"ffffeeffbb", 139, &[114, 75, 102, 51, 26, 21]),
        (b"ffffeeffbb", 140, &[95, 25, 35, 16, 62, 20]),
        (b"ffffeeffbb", 141, &[131, 137, 16, 110, 102, 19]),
        (b"ffffeeffbb", 142, &[125, 121, 108, 34, 6, 19]),
        (b"ffffeeffbb", 143, &[65, 78, 138, 55, 55, 18]),
        (b"ffffeeffbb", 144, &[107, 125, 121, 15, 109, 17]),
        (b"ffffeeffbb", 145, &[35, 13, 122, 42, 22, 17]),
        (b"ffffeeffbb", 146, &[107, 38, 103, 123, 83, 16]),
        (b"ffffeeffbb", 147, &[116, 96, 71, 98, 2, 16]),
        (b"ffffeeffbb", 148, &[127, 23, 75, 99, 71, 15]),
        (b"ffffeeffbb", 149, &[136, 110, 53, 114, 144, 14]),
        (b"ffffeeffbb", 150, &[95, 140, 133, 130, 71, 14]),
        (b"ffffeeffbb", 151, &[15, 50, 29, 137, 0, 14]),
        (b"ffffeeffbb", 152, &[147, 15, 89, 121, 83, 13]),
        (b"ffffeeffbb", 153, &[17, 87, 93, 72, 17, 13]),
        (b"ffffeeffbb", 154, &[109, 113, 3, 133, 106, 12]),
        (b"ffffeeffbb", 155, &[115, 141, 120, 139, 44, 12]),
        (b"ffffeeffbb", 156, &[143, 45, 4, 82, 140, 11]),
        (b"ffffeeffbb", 157, &[149, 92, 15, 106, 82, 11]),
        (b"ffffeeffbb", 158, &[37, 107, 79, 46, 26, 11]),
        (b"ffffeeffbb", 159, &[137, 37, 146, 51, 130, 10]),
        (b"ffffeeffbb", 160, &[155, 69, 29, 115, 77, 10]),
        (b"ffffeeffbb", 161, &[67, 98, 46, 68, 26, 10]),
        (b"ffffeeffbb", 162, &[125, 155, 60, 63, 138, 9]),
        (b"ffffeeffbb", 163, &[96, 43, 118, 93, 90, 9]),
        (b"ffffeeffbb", 164, &[159, 99, 123, 152, 43, 9]),
        (b"ffffeeffbb", 165, &[65, 17, 1, 69, 163, 8]),
        (b"ffffeeffbb", 166, &[135, 108, 25, 165, 119, 8]),
        (b"ffffeeffbb", 167, &[165, 116, 164, 103, 77, 8]),
        (b"ffffeeffbb", 168, &[11, 166, 67, 44, 36, 8]),
        (b"ffffeeffbb", 169, &[65, 59, 71, 149, 164, 7]),
        (b"ffffeeffbb", 170, &[85, 83, 26, 76, 126, 7]),
        (b"ffffeeffbb", 171, &[71, 132, 140, 157, 88, 7]),
        (b"ffffeeffbb", 172, &[3, 6, 127, 47, 52, 7]),
        (b"ffffeeffbb", 173, &[122, 66, 53, 83, 16, 7]),
        (b"ffffeeffbb", 174, &[35, 6, 5, 88, 155, 6]),
        (b"ffffeeffbb", 175, &[95, 20, 84, 56, 122, 6]),
        (b"ffffeeffbb", 176, &[43, 91, 57, 159, 89, 6]),
        (b"ffffeeffbb", 177, &[110, 127, 54, 40, 58, 6]),
        (b"ffffeeffbb", 178, &[49, 115, 43, 47, 27, 6]),
        (b"ffffeeffbb", 179, &[130, 91, 4, 178, 175, 5]),
        (b"ffffeeffbb", 180, &[35, 122, 109, 70, 147, 5]),
        (b"ffffeeffbb", 181, &[94, 94, 4, 79, 119, 5]),
        (b"ffffeeffbb", 182, &[39, 54, 66, 19, 92, 5]),
        (b"ffffeeffbb", 183, &[119, 2, 143, 69, 65, 5]),
        (b"ffffeeffbb", 184, &[67, 57, 90, 44, 39, 5]),
        (b"ffffeeffbb", 185, &[90, 63, 141, 123, 13, 5]),
        (b"ffffeeffbb", 186, &[53, 123, 172, 119, 174, 4]),
        (b"ffffeeffbb", 187, &[153, 21, 68, 28, 151, 4]),
        (b"ffffeeffbb", 188, &[131, 138, 94, 32, 128, 4]),
        (b"ffffeeffbb", 189, &[179, 121, 156, 130, 105, 4]),
        (b"ffffeeffbb", 190, &[185, 179, 164, 131, 83, 4]),
        (b"ffffeeffbb", 191, &[118, 123, 37, 31, 62, 4]),
        (b"ffffeeffbb", 192, &[59, 106, 83, 16, 41, 4]),
        (b"ffffeeffbb", 193, &[57, 37, 47, 86, 20, 4]),
        (b"ffffeeffbb", 194, &[191, 140, 63, 45, 0, 4]),
        (b"ffffeeffbb", 195, &[65, 169, 83, 84, 175, 3]),
        (b"ffffeeffbb", 196, &[67, 158, 64, 6, 157, 3]),
        (b"ffffeeffbb", 197, &[121, 26, 167, 3, 139, 3]),
        (b"ffffeeffbb", 198, &[197, 151, 165, 75, 121, 3]),
        (b"ffffeeffbb", 199, &[55, 175, 36, 22, 104, 3]),
        (b"ffffeeffbb", 200, &[195, 167, 162, 38, 87, 3]),
        (b"ffffeeffbb", 201, &[35, 27, 136, 124, 70, 3]),
        (b"ffffeeffbb", 202, &[87, 64, 153, 76, 54, 3]),
        (b"ffffeeffbb", 203, &[151, 191, 14, 94, 38, 3]),
        (b"ffffeeffbb", 204, &[119, 103, 135, 175, 22, 3]),
        (b"ffffeeffbb", 205, &[200, 79, 123, 115, 7, 3]),
        (b"ffffeeffbb", 206, &[133, 165, 202, 115, 198, 2]),
        (b"ffffeeffbb", 207, &[44, 153, 193, 175, 184, 2]),
        (b"ffffeeffbb", 208, &[91, 190, 125, 86, 171, 2]),
        (b"ffffeeffbb", 209, &[109, 151, 34, 53, 158, 2]),
        (b"ffffeeffbb", 210, &[95, 40, 171, 74, 145, 2]),
        (b"ffffeeffbb", 211, &[84, 195, 162, 150, 132, 2]),
        (b"ffffeeffbb", 212, &[31, 15, 59, 68, 120, 2]),
        (b"ffffeeffbb", 213, &[125, 57, 127, 36, 108, 2]),
        (b"ffffeeffbb", 214, &[51, 132, 2, 55, 96, 2]),
        (b"ffffeeffbb", 215, &[175, 133, 177, 122, 84, 2]),
        (b"ffffeeffbb", 216, &[179, 35, 78, 23, 73, 2]),
        (b"ffffeeffbb", 217, &[53, 101, 208, 186, 61, 2]),
        (b"ffffeeffbb", 218, &[33, 9, 214, 179, 50, 2]),
        (b"ffffeeffbb", 219, &[107, 147, 175, 217, 39, 2]),
        (b"ffffeeffbb", 220, &[175, 81, 179, 79, 29, 2]),
        (b"ffffeeffbb", 221, &[0, 76, 95, 204, 18, 2]),
        (b"ffffeeffbb", 222, &[53, 213, 16, 150, 8, 2]),
        (b"ffffeeffbb", 223, &[158, 161, 42, 136, 221, 1]),
        (b"ffffeeffbb", 224, &[123, 54, 52, 162, 212, 1]),
        (b"ffffeeffbb", 225, &[170, 43, 151, 2, 204, 1]),
        (b"ffffeeffbb", 226, &[27, 68, 224, 105, 195, 1]),
        (b"ffffeeffbb", 227, &[45, 69, 157, 20, 187, 1]),
        (b"ffffeeffbb", 228, &[71, 213, 64, 199, 178, 1]),
        (b"ffffeeffbb", 229, &[129, 203, 66, 186, 170, 1]),
        (b"ffffeeffbb", 230, &[205, 183, 57, 208, 162, 1]),
        (b"ffffeeffbb", 231, &[32, 50, 164, 33, 155, 1]),
        (b"ffffeeffbb", 232, &[35, 135, 53, 123, 147, 1]),
        (b"ffffeeffbb", 233, &[209, 47, 89, 13, 140, 1]),
        (b"ffffeeffbb", 234, &[143, 56, 175, 168, 132, 1]),
        (b"ffffeeffbb", 235, &[225, 157, 216, 121, 125, 1]),
        (b"ffffeeffbb", 236, &[51, 66, 119, 105, 118, 1]),
        (b"ffffeeffbb", 237, &[116, 150, 26, 119, 111, 1]),
        (b"ffffeeffbb", 238, &[221, 15, 87, 162, 104, 1]),
        (b"ffffeeffbb", 239, &[234, 155, 214, 234, 97, 1]),
        (b"ffffeeffbb", 240, &[155, 46, 84, 96, 91, 1]),
        (b"ffffeeffbb", 241, &[187, 48, 90, 225, 84, 1]),
        (b"ffffeeffbb", 242, &[87, 212, 151, 140, 78, 1]),
        (b"ffffeeffbb", 243, &[206, 22, 189, 81, 72, 1]),
        (b"ffffeeffbb", 244, &[119, 93, 122, 48, 66, 1]),
        (b"ffffeeffbb", 245, &[165, 224, 117, 40, 60, 1]),
        (b"ffffeeffbb", 246, &[77, 121, 100, 57, 54, 1]),
        (b"ffffeeffbb", 247, &[52, 128, 242, 98, 48, 1]),
        (b"ffffeeffbb", 248, &[115, 247, 224, 164, 42, 1]),
        (b"ffffeeffbb", 249, &[218, 127, 223, 5, 37, 1]),
        (b"ffffeeffbb", 250, &[95, 54, 168, 118, 31, 1]),
        (b"ffffeeffbb", 251, &[121, 204, 240, 3, 26, 1]),
        (b"ffffeeffbb", 252, &[179, 138, 123, 162, 20, 1]),
        (b"ffffeeffbb", 253, &[21, 50, 1, 91, 15, 1]),
        (b"ffffeeffbb", 254, &[149, 11, 63, 40, 10, 1]),
        (b"ffffeeffbb", 255, &[170, 225, 247, 9, 5, 1]),
        (b"ffffeeffbb", 256, &[187, 255, 238, 255, 255]),
    ];

    for &(bigint, radix, inbaseradix_le) in GROUND_TRUTH.iter() {
        let bigint = BigUint::parse_bytes(bigint, 16).unwrap();
        // to_radix_le
        assert_eq!(bigint.to_radix_le(radix), inbaseradix_le);
        // to_radix_be
        let mut inbase_be = bigint.to_radix_be(radix);
        inbase_be.reverse(); // now le
        assert_eq!(inbase_be, inbaseradix_le);
        // from_radix_le
        assert_eq!(
            BigUint::from_radix_le(inbaseradix_le, radix).unwrap(),
            bigint
        );
        // from_radix_be
        let mut inbaseradix_be = Vec::from(inbaseradix_le);
        inbaseradix_be.reverse();
        assert_eq!(
            BigUint::from_radix_be(&inbaseradix_be, radix).unwrap(),
            bigint
        );
    }

    assert!(BigUint::from_radix_le(&[10, 100, 10], 50).is_none());
    assert_eq!(BigUint::from_radix_le(&[], 2), Some(BigUint::zero()));
    assert_eq!(BigUint::from_radix_be(&[], 2), Some(BigUint::zero()));
}

#[test]
fn test_from_str_radix() {
    let r = to_str_pairs();
    for num_pair in r.iter() {
        let &(ref n, ref rs) = num_pair;
        for str_pair in rs.iter() {
            let &(ref radix, ref str) = str_pair;
            assert_eq!(n, &BigUint::from_str_radix(str, *radix).unwrap());
        }
    }

    let zed = BigUint::from_str_radix("Z", 10).ok();
    assert_eq!(zed, None);
    let blank = BigUint::from_str_radix("_", 2).ok();
    assert_eq!(blank, None);
    let blank_one = BigUint::from_str_radix("_1", 2).ok();
    assert_eq!(blank_one, None);
    let plus_one = BigUint::from_str_radix("+1", 10).ok();
    assert_eq!(plus_one, Some(BigUint::from_slice(&[1])));
    let plus_plus_one = BigUint::from_str_radix("++1", 10).ok();
    assert_eq!(plus_plus_one, None);
    let minus_one = BigUint::from_str_radix("-1", 10).ok();
    assert_eq!(minus_one, None);
    let zero_plus_two = BigUint::from_str_radix("0+2", 10).ok();
    assert_eq!(zero_plus_two, None);
    let three = BigUint::from_str_radix("1_1", 2).ok();
    assert_eq!(three, Some(BigUint::from_slice(&[3])));
    let ff = BigUint::from_str_radix("1111_1111", 2).ok();
    assert_eq!(ff, Some(BigUint::from_slice(&[0xff])));
}

#[test]
fn test_all_str_radix() {
    let n = BigUint::new((0..10).collect());
    for radix in 2..37 {
        let s = n.to_str_radix(radix);
        let x = BigUint::from_str_radix(&s, radix);
        assert_eq!(x.unwrap(), n);

        let s = s.to_ascii_uppercase();
        let x = BigUint::from_str_radix(&s, radix);
        assert_eq!(x.unwrap(), n);
    }
}

#[test]
fn test_big_str() {
    for n in 2..=20_u32 {
        let x: BigUint = BigUint::from(n).pow(10_000_u32);
        let s = x.to_string();
        let y: BigUint = s.parse().unwrap();
        assert_eq!(x, y);
    }
}

#[test]
fn test_lower_hex() {
    let a = BigUint::parse_bytes(b"A", 16).unwrap();
    let hello = BigUint::parse_bytes(b"22405534230753963835153736737", 10).unwrap();

    assert_eq!(format!("{:x}", a), "a");
    assert_eq!(format!("{:x}", hello), "48656c6c6f20776f726c6421");
    assert_eq!(format!("{:♥>+#8x}", a), "♥♥♥♥+0xa");
}

#[test]
fn test_upper_hex() {
    let a = BigUint::parse_bytes(b"A", 16).unwrap();
    let hello = BigUint::parse_bytes(b"22405534230753963835153736737", 10).unwrap();

    assert_eq!(format!("{:X}", a), "A");
    assert_eq!(format!("{:X}", hello), "48656C6C6F20776F726C6421");
    assert_eq!(format!("{:♥>+#8X}", a), "♥♥♥♥+0xA");
}

#[test]
fn test_binary() {
    let a = BigUint::parse_bytes(b"A", 16).unwrap();
    let hello = BigUint::parse_bytes(b"224055342307539", 10).unwrap();

    assert_eq!(format!("{:b}", a), "1010");
    assert_eq!(
        format!("{:b}", hello),
        "110010111100011011110011000101101001100011010011"
    );
    assert_eq!(format!("{:♥>+#8b}", a), "♥+0b1010");
}

#[test]
fn test_octal() {
    let a = BigUint::parse_bytes(b"A", 16).unwrap();
    let hello = BigUint::parse_bytes(b"22405534230753963835153736737", 10).unwrap();

    assert_eq!(format!("{:o}", a), "12");
    assert_eq!(format!("{:o}", hello), "22062554330674403566756233062041");
    assert_eq!(format!("{:♥>+#8o}", a), "♥♥♥+0o12");
}

#[test]
fn test_display() {
    let a = BigUint::parse_bytes(b"A", 16).unwrap();
    let hello = BigUint::parse_bytes(b"22405534230753963835153736737", 10).unwrap();

    assert_eq!(format!("{}", a), "10");
    assert_eq!(format!("{}", hello), "22405534230753963835153736737");
    assert_eq!(format!("{:♥>+#8}", a), "♥♥♥♥♥+10");
}

#[test]
fn test_factor() {
    fn factor(n: usize) -> BigUint {
        let mut f: BigUint = One::one();
        for i in 2..=n {
            // FIXME(#5992): assignment operator overloads
            // f *= FromPrimitive::from_usize(i);
            let bu: BigUint = FromPrimitive::from_usize(i).unwrap();
            f *= bu;
        }
        f
    }

    fn check(n: usize, s: &str) {
        let n = factor(n);
        let ans = BigUint::from_str_radix(s, 10).unwrap();
        assert_eq!(n, ans);
    }

    check(3, "6");
    check(10, "3628800");
    check(20, "2432902008176640000");
    check(30, "265252859812191058636308480000000");
}

#[test]
fn test_bits() {
    assert_eq!(BigUint::new(vec![0, 0, 0, 0]).bits(), 0);
    let n: BigUint = FromPrimitive::from_usize(0).unwrap();
    assert_eq!(n.bits(), 0);
    let n: BigUint = FromPrimitive::from_usize(1).unwrap();
    assert_eq!(n.bits(), 1);
    let n: BigUint = FromPrimitive::from_usize(3).unwrap();
    assert_eq!(n.bits(), 2);
    let n: BigUint = BigUint::from_str_radix("4000000000", 16).unwrap();
    assert_eq!(n.bits(), 39);
    let one: BigUint = One::one();
    assert_eq!((one << 426u16).bits(), 427);
}

#[test]
fn test_iter_sum() {
    let result: BigUint = FromPrimitive::from_isize(1234567).unwrap();
    let data: Vec<BigUint> = vec![
        FromPrimitive::from_u32(1000000).unwrap(),
        FromPrimitive::from_u32(200000).unwrap(),
        FromPrimitive::from_u32(30000).unwrap(),
        FromPrimitive::from_u32(4000).unwrap(),
        FromPrimitive::from_u32(500).unwrap(),
        FromPrimitive::from_u32(60).unwrap(),
        FromPrimitive::from_u32(7).unwrap(),
    ];

    assert_eq!(result, data.iter().sum::<BigUint>());
    assert_eq!(result, data.into_iter().sum::<BigUint>());
}

#[test]
fn test_iter_product() {
    let data: Vec<BigUint> = vec![
        FromPrimitive::from_u32(1001).unwrap(),
        FromPrimitive::from_u32(1002).unwrap(),
        FromPrimitive::from_u32(1003).unwrap(),
        FromPrimitive::from_u32(1004).unwrap(),
        FromPrimitive::from_u32(1005).unwrap(),
    ];
    let result = data.get(0).unwrap()
        * data.get(1).unwrap()
        * data.get(2).unwrap()
        * data.get(3).unwrap()
        * data.get(4).unwrap();

    assert_eq!(result, data.iter().product::<BigUint>());
    assert_eq!(result, data.into_iter().product::<BigUint>());
}

#[test]
fn test_iter_sum_generic() {
    let result: BigUint = FromPrimitive::from_isize(1234567).unwrap();
    let data = vec![1000000_u32, 200000, 30000, 4000, 500, 60, 7];

    assert_eq!(result, data.iter().sum::<BigUint>());
    assert_eq!(result, data.into_iter().sum::<BigUint>());
}

#[test]
fn test_iter_product_generic() {
    let data = vec![1001_u32, 1002, 1003, 1004, 1005];
    let result = data[0].to_biguint().unwrap()
        * data[1].to_biguint().unwrap()
        * data[2].to_biguint().unwrap()
        * data[3].to_biguint().unwrap()
        * data[4].to_biguint().unwrap();

    assert_eq!(result, data.iter().product::<BigUint>());
    assert_eq!(result, data.into_iter().product::<BigUint>());
}

#[test]
fn test_pow() {
    let one = BigUint::from(1u32);
    let two = BigUint::from(2u32);
    let four = BigUint::from(4u32);
    let eight = BigUint::from(8u32);
    let tentwentyfour = BigUint::from(1024u32);
    let twentyfourtyeight = BigUint::from(2048u32);
    macro_rules! check {
        ($t:ty) => {
            assert_eq!(Pow::pow(&two, 0 as $t), one);
            assert_eq!(Pow::pow(&two, 1 as $t), two);
            assert_eq!(Pow::pow(&two, 2 as $t), four);
            assert_eq!(Pow::pow(&two, 3 as $t), eight);
            assert_eq!(Pow::pow(&two, 10 as $t), tentwentyfour);
            assert_eq!(Pow::pow(&two, 11 as $t), twentyfourtyeight);
            assert_eq!(Pow::pow(&two, &(11 as $t)), twentyfourtyeight);
        };
    }
    check!(u8);
    check!(u16);
    check!(u32);
    check!(u64);
    check!(u128);
    check!(usize);

    let pow_1e10000 = BigUint::from(10u32).pow(10_000_u32);
    let manual_1e10000 = repeat(10u32).take(10_000).product::<BigUint>();
    assert!(manual_1e10000 == pow_1e10000);
}

#[test]
fn test_trailing_zeros() {
    assert!(BigUint::from(0u8).trailing_zeros().is_none());
    assert_eq!(BigUint::from(1u8).trailing_zeros().unwrap(), 0);
    assert_eq!(BigUint::from(2u8).trailing_zeros().unwrap(), 1);
    let x: BigUint = BigUint::one() << 128;
    assert_eq!(x.trailing_zeros().unwrap(), 128);
}

#[test]
fn test_trailing_ones() {
    assert_eq!(BigUint::from(0u8).trailing_ones(), 0);
    assert_eq!(BigUint::from(1u8).trailing_ones(), 1);
    assert_eq!(BigUint::from(2u8).trailing_ones(), 0);
    assert_eq!(BigUint::from(3u8).trailing_ones(), 2);
    let x: BigUint = (BigUint::from(3u8) << 128) | BigUint::from(3u8);
    assert_eq!(x.trailing_ones(), 2);
    let x: BigUint = (BigUint::one() << 128) - BigUint::one();
    assert_eq!(x.trailing_ones(), 128);
}

#[test]
fn test_count_ones() {
    assert_eq!(BigUint::from(0u8).count_ones(), 0);
    assert_eq!(BigUint::from(1u8).count_ones(), 1);
    assert_eq!(BigUint::from(2u8).count_ones(), 1);
    assert_eq!(BigUint::from(3u8).count_ones(), 2);
    let x: BigUint = (BigUint::from(3u8) << 128) | BigUint::from(3u8);
    assert_eq!(x.count_ones(), 4);
}

#[test]
fn test_bit() {
    assert!(!BigUint::from(0u8).bit(0));
    assert!(!BigUint::from(0u8).bit(100));
    assert!(!BigUint::from(42u8).bit(4));
    assert!(BigUint::from(42u8).bit(5));
    let x: BigUint = (BigUint::from(3u8) << 128) | BigUint::from(3u8);
    assert!(x.bit(129));
    assert!(!x.bit(130));
}

#[test]
fn test_set_bit() {
    let mut x = BigUint::from(3u8);
    x.set_bit(128, true);
    x.set_bit(129, true);
    assert_eq!(x, (BigUint::from(3u8) << 128) | BigUint::from(3u8));
    x.set_bit(0, false);
    x.set_bit(128, false);
    x.set_bit(130, false);
    assert_eq!(x, (BigUint::from(2u8) << 128) | BigUint::from(2u8));
    x.set_bit(129, false);
    x.set_bit(1, false);
    assert_eq!(x, BigUint::zero());
}
