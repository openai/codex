#![cfg(all( // require `--all-features` to be passed
    feature = "default",
    feature = "alloc",
    feature = "formatting",
    feature = "large-dates",
    feature = "local-offset",
    feature = "macros",
    feature = "parsing",
    feature = "quickcheck",
    feature = "serde-human-readable",
    feature = "serde-well-known",
    feature = "std",
    feature = "rand",
    feature = "serde",
))]
#![allow(
    clippy::cognitive_complexity,
    reason = "many tests in one function is okay"
)]
#![allow(clippy::std_instead_of_core, reason = "irrelevant for tests")]

//! Tests for internal details.
//!
//! This module should only be used when it is not possible to test the implementation in a
//! reasonable manner externally.

use std::format;
use std::num::NonZero;

use crate::ext::DigitCount;
use crate::parsing::combinator::rfc::iso8601;
use crate::parsing::shim::Integer;
use crate::{duration, parsing};

#[test]
fn digit_count() {
    assert_eq!(1_u8.num_digits(), 1);
    assert_eq!(9_u8.num_digits(), 1);
    assert_eq!(10_u8.num_digits(), 2);
    assert_eq!(99_u8.num_digits(), 2);
    assert_eq!(100_u8.num_digits(), 3);

    assert_eq!(1_u16.num_digits(), 1);
    assert_eq!(9_u16.num_digits(), 1);
    assert_eq!(10_u16.num_digits(), 2);
    assert_eq!(99_u16.num_digits(), 2);
    assert_eq!(100_u16.num_digits(), 3);
    assert_eq!(999_u16.num_digits(), 3);
    assert_eq!(1_000_u16.num_digits(), 4);
    assert_eq!(9_999_u16.num_digits(), 4);
    assert_eq!(10_000_u16.num_digits(), 5);

    assert_eq!(1_u32.num_digits(), 1);
    assert_eq!(9_u32.num_digits(), 1);
    assert_eq!(10_u32.num_digits(), 2);
    assert_eq!(99_u32.num_digits(), 2);
    assert_eq!(100_u32.num_digits(), 3);
    assert_eq!(999_u32.num_digits(), 3);
    assert_eq!(1_000_u32.num_digits(), 4);
    assert_eq!(9_999_u32.num_digits(), 4);
    assert_eq!(10_000_u32.num_digits(), 5);
    assert_eq!(99_999_u32.num_digits(), 5);
    assert_eq!(100_000_u32.num_digits(), 6);
    assert_eq!(999_999_u32.num_digits(), 6);
    assert_eq!(1_000_000_u32.num_digits(), 7);
    assert_eq!(9_999_999_u32.num_digits(), 7);
    assert_eq!(10_000_000_u32.num_digits(), 8);
    assert_eq!(99_999_999_u32.num_digits(), 8);
    assert_eq!(100_000_000_u32.num_digits(), 9);
    assert_eq!(999_999_999_u32.num_digits(), 9);
    assert_eq!(1_000_000_000_u32.num_digits(), 10);
}

#[expect(
    let_underscore_drop,
    reason = "no need for the resulting value, which is #![must_use]"
)]
#[test]
fn debug() {
    let _ = format!("{:?}", duration::Padding::Optimize);
    let _ = format!("{:?}", parsing::ParsedItem(b"", 0));
    let _ = format!("{:?}", parsing::component::Period::Am);
    let _ = format!("{:?}", iso8601::ExtendedKind::Basic);
}

#[expect(clippy::clone_on_copy, reason = "purpose of the test")]
#[test]
fn clone() {
    assert_eq!(
        parsing::component::Period::Am.clone(),
        parsing::component::Period::Am
    );
    // does not impl Debug
    assert!(crate::time::Padding::Optimize.clone() == crate::time::Padding::Optimize);
    // does not impl PartialEq
    assert!(matches!(
        iso8601::ExtendedKind::Basic.clone(),
        iso8601::ExtendedKind::Basic
    ));
}

#[test]
fn parsing_internals() {
    assert!(parsing::ParsedItem(b"", ())
        .flat_map(|_| None::<()>)
        .is_none());
    assert!(<NonZero<u8> as Integer>::parse_bytes(b"256").is_none());
}
