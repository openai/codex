mod util;
use schemars::JsonSchema;
use serde::Serialize;
use util::*;

#[derive(Default, JsonSchema, Serialize)]
#[schemars(example = "Struct::default", example = "null")]
struct Struct {
    #[schemars(example = "eight", example = "null")]
    foo: i32,
    bar: bool,
    #[schemars(example = "null")]
    baz: Option<&'static str>,
}

fn eight() -> i32 {
    8
}

fn null() {}

#[test]
fn examples() -> TestResult {
    test_default_generated_schema::<Struct>("examples")
}
