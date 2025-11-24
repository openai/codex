mod util;
use schemars::r#gen::{SchemaGenerator, SchemaSettings};
use serde::Serialize;
use std::collections::HashMap;
use util::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MyStruct {
    pub my_int: i32,
    pub my_bool: bool,
    pub my_nullable_enum: Option<MyEnum>,
    pub my_inner_struct: MyInnerStruct,
    #[serde(skip)]
    pub skip: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_if_none: Option<MyEnum>,
}

#[derive(Serialize)]
pub struct MyInnerStruct {
    pub my_map: HashMap<String, f64>,
    pub my_vec: Vec<&'static str>,
    pub my_empty_map: HashMap<String, f64>,
    pub my_empty_vec: Vec<&'static str>,
    pub my_tuple: (char, u8),
}

#[derive(Serialize)]
pub enum MyEnum {
    StringNewType(String),
    StructVariant { floats: Vec<f32> },
}

fn make_value() -> MyStruct {
    let mut value = MyStruct {
        my_int: 123,
        my_bool: true,
        my_nullable_enum: None,
        my_inner_struct: MyInnerStruct {
            my_map: HashMap::new(),
            my_vec: vec!["hello", "world"],
            my_empty_map: HashMap::new(),
            my_empty_vec: vec![],
            my_tuple: ('💩', 42),
        },
        skip: 123,
        skip_if_none: None,
    };
    value.my_inner_struct.my_map.insert(String::new(), 0.0);
    value
}

#[test]
fn schema_from_value_matches_draft07() -> TestResult {
    let generator = SchemaSettings::draft07().into_generator();
    let actual = generator.into_root_schema_for_value(&make_value())?;

    test_schema(&actual, "from_value_draft07")
}

#[test]
fn schema_from_value_matches_2019_09() -> TestResult {
    let generator = SchemaSettings::draft2019_09().into_generator();
    let actual = generator.into_root_schema_for_value(&make_value())?;

    test_schema(&actual, "from_value_2019_09")
}

#[test]
fn schema_from_value_matches_openapi3() -> TestResult {
    let generator = SchemaSettings::openapi3().into_generator();
    let actual = generator.into_root_schema_for_value(&make_value())?;

    test_schema(&actual, "from_value_openapi3")
}

#[test]
fn schema_from_json_value() -> TestResult {
    let generator = SchemaGenerator::default();
    let actual = generator.into_root_schema_for_value(&serde_json::json!({
        "zero": 0,
        "one": 1,
        "minusOne": -1,
        "zeroPointZero": 0.0,
        "bool": true,
        "null": null,
        "object": {
            "array": ["foo", "bar"]
        },
    }))?;

    test_schema(&actual, "from_json_value")
}
