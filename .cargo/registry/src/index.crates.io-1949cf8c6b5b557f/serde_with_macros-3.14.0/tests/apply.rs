//! Test Cases

#![allow(dead_code)]

use expect_test::expect;
use serde_with_macros::apply;
use std::collections::BTreeMap;

/// Fields `a` and `c` match, as each has a fully specified pattern.
#[test]
fn test_apply_fully_specified() {
    #[apply(
    crate="serde_with_macros",

    Option<String> => #[serde(skip)],
    BTreeMap<String, String> => #[serde(skip)],
)]
    #[derive(Default, serde::Serialize)]
    struct FooBar<'a> {
        a: Option<String>,
        b: Option<i32>,
        c: BTreeMap<String, String>,
        d: BTreeMap<String, i32>,
        e: BTreeMap<i32, String>,
        f: &'a str,
        g: &'static str,

        #[serde_with(skip_apply)]
        skip: Option<String>,
    }

    expect![[r#"
        {
          "b": null,
          "d": {},
          "e": {},
          "f": "",
          "g": "",
          "skip": null
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&FooBar::<'static>::default()).unwrap());
}

/// All fields match as `_` matches any type.
///
/// The `skip` field is ignored because of the `#[serde_with(skip_apply)]` attribute.
#[test]
fn test_apply_all() {
    #[apply(
    crate="serde_with_macros",

    _ => #[serde(skip)],
)]
    #[derive(Default, serde::Serialize)]
    struct FooBar<'a> {
        a: Option<String>,
        b: Option<i32>,
        c: BTreeMap<String, String>,
        d: BTreeMap<String, i32>,
        e: BTreeMap<i32, String>,
        f: &'a str,
        g: &'static str,

        #[serde_with(skip_apply)]
        skip: Option<String>,
    }

    expect![[r#"
        {
          "skip": null
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&FooBar::<'static>::default()).unwrap());
}

/// Fields `a` and `b` match, since both are variants of `Option`.
///
/// No generic in the pattern allows matching with any number of generics on the fields.
/// The `skip` field is ignored because of the `#[serde_with(skip_apply)]` attribute.
#[test]
fn test_apply_partial_no_generic() {
    #[apply(
    crate="serde_with_macros",

    Option => #[serde(skip)],
)]
    #[derive(Default, serde::Serialize)]
    struct FooBar<'a> {
        a: Option<String>,
        b: Option<i32>,
        c: BTreeMap<String, String>,
        d: BTreeMap<String, i32>,
        e: BTreeMap<i32, String>,
        f: &'a str,
        g: &'static str,

        #[serde_with(skip_apply)]
        skip: Option<String>,
    }

    expect![[r#"
        {
          "c": {},
          "d": {},
          "e": {},
          "f": "",
          "g": "",
          "skip": null
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&FooBar::<'static>::default()).unwrap());
}

/// Fields `c` and `d` match, as both have a `String` key and `_` matches any type.
#[test]
fn test_apply_partial_generic() {
    #[apply(
    crate="serde_with_macros",

    BTreeMap<String, _> => #[serde(skip)],
)]
    #[derive(Default, serde::Serialize)]
    struct FooBar<'a> {
        a: Option<String>,
        b: Option<i32>,
        c: BTreeMap<String, String>,
        d: BTreeMap<String, i32>,
        e: BTreeMap<i32, String>,
        f: &'a str,
        g: &'static str,

        #[serde_with(skip_apply)]
        skip: Option<String>,
    }

    expect![[r#"
        {
          "a": null,
          "b": null,
          "e": {},
          "f": "",
          "g": "",
          "skip": null
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&FooBar::<'static>::default()).unwrap());
}

/// Fields `f` and `g` match, since no lifetime matches any reference.
#[test]
fn test_apply_no_lifetime() {
    #[apply(
    crate="serde_with_macros",

    &str => #[serde(skip)],
)]
    #[derive(Default, serde::Serialize)]
    struct FooBar<'a> {
        a: Option<String>,
        b: Option<i32>,
        c: BTreeMap<String, String>,
        d: BTreeMap<String, i32>,
        e: BTreeMap<i32, String>,
        f: &'a str,
        g: &'static str,

        #[serde_with(skip_apply)]
        skip: Option<String>,
    }

    expect![[r#"
        {
          "a": null,
          "b": null,
          "c": {},
          "d": {},
          "e": {},
          "skip": null
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&FooBar::<'static>::default()).unwrap());
}

/// Field `f` matches as the lifetime is identical and `mut` is ignored.
#[test]
fn test_apply_lifetime() {
    #[apply(
    crate="serde_with_macros",

    &'a mut str => #[serde(skip)],
)]
    #[derive(Default, serde::Serialize)]
    struct FooBar<'a> {
        a: Option<String>,
        b: Option<i32>,
        c: BTreeMap<String, String>,
        d: BTreeMap<String, i32>,
        e: BTreeMap<i32, String>,
        f: &'a str,
        g: &'static str,

        #[serde_with(skip_apply)]
        skip: Option<String>,
    }

    expect![[r#"
        {
          "a": null,
          "b": null,
          "c": {},
          "d": {},
          "e": {},
          "g": "",
          "skip": null
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&FooBar::<'static>::default()).unwrap());
}

/// No field matches as the explicit lifetimes are different
#[test]
fn test_apply_mismatched_lifetime() {
    #[apply(
    crate="serde_with_macros",

    &'b str => #[serde(skip)],
)]
    #[derive(Default, serde::Serialize)]
    struct FooBar<'a> {
        a: Option<String>,
        b: Option<i32>,
        c: BTreeMap<String, String>,
        d: BTreeMap<String, i32>,
        e: BTreeMap<i32, String>,
        f: &'a str,
        g: &'static str,

        #[serde_with(skip_apply)]
        skip: Option<String>,
    }

    expect![[r#"
        {
          "a": null,
          "b": null,
          "c": {},
          "d": {},
          "e": {},
          "f": "",
          "g": "",
          "skip": null
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&FooBar::<'static>::default()).unwrap());
}
