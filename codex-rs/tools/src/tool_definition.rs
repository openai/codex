use crate::JsonSchema;
use serde_json::Value as JsonValue;

/// Tool metadata and schemas that downstream crates can adapt into higher-level
/// tool specs.
#[derive(Debug, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: JsonSchema,
    pub output_schema: Option<JsonValue>,
    pub defer_loading: bool,
}

impl ToolDefinition {
    pub fn renamed(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn into_deferred(mut self) -> Self {
        self.output_schema = None;
        self.defer_loading = true;
        self
    }
}

#[cfg(test)]
#[path = "tool_definition_tests.rs"]
mod tests;
