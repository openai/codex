use crate::prelude::*;
use std::collections::BTreeMap;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct Flat {
    f: f32,
    b: bool,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    s: String,
    v: Vec<i32>,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(rename = "Flat")]
struct Deep1 {
    f: f32,
    #[serde(flatten)]
    deep2: Deep2,
    v: Vec<i32>,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct Deep2 {
    b: bool,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    deep3: Option<Deep3>,
}

#[derive(JsonSchema, Deserialize, Serialize)]
struct Deep3 {
    s: String,
}

#[test]
fn flattened_struct() {
    test!(Deep1)
        .assert_snapshot()
        .assert_identical::<Flat>()
        .assert_allows_ser_roundtrip([
            Deep1::default(),
            Deep1 {
                f: 1.0,
                deep2: Deep2 {
                    b: true,
                    deep3: Some(Deep3 {
                        s: "test".to_owned(),
                    }),
                },
                v: vec![123],
            },
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct FlattenValue {
    flag: bool,
    #[serde(flatten)]
    value: Value,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(rename = "FlattenValue")]
struct FlattenMap {
    flag: bool,
    #[serde(flatten)]
    value: BTreeMap<String, Value>,
}

#[test]
fn flattened_value() {
    test!(FlattenValue)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            FlattenValue {
                flag: false,
                value: Value::Null,
            },
            FlattenValue {
                flag: true,
                value: Value::Object(Default::default()),
            },
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn flattened_map() {
    test!(FlattenMap)
        .assert_identical::<FlattenValue>()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}
