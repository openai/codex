mod util;
use schemars::JsonSchema;
use util::*;

fn schema_fn(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
    <bool>::json_schema(generator)
}

struct DoesntImplementJsonSchema;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct Struct {
    #[schemars(schema_with = "schema_fn")]
    foo: DoesntImplementJsonSchema,
    bar: i32,
    #[schemars(schema_with = "schema_fn")]
    baz: DoesntImplementJsonSchema,
}

#[test]
fn struct_normal() -> TestResult {
    test_default_generated_schema::<Struct>("schema_with-struct")
}

#[allow(dead_code)]
#[derive(JsonSchema)]
pub struct Tuple(
    #[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema,
    i32,
    #[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema,
);

#[test]
fn struct_tuple() -> TestResult {
    test_default_generated_schema::<Tuple>("schema_with-tuple")
}

#[derive(JsonSchema)]
pub struct Newtype(#[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema);

#[test]
fn struct_newtype() -> TestResult {
    test_default_generated_schema::<Newtype>("schema_with-newtype")
}

#[derive(JsonSchema)]
#[schemars(transparent)]
pub struct TransparentNewtype(#[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema);

#[test]
fn struct_transparent_newtype() -> TestResult {
    test_default_generated_schema::<TransparentNewtype>("schema_with-transparent-newtype")
}
