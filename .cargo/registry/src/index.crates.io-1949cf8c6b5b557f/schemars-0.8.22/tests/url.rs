mod util;
use schemars::JsonSchema;
use url::Url;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct UrlTypes {
    url: Url,
}

#[test]
fn url_types() -> TestResult {
    test_default_generated_schema::<UrlTypes>("url")
}
