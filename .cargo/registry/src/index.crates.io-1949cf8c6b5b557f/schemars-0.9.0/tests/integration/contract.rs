use crate::prelude::*;

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(rename_all(serialize = "SCREAMING-KEBAB-CASE"), deny_unknown_fields)]
struct StructDenyUnknownFields {
    #[serde(skip_deserializing)]
    read_only: bool,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    write_only: bool,
    #[serde(default)]
    default: bool,
    #[serde(skip_serializing_if = "core::ops::Not::not")]
    skip_serializing_if: bool,
    #[serde(rename(serialize = "ser_renamed", deserialize = "de_renamed"))]
    renamed: bool,
    option: Option<bool>,
}

#[derive(JsonSchema, Deserialize, Serialize)]
struct StructAllowUnknownFields {
    #[serde(flatten)]
    inner: StructDenyUnknownFields,
}

#[test]
fn struct_deny_unknown_fields() {
    test!(StructDenyUnknownFields)
        .assert_snapshot()
        .assert_allows_de_roundtrip([
            json!({ "write_only": false, "skip_serializing_if": false, "de_renamed": false }),
            json!({ "write_only": true, "skip_serializing_if": true, "de_renamed": true, "default": true }),
            json!({ "write_only": true, "skip_serializing_if": true, "de_renamed": true, "option": true }),
        ])
        .assert_rejects_de([
            json!({ "skip_serializing_if": false, "de_renamed": false }),
            json!({ "write_only": false, "de_renamed": false }),
            json!({ "write_only": false, "skip_serializing_if": false }),
            json!({ "write_only": true, "skip_serializing_if": true, "de_renamed": true, "unknown": true }),
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn struct_allow_unknown_fields() {
    test!(StructAllowUnknownFields)
        .assert_snapshot()
        .assert_allows_de_roundtrip([
            json!({ "write_only": false, "skip_serializing_if": false, "de_renamed": false }),
            json!({ "write_only": true, "skip_serializing_if": true, "de_renamed": true, "default": true }),
            json!({ "write_only": true, "skip_serializing_if": true, "de_renamed": true, "option": true }),
            json!({ "write_only": true, "skip_serializing_if": true, "de_renamed": true, "unknown": true }),
        ])
        .assert_rejects_de([
            json!({ "skip_serializing_if": false, "de_renamed": false }),
            json!({ "write_only": false, "de_renamed": false }),
            json!({ "write_only": false, "skip_serializing_if": false }),
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize)]
struct TupleStruct(
    String,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    bool,
    String,
    #[serde(skip_deserializing)] bool,
    String,
);

#[test]
fn tuple_struct() {
    test!(TupleStruct)
        .assert_snapshot()
        .assert_allows_de_roundtrip([json!(["", true, "", ""])])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[allow(dead_code)]
#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(
    rename_all(serialize = "SCREAMING-KEBAB-CASE"),
    rename_all_fields(serialize = "PascalCase")
)]
enum ExternalEnum {
    #[serde(skip_deserializing)]
    ReadOnlyUnit,
    #[serde(skip_serializing)]
    WriteOnlyUnit,
    #[serde(skip_deserializing)]
    ReadOnlyStruct { s: String },
    #[serde(skip_serializing)]
    WriteOnlyStruct { i: isize },
    #[serde(rename(serialize = "ser_renamed_unit", deserialize = "de_renamed_unit"))]
    RenamedUnit,
    #[serde(rename(serialize = "ser_renamed_struct", deserialize = "de_renamed_struct"))]
    RenamedStruct { b: bool },
}

#[test]
fn externally_tagged_enum() {
    test!(ExternalEnum)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            ExternalEnum::ReadOnlyUnit,
            ExternalEnum::ReadOnlyStruct { s: "test".into() },
            ExternalEnum::RenamedUnit,
            ExternalEnum::RenamedStruct { b: true },
        ])
        .assert_allows_de_roundtrip([
            json!("WriteOnlyUnit"),
            json!({ "WriteOnlyStruct": { "i": 123 } }),
            json!("de_renamed_unit"),
            json!({ "de_renamed_struct": { "b": true } }),
        ])
        .assert_rejects_de([
            json!("READ-ONLY-UNIT"),
            json!("ReadOnlyUnit"),
            json!("ser_renamed_unit"),
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[allow(dead_code)]
#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(
    tag = "tag",
    rename_all(serialize = "SCREAMING-KEBAB-CASE"),
    rename_all_fields(serialize = "PascalCase")
)]
enum InternalEnum {
    #[serde(skip_deserializing)]
    ReadOnlyUnit,
    #[serde(skip_serializing)]
    WriteOnlyUnit,
    #[serde(skip_deserializing)]
    ReadOnlyStruct { s: String },
    #[serde(skip_serializing)]
    WriteOnlyStruct { i: isize },
    #[serde(rename(serialize = "ser_renamed_unit", deserialize = "de_renamed_unit"))]
    RenamedUnit,
    #[serde(rename(serialize = "ser_renamed_struct", deserialize = "de_renamed_struct"))]
    RenamedStruct { b: bool },
}

#[test]
fn internally_tagged_enum() {
    test!(InternalEnum)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            InternalEnum::ReadOnlyUnit,
            InternalEnum::ReadOnlyStruct { s: "test".into() },
            InternalEnum::RenamedUnit,
            InternalEnum::RenamedStruct { b: true },
        ])
        .assert_allows_de_roundtrip([
            json!({ "tag": "WriteOnlyUnit" }),
            json!({ "tag": "WriteOnlyStruct", "i": 123 }),
            json!({ "tag": "de_renamed_unit" }),
            json!({ "tag": "de_renamed_struct", "b": true }),
        ])
        .assert_rejects_de([
            json!({ "tag": "READ-ONLY-UNIT" }),
            json!({ "tag": "ReadOnlyUnit" }),
            json!({ "tag": "ser_renamed_unit" }),
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[allow(dead_code)]
#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(
    tag = "tag",
    content = "content",
    rename_all(serialize = "SCREAMING-KEBAB-CASE"),
    rename_all_fields(serialize = "PascalCase")
)]
enum AdjacentEnum {
    #[serde(skip_deserializing)]
    ReadOnlyUnit,
    #[serde(skip_serializing)]
    WriteOnlyUnit,
    #[serde(skip_deserializing)]
    ReadOnlyStruct { s: String },
    #[serde(skip_serializing)]
    WriteOnlyStruct { i: isize },
    #[serde(rename(serialize = "ser_renamed_unit", deserialize = "de_renamed_unit"))]
    RenamedUnit,
    #[serde(rename(serialize = "ser_renamed_struct", deserialize = "de_renamed_struct"))]
    RenamedStruct { b: bool },
}

#[test]
fn adjacently_tagged_enum() {
    test!(AdjacentEnum)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            AdjacentEnum::ReadOnlyUnit,
            AdjacentEnum::ReadOnlyStruct { s: "test".into() },
            AdjacentEnum::RenamedUnit,
            AdjacentEnum::RenamedStruct { b: true },
        ])
        .assert_allows_de_roundtrip([
            json!({ "tag": "WriteOnlyUnit" }),
            json!({ "tag": "WriteOnlyStruct", "content": { "i": 123 } }),
            json!({ "tag": "de_renamed_unit" }),
            json!({ "tag": "de_renamed_struct", "content": { "b": true } }),
        ])
        .assert_rejects_de([
            json!({ "tag": "READ-ONLY-UNIT" }),
            json!({ "tag": "ReadOnlyUnit" }),
            json!({ "tag": "ser_renamed_unit" }),
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[allow(dead_code)]
#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(
    untagged,
    rename_all(serialize = "SCREAMING-KEBAB-CASE"),
    rename_all_fields(serialize = "PascalCase")
)]
enum UntaggedEnum {
    #[serde(skip_deserializing)]
    ReadOnlyUnit,
    #[serde(skip_serializing)]
    WriteOnlyUnit,
    #[serde(skip_deserializing)]
    ReadOnlyStruct { s: String },
    #[serde(skip_serializing)]
    WriteOnlyStruct { i: isize },
    #[serde(rename(serialize = "ser_renamed_unit", deserialize = "de_renamed_unit"))]
    RenamedUnit,
    #[serde(rename(serialize = "ser_renamed_struct", deserialize = "de_renamed_struct"))]
    RenamedStruct { b: bool },
}

#[test]
fn untagged_enum() {
    test!(UntaggedEnum)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            UntaggedEnum::ReadOnlyUnit,
            UntaggedEnum::ReadOnlyStruct { s: "test".into() },
            UntaggedEnum::RenamedUnit,
            UntaggedEnum::RenamedStruct { b: true },
        ])
        .assert_allows_de_roundtrip([json!(null), json!({ "i": 123 }), json!({ "b": true })])
        .assert_rejects_de([json!({ "s": "test" })])
        .assert_matches_de_roundtrip(arbitrary_values());
}
