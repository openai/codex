use crate::prelude::*;
use schemars::generate::SchemaSettings;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct Struct<T: Default + Serialize> {
    #[serde(with = "int_as_str")]
    #[schemars(schema_with = "int_as_str::json_schema")]
    x: i64,
    #[schemars(schema_with = "from_serialize_default::<T>")]
    t: T,
}

mod int_as_str {
    pub(super) fn serialize<S, T>(value: &T, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: std::fmt::Display,
    {
        ser.collect_str(value)
    }

    pub(super) fn deserialize<'de, D, T>(deser: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: std::str::FromStr<Err = std::num::ParseIntError>,
    {
        <&str as serde::Deserialize>::deserialize(deser)?
            .parse()
            .map_err(serde::de::Error::custom)
    }

    pub(super) fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": r"^-?\d+$"
        })
    }
}

fn from_serialize_default<T: Default + Serialize>(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    generator
        .settings()
        .clone()
        .into_generator()
        .into_root_schema_for_value(&T::default())
        .unwrap()
}

#[test]
fn field_schema_with() {
    test!(Struct<String>)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_array,
            "structs with `#derive(Deserialize)` can technically be deserialized from sequences, but that's not intended to be used via JSON, so schemars ignores it",
        ));
}

struct NotJsonSchema;

#[derive(JsonSchema)]
#[schemars(with = "Struct<String>")]
struct Struct2 {
    _x: NotJsonSchema,
}

#[test]
fn container_with() {
    test!(Struct2).assert_identical::<Struct<String>>();
}

#[derive(JsonSchema)]
#[schemars(with = "Struct<String>", description = "Testing...")]
struct Struct3 {
    _x: NotJsonSchema,
}

#[test]
fn container_with_metadata() {
    test!(Struct3).assert_snapshot().custom(|schema, _| {
        assert_eq!(schema.get("description"), Some(&"Testing...".into()));
    });

    assert_ne!(<Struct2>::schema_id(), <Struct3>::schema_id());
}

#[derive(JsonSchema)]
#[schemars(schema_with = "int_as_str::json_schema")]
struct IntAsString {
    _x: NotJsonSchema,
}

#[test]
fn container_schema_with() {
    test!(IntAsString)
        .assert_snapshot()
        .custom(|schema, contract| {
            let mut generator = SchemaSettings::default()
                .with(|s| s.contract = contract)
                .into_generator();
            let expected = int_as_str::json_schema(&mut generator);

            // `title` and `$schema` are added for root schemas - ignore for comparison purposes
            let mut schema = schema.clone();
            schema.remove("title");
            schema.remove("$schema");

            assert_eq!(schema, expected);
        });
}
