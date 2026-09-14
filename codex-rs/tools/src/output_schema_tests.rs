//! Verify materialization preserves schemas, isolates mutations, and respects equality.

use super::ToolOutputSchema;
use pretty_assertions::assert_eq;
use pretty_assertions::assert_ne;
use serde_json::Value;
use std::sync::Arc;

#[test]
fn materializing_mcp_output_preserves_json_and_keeps_mutations_private() {
    let raw: serde_json::Map<String, Value> = serde_json::from_str(
        r#"{"properties":{"z":{"enum":[9007199254740993,1.0]},"a":{"type":"string"}},"required":["z","a"]}"#,
    )
    .unwrap();
    let raw = Arc::new(raw);
    let schema = ToolOutputSchema::from_mcp_output_schema(Some(Arc::clone(&raw)));
    let mut materialized = schema.to_value();
    assert_eq!(
        serde_json::to_string(&materialized["properties"]["structuredContent"]).unwrap(),
        serde_json::to_string(&raw).unwrap(),
    );
    materialized["properties"]["structuredContent"] = Value::Null;
    assert_eq!(
        schema.to_value()["properties"]["structuredContent"],
        Value::Object(raw.as_ref().clone()),
    );
}

#[test]
fn materialized_and_lazy_schemas_compare_in_both_directions() {
    let populated = serde_json::from_value(serde_json::json!({
        "type": "object",
        "properties": {"result": {"type": "string"}},
    }))
    .unwrap();
    for structured_content in [None, Some(Arc::default()), Some(Arc::new(populated))] {
        let lazy = ToolOutputSchema::from_mcp_output_schema(structured_content);
        let materialized = ToolOutputSchema::from(lazy.to_value());
        assert_eq!(materialized, lazy);
        assert_eq!(lazy, materialized);

        let mut different = lazy.to_value();
        different["properties"]["structuredContent"] = serde_json::json!({"type": "null"});
        let different = ToolOutputSchema::from(different);
        assert_ne!(different, lazy);
        assert_ne!(lazy, different);

        assert_eq!(lazy.clone().into_value(), materialized.clone().into_value());
        assert_eq!(lazy.into_value(), materialized.into_value());
    }
}

#[test]
fn consuming_unique_json_schema_reuses_storage() {
    let value = serde_json::json!({"description": "An output schema"});
    let description_ptr = value["description"].as_str().unwrap().as_ptr();
    let schema = ToolOutputSchema::from(value);

    let value = schema.into_value();

    assert_eq!(
        value,
        serde_json::json!({"description": "An output schema"})
    );
    assert_eq!(
        value["description"].as_str().unwrap().as_ptr(),
        description_ptr
    );
}
