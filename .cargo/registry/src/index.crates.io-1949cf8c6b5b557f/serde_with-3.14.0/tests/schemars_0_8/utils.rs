#![allow(dead_code, missing_docs)]

use serde::Serialize;

#[track_caller]
pub fn check_matches_schema<T>(value: &serde_json::Value)
where
    T: schemars_0_8::JsonSchema,
{
    use jsonschema::Validator;
    use std::fmt::Write;

    let schema_object = serde_json::to_value(schemars_0_8::schema_for!(T))
        .expect("schema for T could not be serialized to json");
    let schema = match Validator::new(&schema_object) {
        Ok(schema) => schema,
        Err(e) => panic!("schema for T was not a valid JSON schema: {e}"),
    };

    if let Err(err) = schema.validate(value) {
        let mut message = String::new();

        let _ = writeln!(
            &mut message,
            "Object was not valid according to its own schema:"
        );

        let _ = writeln!(&mut message, "  -> {err}");
        let _ = writeln!(&mut message);
        let _ = writeln!(&mut message, "Object Value:");
        let _ = writeln!(
            &mut message,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("> error: {e}"))
        );
        let _ = writeln!(&mut message);
        let _ = writeln!(&mut message, "JSON Schema:");
        let _ = writeln!(
            &mut message,
            "{}",
            serde_json::to_string_pretty(&schema_object)
                .unwrap_or_else(|e| format!("> error: {e}"))
        );

        panic!("{}", message);
    };
}

#[track_caller]
pub fn check_valid_json_schema<T>(value: &T)
where
    T: schemars_0_8::JsonSchema + Serialize,
{
    let value = serde_json::to_value(value).expect("could not serialize T to json");

    check_matches_schema::<T>(&value);
}
