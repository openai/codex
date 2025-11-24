// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use std::fmt::Debug;
use ule::ULE;
use zerovec::*;

#[make_ule(StructULE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Struct {
    a: u8,
    b: u32,
    c: Option<char>,
}

#[make_ule(HashedStructULE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[zerovec::derive(Debug, Hash)]
struct HashedStruct {
    a: u64,
    b: i16,
    c: Option<char>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[make_ule(TupleStructULE)]
struct TupleStruct(u8, char);

#[make_ule(EnumULE)]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[zerovec::derive(Debug, Hash)]
enum Enum {
    A = 0,
    B = 1,
    D = 2,
    E = 3,
    FooBar = 4,
    F = 5,
}

#[make_ule(OutOfOrderMissingZeroEnumULE)]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[allow(unused)]
enum OutOfOrderMissingZeroEnum {
    E = 3,
    B = 1,
    FooBar = 4,
    D = 2,
    F = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
#[make_ule(NoKVULE)]
#[zerovec::skip_derive(ZeroMapKV)]
struct NoKV(u8, char);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[make_ule(NoOrdULE)]
#[zerovec::skip_derive(ZeroMapKV, Ord)]
struct NoOrd(u8, char);

fn test_zerovec<T: ule::AsULE + Debug + PartialEq>(slice: &[T]) {
    let zerovec: ZeroVec<T> = slice.iter().copied().collect();

    assert_eq!(zerovec, slice);

    let bytes = zerovec.as_bytes();
    let name = std::any::type_name::<T>();
    let reparsed: ZeroVec<T> =
        ZeroVec::parse_bytes(bytes).unwrap_or_else(|_| panic!("Parsing {name} should succeed"));

    assert_eq!(reparsed, slice);
}

fn main() {
    test_zerovec(TEST_SLICE_STRUCT);
    test_zerovec(TEST_SLICE_TUPLESTRUCT);
    test_zerovec(TEST_SLICE_ENUM);

    assert!(EnumULE::parse_bytes_to_slice(&[0]).is_ok());
    assert!(EnumULE::parse_bytes_to_slice(&[1]).is_ok());
    assert!(EnumULE::parse_bytes_to_slice(&[5]).is_ok());
    assert!(EnumULE::parse_bytes_to_slice(&[6]).is_err());
    assert!(OutOfOrderMissingZeroEnumULE::parse_bytes_to_slice(&[0]).is_err());
    assert!(OutOfOrderMissingZeroEnumULE::parse_bytes_to_slice(&[1]).is_ok());
    assert!(OutOfOrderMissingZeroEnumULE::parse_bytes_to_slice(&[5]).is_ok());
    assert!(OutOfOrderMissingZeroEnumULE::parse_bytes_to_slice(&[6]).is_err());
}

const TEST_SLICE_STRUCT: &[Struct] = &[
    Struct {
        a: 101,
        b: 924,
        c: Some('⸘'),
    },
    Struct {
        a: 217,
        b: 4228,
        c: Some('ə'),
    },
    Struct {
        a: 117,
        b: 9090,
        c: Some('ø'),
    },
];

const TEST_SLICE_TUPLESTRUCT: &[TupleStruct] = &[
    TupleStruct(101, 'ř'),
    TupleStruct(76, '°'),
    TupleStruct(15, 'a'),
];

const TEST_SLICE_ENUM: &[Enum] = &[
    Enum::A,
    Enum::FooBar,
    Enum::F,
    Enum::D,
    Enum::B,
    Enum::FooBar,
    Enum::E,
];
