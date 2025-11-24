mod util;
use schemars::JsonSchema;
use util::*;

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

fn ten_and_true() -> MyStruct2 {
    MyStruct2 {
        my_int: 10,
        my_bool: true,
    }
}

fn six() -> i32 {
    6
}

fn custom_serialize<S>(value: &MyStruct2, ser: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    ser.collect_str(&format_args!("i:{} b:{}", value.my_int, value.my_bool))
}

#[allow(dead_code)]
#[derive(Default, JsonSchema)]
#[serde(default)]
struct MyStruct {
    my_int: i32,
    my_bool: bool,
    my_optional_string: Option<String>,
    #[serde(serialize_with = "custom_serialize")]
    my_struct2: MyStruct2,
    #[serde(
        serialize_with = "custom_serialize",
        skip_serializing_if = "is_default"
    )]
    my_struct2_default_skipped: MyStruct2,
    not_serialize: NotSerialize,
}

#[allow(dead_code)]
#[derive(Default, JsonSchema, PartialEq)]
#[serde(default = "ten_and_true")]
struct MyStruct2 {
    #[serde(default = "six")]
    my_int: i32,
    my_bool: bool,
}

#[derive(Default, JsonSchema)]
struct NotSerialize;

#[test]
fn schema_default_values() -> TestResult {
    test_default_generated_schema::<MyStruct>("default")
}
