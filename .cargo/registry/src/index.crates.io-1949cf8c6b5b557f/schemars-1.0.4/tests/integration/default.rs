use crate::prelude::*;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(default)]
struct MyStruct {
    integer: u32,
    boolean: bool,
    option_string: Option<String>,
    #[serde(skip_serializing_if = "str::is_empty")]
    string_skip_empty: String,
    #[serde(with = "struct_2_as_str")]
    #[schemars(with = "str", pattern(r"^\d+ (true|false)$"))]
    struct2: MyStruct2,
    #[serde(skip_serializing)]
    not_serialize: NotSerialize,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[serde(default = "ten_and_true")]
struct MyStruct2 {
    #[serde(default = "six")]
    integer: u32,
    boolean: bool,
}

#[allow(dead_code)]
#[derive(JsonSchema, Deserialize, Default)]
struct NotSerialize(i8);

mod struct_2_as_str {
    use super::MyStruct2;

    pub(super) fn serialize<S>(value: &MyStruct2, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ser.collect_str(&format_args!("{} {}", value.integer, value.boolean))
    }

    pub(super) fn deserialize<'de, D>(deser: D) -> Result<MyStruct2, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Deserialize, Error};
        let error = || Error::custom("invalid string");

        let (i, b) = <&str>::deserialize(deser)?
            .split_once(' ')
            .ok_or_else(error)?;

        Ok(MyStruct2 {
            integer: i.parse().map_err(|_| error())?,
            boolean: b.parse().map_err(|_| error())?,
        })
    }
}

fn ten_and_true() -> MyStruct2 {
    MyStruct2 {
        integer: 10,
        boolean: true,
    }
}

fn six() -> u32 {
    6
}

#[test]
fn default_fields() {
    test!(MyStruct)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            MyStruct::default(),
            MyStruct {
                integer: 123,
                boolean: true,
                option_string: Some("test".into()),
                string_skip_empty: "test".into(),
                struct2: ten_and_true(),
                not_serialize: NotSerialize(42),
            },
        ])
        .assert_allows_de_roundtrip([
            json!({}),
            json!({ "not_serialize": 127 })
        ])
        .assert_rejects_de([
            json!({ "not_serialize": "a string" })
        ])
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_array,
            "structs with `#derive(Deserialize)` can technically be deserialized from sequences, but that's not intended to be used via JSON, so schemars ignores it",
        ));
}
