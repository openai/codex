mod util;
use schemars::JsonSchema;
use util::*;

fn schema_fn(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
    <bool>::json_schema(generator)
}

#[derive(Debug)]
pub struct DoesntImplementJsonSchema;

#[derive(JsonSchema)]
#[schemars(rename_all = "camelCase")]
pub enum External {
    Struct {
        #[schemars(schema_with = "schema_fn")]
        foo: DoesntImplementJsonSchema,
    },
    NewType(#[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema),
    Tuple(
        #[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema,
        i32,
    ),
    #[schemars(schema_with = "schema_fn")]
    Unit,
}

#[test]
fn enum_external_tag() -> TestResult {
    test_default_generated_schema::<External>("schema_with-enum-external")
}

#[derive(JsonSchema)]
#[schemars(tag = "typeProperty")]
pub enum Internal {
    Struct {
        #[schemars(schema_with = "schema_fn")]
        foo: DoesntImplementJsonSchema,
    },
    NewType(#[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema),
    #[schemars(schema_with = "schema_fn")]
    Unit,
}

#[test]
fn enum_internal_tag() -> TestResult {
    test_default_generated_schema::<Internal>("schema_with-enum-internal")
}

#[derive(JsonSchema)]
#[schemars(untagged)]
pub enum Untagged {
    Struct {
        #[schemars(schema_with = "schema_fn")]
        foo: DoesntImplementJsonSchema,
    },
    NewType(#[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema),
    Tuple(
        #[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema,
        i32,
    ),
    #[schemars(schema_with = "schema_fn")]
    Unit,
}

#[test]
fn enum_untagged() -> TestResult {
    test_default_generated_schema::<Untagged>("schema_with-enum-untagged")
}

#[derive(JsonSchema)]
#[schemars(tag = "t", content = "c")]
pub enum Adjacent {
    Struct {
        #[schemars(schema_with = "schema_fn")]
        foo: DoesntImplementJsonSchema,
    },
    NewType(#[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema),
    Tuple(
        #[schemars(schema_with = "schema_fn")] DoesntImplementJsonSchema,
        i32,
    ),
    #[schemars(schema_with = "schema_fn")]
    Unit,
}

#[test]
fn enum_adjacent_tagged() -> TestResult {
    test_default_generated_schema::<Adjacent>("schema_with-enum-adjacent-tagged")
}
