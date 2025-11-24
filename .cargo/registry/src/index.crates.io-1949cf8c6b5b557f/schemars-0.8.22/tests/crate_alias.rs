mod util;
use ::schemars as not_schemars;
use util::*;

#[allow(unused_imports)]
use std as schemars;

#[allow(dead_code)]
#[derive(not_schemars::JsonSchema)]
#[schemars(crate = "not_schemars")]
struct Struct {
    /// This is a document
    foo: i32,
    bar: bool,
}

#[test]
fn test_crate_alias() -> TestResult {
    test_default_generated_schema::<Struct>("crate_alias")
}
