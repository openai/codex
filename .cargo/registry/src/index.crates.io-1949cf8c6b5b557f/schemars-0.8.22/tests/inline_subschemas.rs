mod util;
use schemars::r#gen::SchemaSettings;
use schemars::JsonSchema;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct MyJob {
    spec: MyJobSpec,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct MyJobSpec {
    replicas: u32,
}

#[test]
fn struct_normal() -> TestResult {
    let mut settings = SchemaSettings::default();
    settings.inline_subschemas = true;
    test_generated_schema::<MyJob>("inline-subschemas", settings)
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct RecursiveOuter {
    direct: Option<Box<RecursiveOuter>>,
    indirect: Option<Box<RecursiveInner>>,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct RecursiveInner {
    recursive: RecursiveOuter,
}

#[test]
fn struct_recursive() -> TestResult {
    let mut settings = SchemaSettings::default();
    settings.inline_subschemas = true;
    test_generated_schema::<RecursiveOuter>("inline-subschemas-recursive", settings)
}
