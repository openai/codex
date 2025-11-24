mod util;
use schemars::JsonSchema;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct MyStruct {
    #[schemars(skip)]
    skipped1: i32,
    #[serde(skip)]
    skipped2: bool,
    #[serde(skip_deserializing)]
    readable: String,
    #[serde(skip_serializing)]
    writable: f32,
    included: (),
}

#[test]
fn skip_struct_fields() -> TestResult {
    test_default_generated_schema::<MyStruct>("skip_struct_fields")
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct TupleStruct(
    #[schemars(skip)] i32,
    #[serde(skip)] bool,
    #[serde(skip_deserializing)] String,
    #[serde(skip_serializing)] f32,
    (),
);

#[test]
fn skip_tuple_fields() -> TestResult {
    test_default_generated_schema::<TupleStruct>("skip_tuple_fields")
}

#[derive(JsonSchema)]
pub enum MyEnum {
    #[schemars(skip)]
    Skipped1(i32),
    #[serde(skip)]
    Skipped2,
    #[serde(skip_deserializing)]
    Skipped3,
    #[serde(skip_serializing)]
    Included1(f32),
    Included2,
}

#[test]
fn skip_enum_variants() -> TestResult {
    test_default_generated_schema::<MyEnum>("skip_enum_variants")
}
