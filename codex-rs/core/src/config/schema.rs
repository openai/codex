use schemars::schema::RootSchema;
use schemars::schema_for;
use serde_json::Value;

use super::ConfigToml;

pub fn config_json_schema() -> RootSchema {
    schema_for!(ConfigToml)
}

pub fn config_json_schema_value() -> Value {
    serde_json::to_value(config_json_schema()).expect("serialize config schema")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_includes_core_fields_and_shapes() {
        let schema = config_json_schema_value();
        let config_schema = schema
            .get("definitions")
            .and_then(|defs| defs.get("ConfigToml"))
            .or_else(|| schema.get("schema"))
            .unwrap_or(&schema);
        let Some(props) = config_schema.get("properties") else {
            panic!("config schema missing properties");
        };

        let model_schema = props.get("model").expect("model schema present");
        let model_is_string = match model_schema.get("type") {
            Some(Value::String(t)) => t == "string",
            Some(Value::Array(types)) => types
                .iter()
                .any(|t| t.as_str().is_some_and(|inner| inner == "string")),
            _ => false,
        };
        assert!(model_is_string);

        let sandbox_mode_definition = schema
            .get("definitions")
            .and_then(|defs| defs.get("SandboxMode"))
            .expect("SandboxMode schema present");
        let sandbox_enums = sandbox_mode_definition
            .get("enum")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let sandbox_values = sandbox_enums
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert!(sandbox_values.contains(&"read-only"));
        assert!(sandbox_values.contains(&"workspace-write"));
        assert!(sandbox_values.contains(&"danger-full-access"));

        assert!(
            props
                .get("mcp_servers")
                .and_then(|v| v.get("additionalProperties"))
                .is_some()
        );
        assert!(
            props
                .get("profiles")
                .and_then(|v| v.get("additionalProperties"))
                .is_some()
        );
    }
}
