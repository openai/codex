//cargo test --test test_tool_builder_methods --features "client server macros"
use rmcp::model::{JsonObject, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InputData {
    pub name: String,
    pub age: u32,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OutputData {
    pub greeting: String,
    pub is_adult: bool,
}

#[test]
fn test_with_output_schema() {
    let tool = Tool::new("test", "Test tool", JsonObject::new()).with_output_schema::<OutputData>();

    assert!(tool.output_schema.is_some());

    // Verify the schema contains expected fields
    let schema_str = serde_json::to_string(tool.output_schema.as_ref().unwrap()).unwrap();
    assert!(schema_str.contains("greeting"));
    assert!(schema_str.contains("is_adult"));
}

#[test]
fn test_with_input_schema() {
    let tool = Tool::new("test", "Test tool", JsonObject::new()).with_input_schema::<InputData>();

    // Verify the schema contains expected fields
    let schema_str = serde_json::to_string(&tool.input_schema).unwrap();
    assert!(schema_str.contains("name"));
    assert!(schema_str.contains("age"));
}

#[test]
fn test_chained_builder_methods() {
    let tool = Tool::new("test", "Test tool", JsonObject::new())
        .with_input_schema::<InputData>()
        .with_output_schema::<OutputData>()
        .annotate(rmcp::model::ToolAnnotations::new().read_only(true));

    assert!(tool.output_schema.is_some());
    assert!(tool.annotations.is_some());
    assert_eq!(
        tool.annotations.as_ref().unwrap().read_only_hint,
        Some(true)
    );

    // Verify both schemas are set correctly
    let input_schema_str = serde_json::to_string(&tool.input_schema).unwrap();
    assert!(input_schema_str.contains("name"));
    assert!(input_schema_str.contains("age"));

    let output_schema_str = serde_json::to_string(tool.output_schema.as_ref().unwrap()).unwrap();
    assert!(output_schema_str.contains("greeting"));
    assert!(output_schema_str.contains("is_adult"));
}
