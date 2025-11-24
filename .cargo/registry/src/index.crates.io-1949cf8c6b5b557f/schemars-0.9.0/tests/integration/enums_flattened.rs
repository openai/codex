use crate::prelude::*;
use schemars::generate::SchemaSettings;

macro_rules! fn_values {
    () => {
        fn values() -> impl IntoIterator<Item = Self> {
            [
                Self {
                    f: 1.23,
                    e1: Enum1::B(true),
                    e2: Enum2::F(4.56),
                    e3: Enum3::S2("abc".into()),
                    e4: Enum4::U2(789),
                    e5: Enum5::B3(false),
                },
                Self {
                    f: 9.87,
                    e1: Enum1::S("def".into()),
                    e2: Enum2::U(654),
                    e3: Enum3::B2(true),
                    e4: Enum4::F2(3.21),
                    e5: Enum5::S3("ghi".into()),
                },
            ]
        }
    };
}

#[derive(JsonSchema, Deserialize, Serialize)]
enum Enum1 {
    B(bool),
    S(String),
}

#[derive(JsonSchema, Deserialize, Serialize)]
enum Enum2 {
    U(u32),
    F(f64),
}

#[derive(JsonSchema, Deserialize, Serialize)]
enum Enum3 {
    B2(bool),
    S2(String),
}

#[derive(JsonSchema, Deserialize, Serialize)]
enum Enum4 {
    U2(u32),
    F2(f64),
}

#[derive(JsonSchema, Deserialize, Serialize)]
enum Enum5 {
    B3(bool),
    S3(String),
}

#[derive(JsonSchema, Deserialize, Serialize)]
struct Container {
    f: f32,
    #[serde(flatten)]
    e1: Enum1,
    #[serde(flatten)]
    e2: Enum2,
    #[serde(flatten)]
    e3: Enum3,
    #[serde(flatten)]
    e4: Enum4,
    #[serde(flatten)]
    e5: Enum5,
}

impl Container {
    fn_values!();
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ContainerDenyUnknownFields {
    f: f32,
    #[serde(flatten)]
    e1: Enum1,
    #[serde(flatten)]
    e2: Enum2,
    #[serde(flatten)]
    e3: Enum3,
    #[serde(flatten)]
    e4: Enum4,
    #[serde(flatten)]
    e5: Enum5,
}

impl ContainerDenyUnknownFields {
    fn_values!();
}

fn json_with_extra_field() -> Value {
    json!({
      "f": 1.23,
      "B": true,
      "F": 4.56,
      "S2": "abc",
      "U2": 789,
      "B3": false,
      "extra": null
    })
}

#[test]
fn enums_flattened() {
    test!(Container)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(Container::values())
        .assert_matches_de_roundtrip(arbitrary_values())
        .assert_allows_de_roundtrip([json_with_extra_field()]);
}

#[test]
fn enums_flattened_deny_unknown_fields() {
    test!(ContainerDenyUnknownFields)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(ContainerDenyUnknownFields::values())
        .assert_matches_de_roundtrip(arbitrary_values())
        .assert_rejects_de([json_with_extra_field()]);
}

#[test]
fn enums_flattened_deny_unknown_fields_draft07() {
    test!(ContainerDenyUnknownFields, SchemaSettings::draft07())
        .assert_snapshot()
        .assert_allows_ser_roundtrip(ContainerDenyUnknownFields::values())
        .assert_matches_de_roundtrip(arbitrary_values())
        .assert_rejects_de([json_with_extra_field()]);
}
