mod util;
use enumset::{EnumSet, EnumSetType};
use schemars::JsonSchema;
use util::*;

#[derive(EnumSetType, JsonSchema)]
enum Foo {
    Bar,
    Baz,
}

#[test]
fn enumset() -> TestResult {
    test_default_generated_schema::<EnumSet<Foo>>("enumset")
}
