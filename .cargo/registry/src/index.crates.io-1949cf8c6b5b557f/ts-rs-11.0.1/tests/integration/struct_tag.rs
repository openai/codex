#![allow(dead_code)]

#[cfg(feature = "serde-compat")]
use serde::Serialize;
use ts_rs::TS;

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "type"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "type"))]
struct TaggedType {
    a: i32,
    b: i32,
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "type"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "type"))]
struct EmptyTaggedType {}

#[test]
fn test() {
    assert_eq!(
        TaggedType::inline(),
        "{ \"type\": \"TaggedType\", a: number, b: number, }"
    );

    assert_eq!(
        EmptyTaggedType::inline(),
        r#"{ "type": "EmptyTaggedType", }"#
    );
}
