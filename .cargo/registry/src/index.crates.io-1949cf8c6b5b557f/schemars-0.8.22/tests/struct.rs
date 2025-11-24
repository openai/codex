mod util;
use schemars::JsonSchema;
use util::*;

// Ensure that schemars_derive uses the full path to std::string::String
pub struct String;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct Struct {
    foo: i32,
    bar: bool,
    baz: Option<&'static str>,
}

#[test]
fn struct_normal() -> TestResult {
    test_default_generated_schema::<Struct>("struct-normal")
}

#[allow(dead_code)]
#[derive(JsonSchema)]
pub struct Tuple(i32, bool, Option<&'static str>);

#[test]
fn struct_tuple() -> TestResult {
    test_default_generated_schema::<Tuple>("struct-tuple")
}

#[allow(dead_code)]
#[derive(JsonSchema)]
pub struct Newtype(i32);

#[test]
fn struct_newtype() -> TestResult {
    test_default_generated_schema::<Newtype>("struct-newtype")
}

#[derive(JsonSchema)]
pub struct Unit;

#[test]
fn struct_unit() -> TestResult {
    test_default_generated_schema::<Unit>("struct-unit")
}
