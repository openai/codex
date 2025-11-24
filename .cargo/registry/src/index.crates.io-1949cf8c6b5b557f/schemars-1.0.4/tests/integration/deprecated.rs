#![allow(deprecated)]

use crate::prelude::*;
use pretty_assertions::assert_eq;

#[derive(JsonSchema, Default, Serialize, Deserialize)]
#[deprecated]
struct DeprecatedStruct {
    foo: i32,
    #[deprecated]
    bar: bool,
}

#[allow(deprecated)]
#[test]
fn deprecated_struct() {
    test!(DeprecatedStruct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .custom(|schema, _| {
            assert_eq!(
                schema.as_value().pointer("/deprecated"),
                Some(&Value::Bool(true)),
            );
            assert_eq!(
                schema.as_value().pointer("/properties/bar/deprecated"),
                Some(&Value::Bool(true)),
            );
        });
}

#[derive(JsonSchema, Default, Serialize, Deserialize)]
#[deprecated]
enum DeprecatedEnum {
    #[default]
    Unit,
    #[deprecated]
    DeprecatedUnitVariant,
    #[deprecated]
    DeprecatedStructVariant {
        foo: i32,
        #[deprecated]
        deprecated_field: bool,
    },
}

#[test]
fn deprecated_enum() {
    test!(DeprecatedEnum)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .custom(|schema, _| {
            assert_eq!(
                schema.as_value().pointer("/deprecated"),
                Some(&Value::Bool(true)),
            );
            assert_eq!(
                schema.as_value().pointer("/oneOf/1/deprecated"),
                Some(&Value::Bool(true)),
            );
            assert_eq!(
                schema.as_value().pointer("/oneOf/2/deprecated"),
                Some(&Value::Bool(true)),
            );
            assert_eq!(
                schema.as_value().pointer("/oneOf/2/properties/DeprecatedStructVariant/properties/deprecated_field/deprecated"),
                Some(&Value::Bool(true)),
            );
        });
}
