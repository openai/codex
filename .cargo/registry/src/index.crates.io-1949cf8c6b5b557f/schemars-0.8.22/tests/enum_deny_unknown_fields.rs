mod util;
use schemars::{JsonSchema, Map};
use util::*;

// Ensure that schemars_derive uses the full path to std::string::String
pub struct String;

#[derive(JsonSchema)]
struct UnitStruct;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct Struct {
    foo: i32,
    bar: bool,
}

// Outer container should always have additionalProperties: false
// `Struct` variant should have additionalProperties: false
#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(rename_all = "camelCase", deny_unknown_fields)]
enum External {
    UnitOne,
    StringMap(Map<&'static str, &'static str>),
    UnitStructNewType(UnitStruct),
    StructNewType(Struct),
    Struct {
        foo: i32,
        bar: bool,
    },
    UnitTwo,
    Tuple(i32, bool),
    #[schemars(with = "i32")]
    WithInt,
}

#[test]
fn enum_external_tag() -> TestResult {
    test_default_generated_schema::<External>("enum-external-duf")
}

// Only `Struct` variant should have additionalProperties: false
#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(tag = "typeProperty", deny_unknown_fields)]
enum Internal {
    UnitOne,
    StringMap(Map<&'static str, &'static str>),
    UnitStructNewType(UnitStruct),
    StructNewType(Struct),
    Struct {
        foo: i32,
        bar: bool,
    },
    UnitTwo,
    #[schemars(with = "i32")]
    WithInt,
}

#[test]
fn enum_internal_tag() -> TestResult {
    test_default_generated_schema::<Internal>("enum-internal-duf")
}

// Only `Struct` variant should have additionalProperties: false
#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(untagged, deny_unknown_fields)]
enum Untagged {
    UnitOne,
    StringMap(Map<&'static str, &'static str>),
    UnitStructNewType(UnitStruct),
    StructNewType(Struct),
    Struct {
        foo: i32,
        bar: bool,
    },
    Tuple(i32, bool),
    #[schemars(with = "i32")]
    WithInt,
}

#[test]
fn enum_untagged() -> TestResult {
    test_default_generated_schema::<Untagged>("enum-untagged-duf")
}

// Outer container and `Struct` variant should have additionalProperties: false
#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(tag = "t", content = "c", deny_unknown_fields)]
enum Adjacent {
    UnitOne,
    StringMap(Map<&'static str, &'static str>),
    UnitStructNewType(UnitStruct),
    StructNewType(Struct),
    Struct {
        foo: i32,
        bar: bool,
    },
    Tuple(i32, bool),
    UnitTwo,
    #[schemars(with = "i32")]
    WithInt,
}

#[test]
fn enum_adjacent_tagged() -> TestResult {
    test_default_generated_schema::<Adjacent>("enum-adjacent-tagged-duf")
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(tag = "typeProperty", deny_unknown_fields)]
enum SimpleInternal {
    A,
    B,
    C,
}

#[test]
fn enum_simple_internal_tag() -> TestResult {
    test_default_generated_schema::<SimpleInternal>("enum-simple-internal-duf")
}
