#![allow(dead_code)]

#[cfg(feature = "serde-compat")]
use serde::Deserialize;
use ts_rs::TS;

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Deserialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "kind", content = "d"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "kind", content = "d"))]
#[ts(export, export_to = "union_serde/")]
enum SimpleEnum {
    A,
    B,
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Deserialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "kind", content = "data"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "kind", content = "data"))]
#[ts(export, export_to = "union_serde/")]
enum ComplexEnum {
    A,
    B { foo: String, bar: f64 },
    W(SimpleEnum),
    F { nested: SimpleEnum },
    T(i32, SimpleEnum),
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Deserialize))]
#[cfg_attr(feature = "serde-compat", serde(untagged))]
#[cfg_attr(not(feature = "serde-compat"), ts(untagged))]
#[ts(export, export_to = "union_serde/")]
enum Untagged {
    Foo(String),
    Bar(i32),
    None,
}

#[test]
fn test_serde_enum() {
    assert_eq!(
        SimpleEnum::decl(),
        r#"type SimpleEnum = { "kind": "A" } | { "kind": "B" };"#
    );
    assert_eq!(
        ComplexEnum::decl(),
        r#"type ComplexEnum = { "kind": "A" } | { "kind": "B", "data": { foo: string, bar: number, } } | { "kind": "W", "data": SimpleEnum } | { "kind": "F", "data": { nested: SimpleEnum, } } | { "kind": "T", "data": [number, SimpleEnum] };"#
    );

    assert_eq!(
        Untagged::decl(),
        r#"type Untagged = string | number | null;"#
    )
}
