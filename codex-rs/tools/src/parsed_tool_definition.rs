use crate::JsonSchema;
use serde_json::Value as JsonValue;

/// Parsed tool metadata and schemas that downstream crates can adapt into
/// higher-level tool specs.
#[derive(Debug, PartialEq)]
pub struct ParsedToolDefinition {
    pub description: String,
    pub input_schema: JsonSchema,
    pub output_schema: Option<JsonValue>,
    pub defer_loading: bool,
}
