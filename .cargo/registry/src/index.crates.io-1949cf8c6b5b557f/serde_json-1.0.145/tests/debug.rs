#![allow(clippy::uninlined_format_args)]

use indoc::indoc;
use serde_json::{json, Number, Value};

#[test]
fn number() {
    assert_eq!(format!("{:?}", Number::from(1)), "Number(1)");
    assert_eq!(format!("{:?}", Number::from(-1)), "Number(-1)");
    assert_eq!(
        format!("{:?}", Number::from_f64(1.0).unwrap()),
        "Number(1.0)"
    );
}

#[test]
fn value_null() {
    assert_eq!(format!("{:?}", json!(null)), "Null");
}

#[test]
fn value_bool() {
    assert_eq!(format!("{:?}", json!(true)), "Bool(true)");
    assert_eq!(format!("{:?}", json!(false)), "Bool(false)");
}

#[test]
fn value_number() {
    assert_eq!(format!("{:?}", json!(1)), "Number(1)");
    assert_eq!(format!("{:?}", json!(-1)), "Number(-1)");
    assert_eq!(format!("{:?}", json!(1.0)), "Number(1.0)");
    assert_eq!(Number::from_f64(1.0).unwrap().to_string(), "1.0"); // not just "1"
    assert_eq!(Number::from_f64(12e40).unwrap().to_string(), "1.2e41");
}

#[test]
fn value_string() {
    assert_eq!(format!("{:?}", json!("s")), "String(\"s\")");
}

#[test]
fn value_array() {
    assert_eq!(format!("{:?}", json!([])), "Array []");
}

#[test]
fn value_object() {
    assert_eq!(format!("{:?}", json!({})), "Object {}");
}

#[test]
fn error() {
    let err = serde_json::from_str::<Value>("{0}").unwrap_err();
    let expected = "Error(\"key must be a string\", line: 1, column: 2)";
    assert_eq!(format!("{:?}", err), expected);
}

#[test]
fn indented() {
    let j = json!({
        "Array": [true],
        "Bool": true,
        "EmptyArray": [],
        "EmptyObject": {},
        "Null": null,
        "Number": 1,
        "String": "...",
    });
    let expected = indoc! {r#"
        Object {
            "Array": Array [
                Bool(true),
            ],
            "Bool": Bool(true),
            "EmptyArray": Array [],
            "EmptyObject": Object {},
            "Null": Null,
            "Number": Number(1),
            "String": String("..."),
        }"#
    };
    assert_eq!(format!("{:#?}", j), expected);
}
