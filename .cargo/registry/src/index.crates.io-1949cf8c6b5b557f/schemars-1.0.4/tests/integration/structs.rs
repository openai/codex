use crate::prelude::*;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct UnitStruct;

#[test]
fn unit() {
    test!(UnitStruct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct NormalStruct {
    foo: String,
    bar: bool,
}

#[test]
fn normal() {
    test!(NormalStruct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_allows_de_roundtrip([json!({
            "foo": "",
            "bar": true,
            "unknown": 123
        })])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct NewType(String);

#[test]
fn newtype() {
    test!(NewType)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct TupleStruct(String, bool);

#[test]
fn tuple() {
    test!(TupleStruct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct RenamedFields {
    camel_case: i32,
    #[serde(rename = "new_name")]
    old_name: i32,
}

#[test]
fn renamed_fields() {
    test!(RenamedFields)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct DenyUnknownFields {
    foo: String,
    bar: bool,
}

#[test]
fn deny_unknown_fields() {
    test!(DenyUnknownFields)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_rejects_de([json!({
            "foo": "",
            "bar": true,
            "unknown": 123
        })])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(example = PropertyOrder::default(), extend("x-custom" = PropertyOrder::default()))]
struct PropertyOrder {
    #[serde(rename = "$defs")]
    pub defs: UnitStruct,
    pub examples: UnitStruct,
    pub properties: UnitStruct,
    pub required: UnitStruct,
    #[serde(rename = "$schema")]
    pub schema: UnitStruct,
    pub title: UnitStruct,
    pub r#type: UnitStruct,
    #[serde(rename = "x-custom")]
    pub x_custom: UnitStruct,
}

#[cfg_attr(not(feature = "preserve_order"), ignore)]
#[test]
fn property_order() {
    test!(PropertyOrder).assert_snapshot().custom(|schema, _| {
        fn get_property_keys(value: &Value) -> Vec<&str> {
            value
                .as_object()
                .expect("expected value to be an object")
                .keys()
                .map(String::as_str)
                .collect()
        }

        let value = serde_json::to_value(schema).unwrap();

        assert!(matches!(
            get_property_keys(&value).as_slice(),
            // order of `examples`, `required` and `x-custom` is unspecified
            &["$schema", "title", "type", "properties", .., "$defs"]
        ));

        let field_order = &[
            "$defs",
            "examples",
            "properties",
            "required",
            "$schema",
            "title",
            "type",
            "x-custom",
        ];

        assert_eq!(
            get_property_keys(&value["properties"]).as_slice(),
            field_order,
        );
        assert_eq!(
            get_property_keys(&value["examples"][0]).as_slice(),
            field_order,
        );
        assert_eq!(
            get_property_keys(&value["x-custom"]).as_slice(),
            field_order,
        );
    });
}
