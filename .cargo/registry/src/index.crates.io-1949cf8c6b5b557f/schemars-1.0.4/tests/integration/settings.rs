use crate::prelude::*;
use pretty_assertions::assert_eq;
use schemars::{generate::SchemaSettings, Schema};

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
            let defs = s.pointer_mut("/components/schemas").unwrap().take();
            s.insert("$defs".to_owned(), defs);
        }),
    );
    settings.transforms.push(Box::new(|s: &mut Schema| {
        *s.pointer_mut("/components/schemas").unwrap() = s.remove("$defs").unwrap();
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
