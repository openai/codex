mod util;
use schemars::JsonSchema;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct MyStruct {
    foo: i32,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct Container {
    result1: Result<MyStruct, Vec<String>>,
    result2: Result<bool, ()>,
}

#[test]
fn result() -> TestResult {
    test_default_generated_schema::<Container>("result")
}
