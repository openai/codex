use crate::prelude::*;
use schemars::generate::SchemaSettings;
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MyStruct {
    pub my_int: i32,
    pub my_bool: bool,
    pub my_nullable_enum: Option<MyEnum>,
    pub my_inner_struct: MyInnerStruct,
    #[serde(skip)]
    pub _skip: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_if_none: Option<MyEnum>,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct MyInnerStruct {
    pub my_map: BTreeMap<String, f64>,
    pub my_vec: Vec<f32>,
    pub my_empty_map: BTreeMap<String, f64>,
    pub my_empty_vec: Vec<f32>,
    pub my_tuple: (char, u8),
}

#[derive(JsonSchema, Deserialize, Serialize, Clone)]
pub enum MyEnum {
    NewType(String),
    Struct { floats: Vec<f32> },
}

fn struct_value() -> MyStruct {
    MyStruct {
        my_int: 123,
        my_bool: true,
        my_nullable_enum: None,
        my_inner_struct: MyInnerStruct {
            my_map: [("k".to_owned(), 1.23)].into_iter().collect(),
            my_vec: vec![1.0, 2.0, 3.0],
            my_empty_map: BTreeMap::new(),
            my_empty_vec: Vec::new(),
            my_tuple: ('💩', 42),
        },
        _skip: 123,
        skip_if_none: None,
    }
}

#[test]
fn custom_struct() {
    let value = struct_value();

    test!(value: value.clone())
        .assert_snapshot()
        .assert_allows_ser_roundtrip([value, MyStruct::default()]);
}

#[test]
fn custom_struct_openapi3() {
    let value = struct_value();

    test!(value: value.clone(), SchemaSettings::openapi3()).assert_snapshot();

    // schemars uses a nonstandard meta-schema for openapi 3.0 which the jsonschema crate doesn't
    // accept, so we swap it out for the standard draft-04 meta-schema.
    let draft04 = "http://json-schema.org/draft-04/schema";
    test!(value: value.clone(), SchemaSettings::openapi3().with(|o| o.meta_schema = Some(draft04.into())))
        .assert_allows_ser_roundtrip([value, MyStruct::default()]);
}

#[test]
fn json_value() {
    let value = json!({
        "zero": 0,
        "zeroPointZero": 0.0,
        "bool": true,
        "null": null,
        "object": {
            "strings": ["foo", "bar"],
            "mixed": [1, true]
        },
    });

    test!(value: value.clone())
        .assert_snapshot()
        .assert_allows_ser_roundtrip([value]);
}
