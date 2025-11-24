//! Test Cases

use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_with_macros::skip_serializing_none;

macro_rules! test {
    ($fn:ident, $struct:ident) => {
        #[test]
        fn $fn() {
            let expected = json!({});
            let data = $struct {
                a: None,
                b: None,
                c: None,
                d: None,
            };
            let res = serde_json::to_value(&data).unwrap();
            assert_eq!(expected, res);
            assert_eq!(data, serde_json::from_value(res).unwrap());
        }
    };
}

macro_rules! test_tuple {
    ($fn:ident, $struct:ident) => {
        #[test]
        fn $fn() {
            let expected = json!([]);
            let data = $struct(None, None);
            let res = serde_json::to_value(&data).unwrap();
            assert_eq!(expected, res);
        }
    };
}

#[skip_serializing_none]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DataBasic {
    a: Option<String>,
    b: Option<String>,
    c: Option<String>,
    d: Option<String>,
}
test!(test_basic, DataBasic);

// This tests different ways of qualifying the Option type
#[allow(unused_qualifications)]
#[skip_serializing_none]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DataFullyQualified {
    a: ::std::option::Option<String>,
    b: std::option::Option<String>,
    c: ::std::option::Option<i64>,
    d: core::option::Option<String>,
}
test!(test_fully_qualified, DataFullyQualified);

fn never<T>(_t: &T) -> bool {
    false
}

#[skip_serializing_none]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DataExistingAnnotation {
    #[serde(skip_serializing_if = "Option::is_none")]
    a: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "abc")]
    b: Option<String>,
    #[serde(default)]
    c: Option<String>,
    #[serde(skip_serializing_if = "never")]
    #[serde(rename = "name")]
    d: Option<String>,
}

#[test]
fn test_existing_annotation() {
    let expected = json!({ "name": null });
    let data = DataExistingAnnotation {
        a: None,
        b: None,
        c: None,
        d: None,
    };
    let res = serde_json::to_value(&data).unwrap();
    assert_eq!(expected, res);
    assert_eq!(data, serde_json::from_value(res).unwrap());
}

#[skip_serializing_none]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DataSerializeAlways {
    #[serialize_always]
    a: Option<String>,
    #[serialize_always]
    b: Option<String>,
    c: i64,
    #[serialize_always]
    d: Option<String>,
}

#[test]
fn test_serialize_always() {
    let expected = json!({
        "a": null,
        "b": null,
        "c": 0,
        "d": null
    });
    let data = DataSerializeAlways {
        a: None,
        b: None,
        c: 0,
        d: None,
    };
    let res = serde_json::to_value(&data).unwrap();
    assert_eq!(expected, res);
    assert_eq!(data, serde_json::from_value(res).unwrap());
}

// This tests different ways of qualifying the Option type
#[allow(unused_qualifications)]
#[skip_serializing_none]
#[derive(Debug, Eq, PartialEq, Serialize)]
struct DataTuple(Option<String>, std::option::Option<String>);
test_tuple!(test_tuple, DataTuple);

// This tests different ways of qualifying the Option type
#[allow(unused_qualifications)]
#[skip_serializing_none]
#[derive(Debug, Eq, PartialEq, Serialize)]
enum DataEnum {
    Tuple(Option<i64>, std::option::Option<bool>),
    Struct {
        a: Option<String>,
        b: Option<String>,
    },
}

#[test]
fn test_enum() {
    let expected = json!({
        "Tuple": []
    });
    let data = DataEnum::Tuple(None, None);
    let res = serde_json::to_value(data).unwrap();
    assert_eq!(expected, res);

    let expected = json!({
        "Struct": {}
    });
    let data = DataEnum::Struct { a: None, b: None };
    let res = serde_json::to_value(data).unwrap();
    assert_eq!(expected, res);
}
