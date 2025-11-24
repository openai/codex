use crate::prelude::*;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct UnitStruct;

#[test]
fn unit() {
    test!(UnitStruct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct NormalStruct {
    foo: String,
    bar: bool,
}

#[test]
fn normal() {
    test!(NormalStruct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_allows_de_roundtrip([json!({
            "foo": "",
            "bar": true,
            "unknown": 123
        })])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct NewType(String);

#[test]
fn newtype() {
    test!(NewType)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct TupleStruct(String, bool);

#[test]
fn tuple() {
    test!(TupleStruct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct RenamedFields {
    camel_case: i32,
    #[serde(rename = "new_name")]
    old_name: i32,
}

#[test]
fn renamed_fields() {
    test!(RenamedFields)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct DenyUnknownFields {
    foo: String,
    bar: bool,
}

#[test]
fn deny_unknown_fields() {
    test!(DenyUnknownFields)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_rejects_de([json!({
            "foo": "",
            "bar": true,
            "unknown": 123
        })])
        .assert_matches_de_roundtrip(arbitrary_values());
}
