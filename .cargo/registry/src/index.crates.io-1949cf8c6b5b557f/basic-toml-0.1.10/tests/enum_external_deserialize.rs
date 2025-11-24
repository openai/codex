#![allow(clippy::wildcard_imports)]

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Struct {
    value: Enum,
}

#[derive(Debug, Deserialize, PartialEq)]
enum Enum {
    Variant,
}

#[test]
fn unknown_variant() {
    let error = basic_toml::from_str::<Struct>("value = \"NonExistent\"").unwrap_err();

    assert_eq!(
        error.to_string(),
        "unknown variant `NonExistent`, expected `Variant` for key `value` at line 1 column 1"
    );
}

#[test]
fn from_str() {
    let s = basic_toml::from_str::<Struct>("value = \"Variant\"").unwrap();

    assert_eq!(Enum::Variant, s.value);
}
