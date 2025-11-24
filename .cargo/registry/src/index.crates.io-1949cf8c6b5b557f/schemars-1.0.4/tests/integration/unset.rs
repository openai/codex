use crate::prelude::*;

#[derive(Default, Deserialize, Serialize, JsonSchema)]
#[serde(from = "FlatStruct", rename_all = "kebab-case")]
#[schemars(!from)]
pub struct Struct {
    #[serde(flatten)]
    inner: Inner,
}

#[derive(Default, Deserialize, Serialize, JsonSchema)]
pub struct Inner {
    /// Documentation for field
    pub field: String,
}

#[derive(Deserialize)]
pub struct FlatStruct {
    pub field: String,
}

impl From<FlatStruct> for Struct {
    fn from(value: FlatStruct) -> Self {
        Self {
            inner: Inner { field: value.field },
        }
    }
}

#[test]
fn unset_attributes() {
    test!(Struct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_array,
            "structs with `#derive(Deserialize)` can technically be deserialized from sequences, but that's not intended to be used via JSON, so schemars ignores it",
        ));
}
