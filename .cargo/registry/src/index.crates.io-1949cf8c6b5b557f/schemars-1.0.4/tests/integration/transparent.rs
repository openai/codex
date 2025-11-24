use crate::prelude::*;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(transparent)]
pub struct TransparentStruct {
    inner: String,
}

#[test]
fn transparent_struct() {
    test!(TransparentStruct)
        .assert_identical::<String>()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(transparent)]
/// A doc comment
pub struct TransparentStructWithDoc {
    /// Another doc comment (ignored)
    inner: String,
}

#[test]
fn transparent_struct_with_doc() {
    test!(TransparentStructWithDoc)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(transparent)]
pub struct TransparentNewType(String);

#[test]
fn transparent_newtype() {
    test!(TransparentNewType)
        .assert_identical::<String>()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TransparentNewTypeWithValidation(#[schemars(length(min = 1))] String);

#[test]
fn transparent_newtype_with_validation() {
    test!(TransparentNewTypeWithValidation)
        .with_validator(|v| !v.0.is_empty())
        .assert_snapshot()
        .assert_allows_ser_roundtrip([TransparentNewTypeWithValidation("a@a.com".into())])
        .assert_matches_de_roundtrip(arbitrary_values());
}
