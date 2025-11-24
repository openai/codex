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
enum External {
    TaggedUnitOne,
    TaggedStruct {
        baz: i32,
        foobar: bool,
    },
    #[serde(untagged)]
    UnitOne,
    #[serde(untagged)]
    UnitStructNewType(UnitStruct),
    #[serde(untagged)]
    StructNewType(Struct),
    #[serde(untagged)]
    Struct {
        baz: i32,
        foobar: bool,
    },
    #[serde(untagged)]
    Tuple(i32, bool),
    #[serde(untagged)]
    StringMap(BTreeMap<String, String>),
}

impl External {
    fn values() -> impl IntoIterator<Item = Self> {
        [
            Self::TaggedUnitOne,
            Self::TaggedStruct {
                baz: 123,
                foobar: true,
            },
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
                baz: 123,
                foobar: true,
            },
            Self::Tuple(456, false),
        ]
    }
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(tag = "tag")]
enum Internal {
    TaggedUnitOne,
    TaggedStruct {
        baz: i32,
        foobar: bool,
    },
    #[serde(untagged)]
    UnitOne,
    #[serde(untagged)]
    UnitStructNewType(UnitStruct),
    #[serde(untagged)]
    StructNewType(Struct),
    #[serde(untagged)]
    Struct {
        baz: i32,
        foobar: bool,
    },
    // Internally-tagged enums don't support tuple variants
    // #[serde(untagged)]
    // Tuple(i32, bool),
    #[serde(untagged)]
    StringMap(BTreeMap<String, String>),
}

impl Internal {
    fn values() -> impl IntoIterator<Item = Self> {
        [
            Self::TaggedUnitOne,
            Self::TaggedStruct {
                baz: 123,
                foobar: true,
            },
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
                baz: 123,
                foobar: true,
            },
            // Self::Tuple(456, false),
        ]
    }
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(tag = "tag", content = "content")]
enum Adjacent {
    TaggedUnitOne,
    TaggedStruct {
        baz: i32,
        foobar: bool,
    },
    #[serde(untagged)]
    UnitOne,
    #[serde(untagged)]
    UnitStructNewType(UnitStruct),
    #[serde(untagged)]
    StructNewType(Struct),
    #[serde(untagged)]
    Struct {
        baz: i32,
        foobar: bool,
    },
    #[serde(untagged)]
    Tuple(i32, bool),
    #[serde(untagged)]
    StringMap(BTreeMap<String, String>),
}

impl Adjacent {
    fn values() -> impl IntoIterator<Item = Self> {
        [
            Self::TaggedUnitOne,
            Self::TaggedStruct {
                baz: 123,
                foobar: true,
            },
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
                baz: 123,
                foobar: true,
            },
            Self::Tuple(456, false),
        ]
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
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_array,
            "internally tagged enums can technically be deserialized from sequences, but that's not intended to be used via JSON, so schemars ignores it",
        ));
}

#[test]
fn adjacently_tagged_enum() {
    test!(Adjacent)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(Adjacent::values())
        .assert_matches_de_roundtrip(arbitrary_values());
}
