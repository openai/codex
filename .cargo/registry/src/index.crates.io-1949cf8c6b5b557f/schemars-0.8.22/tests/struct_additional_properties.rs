mod util;
use schemars::JsonSchema;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Struct {
    foo: i32,
    bar: bool,
    baz: Option<String>,
}

#[test]
fn struct_normal_additional_properties() -> TestResult {
    test_default_generated_schema::<Struct>("struct-normal-additional-properties")
}
