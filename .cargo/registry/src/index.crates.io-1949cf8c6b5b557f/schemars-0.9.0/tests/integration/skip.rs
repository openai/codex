use crate::prelude::*;

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Struct {
    #[serde(skip)]
    _skipped: bool,
    included: bool,
}

#[test]
fn skip_struct_field() {
    test!(Struct)
        .assert_snapshot()
        .assert_allows_de_roundtrip([json!({ "included": true })])
        .assert_rejects_de([json!({
            "_skipped": true,
            "included": true
        })])
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_array,
            "structs with `#derive(Deserialize)` can technically be deserialized from sequences, but that's not intended to be used via JSON, so schemars ignores it",
        ));
}

#[derive(JsonSchema, Deserialize, Serialize)]
pub enum Enum {
    Included1,
    #[serde(skip)]
    _Skipped,
    Included2,
}

#[test]
fn skip_enum_variants() {
    test!(Enum)
        .assert_snapshot()
        .assert_allows_de_roundtrip([json!("Included1"), json!("Included2")])
        .assert_rejects_de([json!("_Skipped")])
        .assert_matches_de_roundtrip(arbitrary_values());
}
