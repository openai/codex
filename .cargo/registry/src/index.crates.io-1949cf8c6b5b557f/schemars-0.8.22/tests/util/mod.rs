use pretty_assertions::assert_eq;
use schemars::{r#gen::SchemaSettings, schema::RootSchema, schema_for, JsonSchema};
use std::error::Error;
use std::fs;

pub type TestResult = Result<(), Box<dyn Error>>;

#[allow(dead_code)] // https://github.com/rust-lang/rust/issues/46379
pub fn test_generated_schema<T: JsonSchema>(file: &str, settings: SchemaSettings) -> TestResult {
    let actual = settings.into_generator().into_root_schema_for::<T>();
    test_schema(&actual, file)
}

#[allow(dead_code)] // https://github.com/rust-lang/rust/issues/46379
pub fn test_default_generated_schema<T: JsonSchema>(file: &str) -> TestResult {
    let actual = schema_for!(T);
    test_schema(&actual, file)
}

pub fn test_schema(actual: &RootSchema, file: &str) -> TestResult {
    let expected_json = match fs::read_to_string(format!("tests/expected/{}.json", file)) {
        Ok(j) => j,
        Err(e) => {
            write_actual_to_file(actual, file)?;
            return Err(Box::from(e));
        }
    };
    let expected = &serde_json::from_str(&expected_json)?;

    if actual != expected {
        write_actual_to_file(actual, file)?;
    }

    assert_eq!(expected, actual);
    Ok(())
}

fn write_actual_to_file(schema: &RootSchema, file: &str) -> TestResult {
    let actual_json = serde_json::to_string_pretty(&schema)?;
    fs::write(format!("tests/actual/{}.json", file), actual_json)?;
    Ok(())
}
