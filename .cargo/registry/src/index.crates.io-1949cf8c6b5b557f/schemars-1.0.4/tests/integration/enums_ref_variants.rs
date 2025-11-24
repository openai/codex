use crate::prelude::*;
use std::collections::BTreeMap;

#[derive(JsonSchema, Deserialize, Serialize)]
struct UnitStruct;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct Struct {
    foo: i32,
    bar: bool,
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[schemars(_unstable_ref_variants)]
enum External {
    UnitOne,
    StringMap(BTreeMap<String, String>),
    UnitStructNewType(UnitStruct),
    StructNewType(Struct),
    Struct {
        foo: i32,
        bar: bool,
    },
    Tuple(i32, bool),
    UnitTwo,
    #[serde(with = "unit_variant_as_u64")]
    #[schemars(with = "u64")]
    UnitAsInt,
    #[serde(with = "tuple_variant_as_str")]
    #[schemars(schema_with = "tuple_variant_as_str::json_schema")]
    TupleAsStr(i32, bool),
}

impl External {
    fn values() -> impl IntoIterator<Item = Self> {
        [
            Self::UnitOne,
            Self::StringMap(
                [("hello".to_owned(), "world".to_owned())]
                    .into_iter()
                    .collect(),
            ),
            Self::UnitStructNewType(UnitStruct),
            Self::StructNewType(Struct {
                foo: 123,
                bar: true,
            }),
            Self::Struct {
                foo: 123,
                bar: true,
            },
            Self::Tuple(456, false),
            Self::UnitTwo,
            Self::UnitAsInt,
            Self::TupleAsStr(789, true),
        ]
    }
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(tag = "tag")]
#[schemars(_unstable_ref_variants)]
enum Internal {
    UnitOne,
    StringMap(BTreeMap<String, String>),
    UnitStructNewType(UnitStruct),
    StructNewType(Struct),
    Struct { foo: i32, bar: bool },
    // Internally-tagged enums don't support tuple variants
    //  Tuple(i32, bool),
    UnitTwo,
    // Internally-tagged enum variants don't support non-object "payloads"
    //  #[serde(with = "unit_variant_as_u64")]
    //  #[schemars(with = "u64")]
    //  UnitAsInt,
    // Internally-tagged enums don't support tuple variants
    //  #[serde(with = "tuple_variant_as_str")]
    //  #[schemars(schema_with = "tuple_variant_as_str::json_schema")]
    //  TupleAsStr(i32, bool),
}

impl Internal {
    fn values() -> impl IntoIterator<Item = Self> {
        [
            Self::UnitOne,
            Self::StringMap(
                [("hello".to_owned(), "world".to_owned())]
                    .into_iter()
                    .collect(),
            ),
            Self::UnitStructNewType(UnitStruct),
            Self::StructNewType(Struct {
                foo: 123,
                bar: true,
            }),
            Self::Struct {
                foo: 123,
                bar: true,
            },
            // Self::Tuple(456, false),
            Self::UnitTwo,
            // Self::UnitAsInt,
            // Self::TupleAsStr(789, true),
        ]
    }
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(tag = "tag", content = "content")]
#[schemars(_unstable_ref_variants)]
enum Adjacent {
    UnitOne,
    StringMap(BTreeMap<String, String>),
    UnitStructNewType(UnitStruct),
    StructNewType(Struct),
    Struct {
        foo: i32,
        bar: bool,
    },
    Tuple(i32, bool),
    UnitTwo,
    #[serde(with = "unit_variant_as_u64")]
    #[schemars(with = "u64")]
    UnitAsInt,
    #[serde(with = "tuple_variant_as_str")]
    #[schemars(schema_with = "tuple_variant_as_str::json_schema")]
    TupleAsStr(i32, bool),
}

impl Adjacent {
    fn values() -> impl IntoIterator<Item = Self> {
        [
            Self::UnitOne,
            Self::StringMap(
                [("hello".to_owned(), "world".to_owned())]
                    .into_iter()
                    .collect(),
            ),
            Self::UnitStructNewType(UnitStruct),
            Self::StructNewType(Struct {
                foo: 123,
                bar: true,
            }),
            Self::Struct {
                foo: 123,
                bar: true,
            },
            Self::Tuple(456, false),
            Self::UnitTwo,
            Self::UnitAsInt,
            Self::TupleAsStr(789, true),
        ]
    }
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(untagged)]
#[schemars(_unstable_ref_variants)]
enum Untagged {
    UnitOne,
    StringMap(BTreeMap<String, String>),
    UnitStructNewType(UnitStruct),
    StructNewType(Struct),
    Struct {
        foo: i32,
        bar: bool,
    },
    Tuple(i32, bool),
    UnitTwo,
    #[serde(with = "unit_variant_as_u64")]
    #[schemars(with = "u64")]
    UnitAsInt,
    #[serde(with = "tuple_variant_as_str")]
    #[schemars(schema_with = "tuple_variant_as_str::json_schema")]
    TupleAsStr(i32, bool),
}

impl Untagged {
    fn values() -> impl IntoIterator<Item = Self> {
        [
            Self::UnitOne,
            Self::StringMap(
                [("hello".to_owned(), "world".to_owned())]
                    .into_iter()
                    .collect(),
            ),
            Self::UnitStructNewType(UnitStruct),
            Self::StructNewType(Struct {
                foo: 123,
                bar: true,
            }),
            Self::Struct {
                foo: 123,
                bar: true,
            },
            Self::Tuple(456, false),
            Self::UnitTwo,
            Self::UnitAsInt,
            Self::TupleAsStr(789, true),
        ]
    }
}

mod unit_variant_as_u64 {
    pub(super) fn serialize<S>(ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ser.serialize_u64(42)
    }

