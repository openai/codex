use num_bigint::{BigInt, Sign, ToBigInt};
use num_traits::ToPrimitive;

enum ValueVec {
    N,
    P(&'static [u32]),
    M(&'static [u32]),
}

use crate::ValueVec::*;

impl ToBigInt for ValueVec {
    fn to_bigint(&self) -> Option<BigInt> {
        match self {
            &N => Some(BigInt::from_slice(Sign::NoSign, &[])),
            &P(s) => Some(BigInt::from_slice(Sign::Plus, s)),
            &M(s) => Some(BigInt::from_slice(Sign::Minus, s)),
        }
    }
}

// a, !a
const NOT_VALUES: &[(ValueVec, ValueVec)] = &[
    (N, M(&[1])),
    (P(&[1]), M(&[2])),
    (P(&[2]), M(&[3])),
    (P(&[!0 - 2]), M(&[!0 - 1])),
    (P(&[!0 - 1]), M(&[!0])),
    (P(&[!0]), M(&[0, 1])),
    (P(&[0, 1]), M(&[1, 1])),
    (P(&[1, 1]), M(&[2, 1])),
];

// a, b, a & b, a | b, a ^ b
const BITWISE_VALUES: &[(ValueVec, ValueVec, ValueVec, ValueVec, ValueVec)] = &[
    (N, N, N, N, N),
    (N, P(&[1]), N, P(&[1]), P(&[1])),
    (N, P(&[!0]), N, P(&[!0]), P(&[!0])),
    (N, P(&[0, 1]), N, P(&[0, 1]), P(&[0, 1])),
    (N, M(&[1]), N, M(&[1]), M(&[1])),
    (N, M(&[!0]), N, M(&[!0]), M(&[!0])),
    (N, M(&[0, 1]), N, M(&[0, 1]), M(&[0, 1])),
    (P(&[1]), P(&[!0]), P(&[1]), P(&[!0]), P(&[!0 - 1])),
    (P(&[!0]), P(&[!0]), P(&[!0]), P(&[!0]), N),
    (P(&[!0]), P(&[1, 1]), P(&[1]), P(&[!0, 1]), P(&[!0 - 1, 1])),
    (P(&[1]), M(&[!0]), P(&[1]), M(&[!0]), M(&[0, 1])),
    (P(&[!0]), M(&[1]), P(&[!0]), M(&[1]), M(&[0, 1])),
    (P(&[!0]), M(&[!0]), P(&[1]), M(&[1]), M(&[2])),
    (P(&[!0]), M(&[1, 1]), P(&[!0]), M(&[1, 1]), M(&[0, 2])),
    (P(&[1, 1]), M(&[!0]), P(&[1, 1]), M(&[!0]), M(&[0, 2])),
    (M(&[1]), M(&[!0]), M(&[!0]), M(&[1]), P(&[!0 - 1])),
    (M(&[!0]), M(&[!0]), M(&[!0]), M(&[!0]), N),
    (M(&[!0]), M(&[1, 1]), M(&[!0, 1]), M(&[1]), P(&[!0 - 1, 1])),
];

const I32_MIN: i64 = i32::MIN as i64;
const I32_MAX: i64 = i32::MAX as i64;
const U32_MAX: i64 = u32::MAX as i64;

// some corner cases
const I64_VALUES: &[i64] = &[
    i64::MIN,
    i64::MIN + 1,
    i64::MIN + 2,
    i64::MIN + 3,
    -U32_MAX - 3,
    -U32_MAX - 2,
    -U32_MAX - 1,
    -U32_MAX,
    -U32_MAX + 1,
    -U32_MAX + 2,
    -U32_MAX + 3,
    I32_MIN - 3,
    I32_MIN - 2,
    I32_MIN - 1,
    I32_MIN,
    I32_MIN + 1,
    I32_MIN + 2,
    I32_MIN + 3,
    -3,
    -2,
    -1,
    0,
    1,
    2,
    3,
    I32_MAX - 3,
    I32_MAX - 2,
    I32_MAX - 1,
    I32_MAX,
    I32_MAX + 1,
    I32_MAX + 2,
    I32_MAX + 3,
    U32_MAX - 3,
    U32_MAX - 2,
    U32_MAX - 1,
    U32_MAX,
    U32_MAX + 1,
    U32_MAX + 2,
    U32_MAX + 3,
    i64::MAX - 3,
    i64::MAX - 2,
    i64::MAX - 1,
    i64::MAX,
];

#[test]
fn test_not() {
    for &(ref a, ref not) in NOT_VALUES.iter() {
        let a = a.to_bigint().unwrap();
        let not = not.to_bigint().unwrap();

        // sanity check for tests that fit in i64
        if let (Some(prim_a), Some(prim_not)) = (a.to_i64(), not.to_i64()) {
            assert_eq!(!prim_a, prim_not);
        }

        assert_eq!(!a.clone(), not, "!{:x}", a);
        assert_eq!(!not.clone(), a, "!{:x}", not);
    }
}

#[test]
fn test_not_i64() {
    for &prim_a in I64_VALUES.iter() {
        let a = prim_a.to_bigint().unwrap();
        let not = (!prim_a).to_bigint().unwrap();
        assert_eq!(!a.clone(), not, "!{:x}", a);
    }
}

#[test]
fn test_bitwise() {
    for &(ref a, ref b, ref and, ref or, ref xor) in BITWISE_VALUES.iter() {
        let a = a.to_bigint().unwrap();
        let b = b.to_bigint().unwrap();
        let and = and.to_bigint().unwrap();
        let or = or.to_bigint().unwrap();
        let xor = xor.to_bigint().unwrap();

        // sanity check for tests that fit in i64
        if let (Some(prim_a), Some(prim_b)) = (a.to_i64(), b.to_i64()) {
            if let Some(prim_and) = and.to_i64() {
                assert_eq!(prim_a & prim_b, prim_and);
            }
            if let Some(prim_or) = or.to_i64() {
                assert_eq!(prim_a | prim_b, prim_or);
            }
            if let Some(prim_xor) = xor.to_i64() {
                assert_eq!(prim_a ^ prim_b, prim_xor);
            }
        }

        assert_eq!(a.clone() & &b, and, "{:x} & {:x}", a, b);
        assert_eq!(b.clone() & &a, and, "{:x} & {:x}", b, a);
        assert_eq!(a.clone() | &b, or, "{:x} | {:x}", a, b);
        assert_eq!(b.clone() | &a, or, "{:x} | {:x}", b, a);
        assert_eq!(a.clone() ^ &b, xor, "{:x} ^ {:x}", a, b);
        assert_eq!(b.clone() ^ &a, xor, "{:x} ^ {:x}", b, a);
    }
}

#[test]
fn test_bitwise_i64() {
    for &prim_a in I64_VALUES.iter() {
        let a = prim_a.to_bigint().unwrap();
        for &prim_b in I64_VALUES.iter() {
            let b = prim_b.to_bigint().unwrap();
            let and = (prim_a & prim_b).to_bigint().unwrap();
            let or = (prim_a | prim_b).to_bigint().unwrap();
            let xor = (prim_a ^ prim_b).to_bigint().unwrap();
            assert_eq!(a.clone() & &b, and, "{:x} & {:x}", a, b);
            assert_eq!(a.clone() | &b, or, "{:x} | {:x}", a, b);
            assert_eq!(a.clone() ^ &b, xor, "{:x} ^ {:x}", a, b);
        }
    }
}
