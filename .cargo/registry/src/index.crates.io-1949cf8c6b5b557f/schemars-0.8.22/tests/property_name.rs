mod util;
use schemars::JsonSchema;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
#[serde(rename_all = "camelCase")]
struct MyStruct {
    camel_case: i32,
    #[serde(rename = "new_name_1")]
    old_name_1: i32,
    #[serde(rename = "ignored")]
    #[schemars(rename = "new_name_2")]
    old_name_2: i32,
}

#[test]
fn set_struct_property_names() -> TestResult {
    test_default_generated_schema::<MyStruct>("property-name-struct")
}
