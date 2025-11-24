mod util;
use schemars::JsonSchema;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct Flat {
    f: f32,
    b: bool,
    s: String,
    #[serde(default)]
    os: String,
    v: Vec<i32>,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(rename = "Flat")]
struct Deep1 {
    f: f32,
    #[schemars(flatten)]
    deep2: Deep2,
    v: Vec<i32>,
}

#[allow(clippy::option_option, dead_code)]
#[derive(JsonSchema)]
struct Deep2 {
    b: bool,
    #[serde(flatten)]
    deep3: Deep3,
    #[serde(flatten)]
    deep4: Box<Option<Option<Box<Deep4>>>>,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct Deep3 {
    s: &'static str,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct Deep4 {
    #[serde(default)]
    os: &'static str,
}

#[test]
fn test_flat_schema() -> TestResult {
    test_default_generated_schema::<Flat>("flatten")
}

#[test]
fn test_flattened_schema() -> TestResult {
    test_default_generated_schema::<Deep1>("flatten")
}
