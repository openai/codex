#![allow(dead_code, unused_imports)]

use std::error::Error;

use serde::Serialize;
use ts_rs::TS;

struct Unsupported;

#[derive(TS)]
#[ts(export, export_to = "skip/")]
struct Skip {
    a: i32,
    b: i32,
    #[ts(skip)]
    c: String,
    #[ts(skip)]
    d: Box<dyn Error>,
}

#[test]
fn simple() {
    assert_eq!(Skip::inline(), "{ a: number, b: number, }");
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[ts(export, export_to = "skip/")]
enum Externally {
    A(
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        Unsupported,
    ),
    B(
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        Unsupported,
        i32,
    ),
    C {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        x: Unsupported,
    },
    D {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        x: Unsupported,
        y: i32,
    },
}

#[test]
fn externally_tagged() {
    // TODO: variant C should probably not generate `{}`
    assert_eq!(
        Externally::decl(),
        r#"type Externally = "A" | { "B": [number] } | { "C": {  } } | { "D": { y: number, } };"#
    );
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "t"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "t"))]
#[ts(export, export_to = "skip/")]
enum Internally {
    A(
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        Unsupported,
    ),
    B {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        x: Unsupported,
    },
    C {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        x: Unsupported,
        y: i32,
    },
}

#[test]
fn internally_tagged() {
    assert_eq!(
        Internally::decl(),
        r#"type Internally = { "t": "A" } | { "t": "B", } | { "t": "C", y: number, };"#
    );
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "t", content = "c"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "t", content = "c"))]
#[ts(export, export_to = "skip/")]
enum Adjacently {
    A(
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        Unsupported,
    ),
    B(
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        Unsupported,
        i32,
    ),
    C {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        x: Unsupported,
    },
    D {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        x: Unsupported,
        y: i32,
    },
}

#[test]
fn adjacently_tagged() {
    // TODO: variant C should probably not generate `{ .., "c": { } }`
    assert_eq!(
        Adjacently::decl(),
        r#"type Adjacently = { "t": "A" } | { "t": "B", "c": [number] } | { "t": "C", "c": {  } } | { "t": "D", "c": { y: number, } };"#
    );
}
