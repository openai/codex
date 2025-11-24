#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::TryFrom;

#[test]
fn test_with_repr() {
    #[derive(TryFrom, Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i16)]
    #[try_from(repr)]
    enum Enum {
        A,
        B = -21,
        C,
        D,
    }
    assert_eq!(Enum::A, Enum::try_from(0i16).unwrap());
    assert_eq!(Enum::B, Enum::try_from(-21).unwrap());
    assert_eq!(Enum::C, Enum::try_from(-20).unwrap());
    assert_eq!(Enum::D, Enum::try_from(-19).unwrap());
    assert!(Enum::try_from(-1).is_err());
}

#[test]
fn enum_without_repr() {
    #[derive(TryFrom, Clone, Copy, Debug, Eq, PartialEq)]
    #[try_from(repr)]
    enum Enum {
        A,
        B = -21,
        C,
        D,
    }
    assert_eq!(Enum::A, Enum::try_from(0isize).unwrap());
    assert_eq!(Enum::B, Enum::try_from(-21).unwrap());
    assert_eq!(Enum::C, Enum::try_from(-20).unwrap());
    assert_eq!(Enum::D, Enum::try_from(-19).unwrap());
    assert!(Enum::try_from(-1).is_err());
}

#[test]
fn enum_with_complex_repr() {
    #[derive(TryFrom, Clone, Copy, Debug, Eq, PartialEq)]
    #[try_from(repr)]
    #[repr(align(16), i32)]
    enum Enum {
        A,
        B = -21,
        C,
        D,
    }
    assert_eq!(Enum::A, Enum::try_from(0i32).unwrap());
    assert_eq!(Enum::B, Enum::try_from(-21).unwrap());
    assert_eq!(Enum::C, Enum::try_from(-20).unwrap());
    assert_eq!(Enum::D, Enum::try_from(-19).unwrap());
    assert!(Enum::try_from(-1).is_err());
}

#[test]
fn test_discriminants_on_enum_with_fields() {
    #[derive(TryFrom, Clone, Copy, Debug, Eq, PartialEq)]
    #[try_from(repr)]
    #[repr(i16)]
    enum Enum {
        A,
        Discriminant = 5,
        Field(usize),
        Empty {},
        FieldWithDiscriminant(u8, i64) = -14,
        EmptyTuple(),
    }

    assert_eq!(Enum::A, Enum::try_from(0).unwrap());
    assert_eq!(Enum::Discriminant, Enum::try_from(5).unwrap());
    assert!(Enum::try_from(6).is_err());
    assert_eq!(Enum::Empty {}, Enum::try_from(7).unwrap());
    assert!(Enum::try_from(-14).is_err());
    assert_eq!(Enum::EmptyTuple(), Enum::try_from(-13).unwrap());
}
