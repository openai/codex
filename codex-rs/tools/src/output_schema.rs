//! Shares immutable output schema storage and materializes private JSON for consumers.

use crate::mcp_call_tool_result_output_schema;
use serde_json::Map;
use serde_json::Value;
use std::sync::Arc;

/// Immutable output schema that shares an MCP tool's structured result schema
/// until a consumer needs the complete call-result envelope.
#[derive(Debug, Clone)]
pub struct ToolOutputSchema(OutputSchemaStorage);

#[derive(Debug, Clone)]
enum OutputSchemaStorage {
    Json(Arc<Value>),
    McpCallToolResult(Option<Arc<Map<String, Value>>>),
}

impl ToolOutputSchema {
    pub(crate) fn from_mcp_output_schema(
        structured_content: Option<Arc<Map<String, Value>>>,
    ) -> Self {
        Self(OutputSchemaStorage::McpCallToolResult(structured_content))
    }

    /// Materialize the schema for consumers that traverse JSON values.
    pub fn to_value(&self) -> Value {
        match &self.0 {
            OutputSchemaStorage::Json(schema) => schema.as_ref().clone(),
            OutputSchemaStorage::McpCallToolResult(structured_content) => {
                mcp_call_tool_result_output_schema(Value::Object(
                    structured_content.as_deref().cloned().unwrap_or_default(),
                ))
            }
        }
    }

    /// Consume the schema, reusing uniquely owned JSON and cloning shared storage.
    pub fn into_value(self) -> Value {
        match self.0 {
            OutputSchemaStorage::Json(schema) => Arc::unwrap_or_clone(schema),
            OutputSchemaStorage::McpCallToolResult(structured_content) => {
                mcp_call_tool_result_output_schema(Value::Object(
                    structured_content
                        .map(Arc::unwrap_or_clone)
                        .unwrap_or_default(),
                ))
            }
        }
    }
}

impl From<Value> for ToolOutputSchema {
    fn from(value: Value) -> Self {
        Self(OutputSchemaStorage::Json(Arc::new(value)))
    }
}

impl PartialEq for ToolOutputSchema {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (OutputSchemaStorage::Json(left), OutputSchemaStorage::Json(right)) => left == right,
            (
                OutputSchemaStorage::McpCallToolResult(left),
                OutputSchemaStorage::McpCallToolResult(right),
            ) => match (left, right) {
                (Some(left), Some(right)) => left == right,
                (None, None) => true,
                (Some(schema), None) | (None, Some(schema)) => schema.is_empty(),
            },
            (OutputSchemaStorage::Json(left), OutputSchemaStorage::McpCallToolResult(_)) => {
                left.as_ref() == &other.to_value()
            }
            (OutputSchemaStorage::McpCallToolResult(_), OutputSchemaStorage::Json(right)) => {
                &self.to_value() == right.as_ref()
            }
        }
    }
}

#[cfg(test)]
#[path = "output_schema_tests.rs"]
mod tests;
