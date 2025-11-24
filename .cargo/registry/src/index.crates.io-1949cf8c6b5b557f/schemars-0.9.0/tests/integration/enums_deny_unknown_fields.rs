use crate::prelude::*;
use std::collections::BTreeMap;

macro_rules! fn_values {
    () => {
        fn values() -> impl IntoIterator<Item = Self> {
            [
                Self::Unit,
                Self::StringMap(
                    [("hello".to_owned(), "world".to_owned())]
                        .into_iter()
                        .collect(),
                ),
                Self::StructNewType(Struct {
                    foo: 123,
                    bar: true,
                }),
                Self::StructDenyUnknownFieldsNewType(StructDenyUnknownFields {
                    baz: 123,
                    foobar: true,
                }),
                Self::Struct {
                    foo: 123,
                    bar: true,
                },
            ]
        }
    };
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct Struct {
    foo: i32,
    bar: bool,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct StructDenyUnknownFields {
    baz: i32,
    foobar: bool,
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
enum External {
    Unit,
    StringMap(BTreeMap<String, String>),
    StructNewType(Struct),
    StructDenyUnknownFieldsNewType(StructDenyUnknownFields),
    Struct { foo: i32, bar: bool },
}

impl External {
    fn_values!();
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(tag = "tag", deny_unknown_fields)]
enum Internal {
    Unit,
    StringMap(BTreeMap<String, String>),
    StructNewType(Struct),
    StructDenyUnknownFieldsNewType(StructDenyUnknownFields),
    Struct { foo: i32, bar: bool },
}

impl Internal {
    fn_values!();
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(tag = "tag", content = "content", deny_unknown_fields)]
enum Adjacent {
    Unit,
    StringMap(BTreeMap<String, String>),
    StructNewType(Struct),
    StructDenyUnknownFieldsNewType(StructDenyUnknownFields),
    Struct { foo: i32, bar: bool },
}

impl Adjacent {
    fn_values!();
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
enum Untagged {
    Unit,
    StringMap(BTreeMap<String, String>),
    StructNewType(Struct),
    StructDenyUnknownFieldsNewType(StructDenyUnknownFields),
    Struct { foo: i32, bar: bool },
}

impl Untagged {
    fn_values!();
}

#[test]
fn externally_tagged_enum() {
    test!(External)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(External::values())
        .assert_matches_de_roundtrip(arbitrary_values())
        .assert_rejects_de([
            json!({
                "Struct": {
                    "foo": 123,
                    "bar": true,
                    "extra": null
                }
            }),
            json!({
                "StructDenyUnknownFieldsNewType": {
                    "baz": 123,
                    "foobar": true,
                    "extra": null
                }
            }),
        ])
        .assert_allows_de_roundtrip([json!({
            "StructNewType": {
                "foo": 123,
                "bar": true,
                "extra": null
            }
        })]);
}

#[test]
fn internally_tagged_enum() {
    test!(Internal)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(Internal::values())
        .assert_matches_de_roundtrip(arbitrary_values())
        .assert_rejects_de([
            json!({
                "tag": "Struct",
                "foo": 123,
                "bar": true,
                "extra": null
            }),
            json!({
                "tag": "StructDenyUnknownFieldsNewType",
                "baz": 123,
                "foobar": true,
                "extra": null
            }),
        ])
        .assert_allows_de_roundtrip([json!({
            "tag": "StructNewType",
            "foo": 123,
            "bar": true,
            "extra": null
        })]);
}

#[test]
fn adjacently_tagged_enum() {
    test!(Adjacent)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(Adjacent::values())
        .assert_matches_de_roundtrip(arbitrary_values())
        .assert_rejects_de([
            json!({
                "tag": "Struct",
                "content": {
                    "foo": 123,
                    "bar": true,
                    "extra": null
                }
            }),
            json!({
                "tag": "StructDenyUnknownFieldsNewType",
                "content": {
                    "baz": 123,
                    "foobar": true,
                    "extra": null
                }
            }),
        ])
        .assert_allows_de_roundtrip([json!({
            "tag": "StructNewType",
            "content": {
                "foo": 123,
                "bar": true,
                "extra": null
            }
        })]);
}

#[test]
fn untagged_enum() {
    test!(Untagged)
        .assert_snapshot()
        .assert_allows_ser_roundtrip(Untagged::values())
        .assert_matches_de_roundtrip(arbitrary_values())
        .assert_rejects_de([json!({
            "baz": 123,
            "foobar": true,
            "extra": null
        })])
        .assert_allows_de_roundtrip([json!({
            "foo": 123,
            "bar": true,
            "extra": null
        })]);
}
