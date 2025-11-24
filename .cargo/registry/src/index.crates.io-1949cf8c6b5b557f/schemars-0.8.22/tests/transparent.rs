mod util;
use schemars::JsonSchema;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
pub struct OuterStruct {
    inner: TransparentStruct,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[serde(transparent)]
pub struct TransparentStruct {
    #[serde(with = "TransparentNewType")]
    inner: (),
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(transparent)]
pub struct TransparentNewType(Option<InnerStruct>);

#[allow(dead_code)]
#[derive(JsonSchema)]
pub struct InnerStruct(String, i32);

#[test]
fn transparent_struct() -> TestResult {
    test_default_generated_schema::<OuterStruct>("transparent-struct")
}
