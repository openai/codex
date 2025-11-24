mod util;
use schemars::r#gen::SchemaSettings;
use schemars::schema::RootSchema;
use util::*;

#[test]
fn schema_matches_draft07() -> TestResult {
    test_generated_schema::<RootSchema>("schema", SchemaSettings::draft07())
}

#[test]
fn schema_matches_2019_09() -> TestResult {
    test_generated_schema::<RootSchema>("schema-2019_09", SchemaSettings::draft2019_09())
}

#[test]
fn schema_matches_openapi3() -> TestResult {
    test_generated_schema::<RootSchema>("schema-openapi3", SchemaSettings::openapi3())
}