    pub(super) fn deserialize<'de, D>(deser: D) -> Result<(), D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Deserialize;

        u64::deserialize(deser).map(|_| ())
    }
}

mod tuple_variant_as_str {
    pub(super) fn serialize<S>(i: &i32, b: &bool, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ser.collect_str(&format_args!("{i} {b}"))
    }

    pub(super) fn deserialize<'de, D>(deser: D) -> Result<(i32, bool), D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Deserialize, Error};
        let error = || Error::custom("invalid string");

        let (i, b) = <&str>::deserialize(deser)?
            .split_once(' ')
            .ok_or_else(error)?;

        Ok((
            i.parse().map_err(|_| error())?,
            b.parse().map_err(|_| error())?,
        ))
    }

    pub(super) fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": r"^\d+ (true|false)$"
        })
    }
}

#[test]
fn externally_tagged_enum() {
    test!(External)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(External::values())
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn internally_tagged_enum() {
    test!(Internal)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(Internal::values())
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn adjacently_tagged_enum() {
    test!(Adjacent)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(Adjacent::values())
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn untagged_enum() {
    test!(Untagged)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(Untagged::values())
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Serialize, Deserialize)]
#[schemars(_unstable_ref_variants)]
enum NoVariants {}

#[test]
fn no_variants() {
    test!(NoVariants)
        .assert_snapshot()
        .assert_rejects_de(arbitrary_values());
}

#[derive(JsonSchema, Serialize, Deserialize)]
#[serde(rename_all_fields = "UPPERCASE", rename_all = "snake_case")]
#[schemars(_unstable_ref_variants)]
enum Renamed {
    StructVariant {
        field: String,
    },
    #[serde(rename = "custom name variant")]
    RenamedStructVariant {
        #[serde(rename = "custom name field")]
        field: String,
    },
}

#[test]
fn renamed() {
    test!(Renamed)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            Renamed::StructVariant {
                field: "foo".to_owned(),
            },
            Renamed::RenamedStructVariant {
                field: "bar".to_owned(),
            },
        ])
        .assert_rejects_de(arbitrary_values());
}
