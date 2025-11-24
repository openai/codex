use crate::prelude::*;
use pretty_assertions::assert_eq;
use schemars::{generate::SchemaSettings, Schema, SchemaGenerator};
use std::any::type_name;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(inline)]
pub struct OuterStruct {
    #[schemars(extend("examples" = [8, null]))]
    maybe_int: Option<i32>,
    values: serde_json::Map<String, Value>,
    value: Value,
    inner: InnerEnum,
    maybe_inner: Option<InnerEnum>,
    tuples: Vec<(u8, i64)>,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
pub enum InnerEnum {
    #[default]
    UndocumentedUnit1,
    UndocumentedUnit2,
    /// This is a documented unit variant
    DocumentedUnit,
    ValueNewType(Value),
}

#[test]
fn draft07() {
    test!(OuterStruct, SchemaSettings::draft07())
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn draft2019_09() {
    test!(OuterStruct, SchemaSettings::draft2019_09())
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn draft2020_12() {
    test!(OuterStruct, SchemaSettings::draft2020_12())
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn openapi3() {
    let mut settings = SchemaSettings::openapi3();
    // Hack to apply recursive transforms to schemas at components.schemas:
    // First, move them to $defs, then run the transforms, then move them back again.
    settings.transforms.insert(
        0,
        Box::new(|s: &mut Schema| {
            let obj = s.ensure_object();
            let defs = obj["components"]["schemas"].take();
            obj.insert("$defs".to_owned(), defs);
        }),
    );
    settings.transforms.push(Box::new(|s: &mut Schema| {
        let obj = s.ensure_object();
        obj["components"]["schemas"] = obj.remove("$defs").unwrap();
    }));

    test!(OuterStruct, settings.clone()).assert_snapshot();

    // Ensure that `take_definitions()` applies transforms correctly

    let gen1 = settings.into_generator();
    let definitions1 =
        &Value::from(gen1.into_root_schema_for::<OuterStruct>())["components"]["schemas"];

    let mut gen2 = SchemaSettings::openapi3().into_generator();
    gen2.subschema_for::<OuterStruct>();
    let definitions2 = Value::Object(gen2.take_definitions(true));

    assert_eq!(definitions1, &definitions2);
}

#[test]
fn include_type_name() {
    test!(
        OuterStruct,
        SchemaSettings::default().with(|s| s.include_type_name = true)
    )
    .assert_snapshot();
}

#[test]
fn include_type_name_handles_with_attributes() {
    fn dummy(_: &mut SchemaGenerator) -> Schema {
        Schema::default()
    }

    #[derive(JsonSchema)]
    struct Foo {
        _int: i32,
        _value: Value,
        #[schemars(with = "f64")]
        _with_type: i32,
        #[schemars(schema_with = "dummy")]
        _with_fn: i32,
    }

    let schema = SchemaSettings::default()
        .with(|s| s.include_type_name = true)
        .into_generator()
        .into_root_schema_for::<Foo>();

    assert_eq!(schema.as_value()["x-rust-type"], type_name::<Foo>());
    assert_eq!(
        schema
            .as_value()
            .pointer("/properties/_int/x-rust-type")
            .and_then(Value::as_str),
        Some(type_name::<i32>())
    );
    assert_eq!(
        schema
            .as_value()
            .pointer("/properties/_value/x-rust-type")
            .and_then(Value::as_str),
        Some(type_name::<Value>())
    );
    assert_eq!(
        schema
            .as_value()
            .pointer("/properties/_with_type/x-rust-type")
            .and_then(Value::as_str),
        Some(type_name::<f64>())
    );
    assert_eq!(
        schema
            .as_value()
            .pointer("/properties/_with_fn/x-rust-type"),
        None,
    );
}
