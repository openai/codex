#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::ToString;

use derive_more::FromStr;

#[derive(FromStr)]
struct MyInt(i32);

#[derive(FromStr)]
struct Point1D {
    x: i32,
}

/// Making sure that `FromStr` does not trigger an ambiguous associated item error for `Err`.
#[derive(FromStr)]
enum EnumWithErr {
    Err,
}

#[derive(Debug, FromStr, PartialEq, Eq)]
enum EnumNoFields {
    Foo,
    Bar,
    Baz,
    BaZ,
}

#[test]
fn enum_test() {
    assert_eq!("Foo".parse::<EnumNoFields>().unwrap(), EnumNoFields::Foo);
    assert_eq!("FOO".parse::<EnumNoFields>().unwrap(), EnumNoFields::Foo);
    assert_eq!("foo".parse::<EnumNoFields>().unwrap(), EnumNoFields::Foo);
    assert_eq!(
        "other".parse::<EnumNoFields>().unwrap_err().to_string(),
        "Invalid `EnumNoFields` string representation",
    );
}

#[test]
fn enum_test_case_sensitive() {
    assert_eq!("Baz".parse::<EnumNoFields>().unwrap(), EnumNoFields::Baz);
    assert_eq!("BaZ".parse::<EnumNoFields>().unwrap(), EnumNoFields::BaZ);
    assert_eq!(
        "baz".parse::<EnumNoFields>().unwrap_err().to_string(),
        "Invalid `EnumNoFields` string representation",
    );
}
