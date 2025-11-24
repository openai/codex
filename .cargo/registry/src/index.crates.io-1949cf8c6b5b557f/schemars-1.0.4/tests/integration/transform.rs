use crate::prelude::*;
use pretty_assertions::assert_eq;
use schemars::{
    transform::{RecursiveTransform, Transform},
    Schema,
};
use serde_json::Map;

fn insert_upper_type(schema: &mut Schema) {
    if let Some(Value::String(ty)) = schema.get("type") {
        schema.insert("x-upperType".to_owned(), ty.to_uppercase().into());
    }
}

fn insert_property_count(schema: &mut Schema) {
    let count = schema
        .get("properties")
        .and_then(Value::as_object)
        .map_or(0, Map::len);
    schema.insert("x-propertyCount".to_owned(), count.into());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(transform = RecursiveTransform(insert_upper_type), transform = insert_property_count)]
struct Struct {
    value: Value,
    #[schemars(transform = insert_property_count)]
    int: i32,
}

#[test]
fn transform_struct() {
    test!(Struct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .custom(assert_upper_type_valid);
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(transform = RecursiveTransform(insert_upper_type), transform = insert_property_count)]
enum External {
    #[default]
    #[schemars(transform = insert_property_count)]
    Unit,
    #[schemars(transform = insert_property_count)]
    NewType(Value),
}

#[test]
fn transform_enum() {
    test!(External)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .custom(assert_upper_type_valid);
}

fn assert_upper_type_valid(schema: &Schema, _: schemars::generate::Contract) {
    let mut schema = schema.clone();

    RecursiveTransform(|s: &mut Schema| {
        assert_eq!(
            s.remove("x-upperType").map(|v| v.to_string()),
            s.get("type").map(|v| v.to_string().to_uppercase()),
        );
    })
    .transform(&mut schema);

    assert!(!schema.to_value().to_string().contains("x-upperType"));
}
