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
pub struct TransparentNewType(String);

#[test]
fn transparent_newtype() {
    test!(TransparentNewType)
        .assert_identical::<String>()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}
