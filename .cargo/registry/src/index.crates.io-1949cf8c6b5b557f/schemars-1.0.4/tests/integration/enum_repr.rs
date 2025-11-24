use crate::prelude::*;
use schemars::JsonSchema_repr;
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(JsonSchema_repr, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
#[serde(rename = "EnumWithReprAttr")]
/// Description from comment
pub enum Enum {
    Zero,
    One,
    Five = 5,
    Six,
    Three = 3,
}

#[test]
fn enum_repr() {
    test!(Enum)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([Enum::Zero, Enum::One, Enum::Five, Enum::Six, Enum::Three])
        .assert_allows_de_roundtrip([
            Value::from(0),
            Value::from(1),
            Value::from(5),
            Value::from(6),
            Value::from(3),
        ])
        .assert_rejects_de([
            Value::from("Zero"),
            Value::from("One"),
            Value::from("Five"),
            Value::from("Six"),
            Value::from("Three"),
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}
