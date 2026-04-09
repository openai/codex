use crate::JsonSchema;
use serde_json::Value as JsonValue;

/// Where a tool definition originated before it was normalized into the shared
/// `ToolDefinition` shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolOrigin {
    /// A built-in Codex tool defined directly in Rust, such as `exec_command`,
    /// `view_image`, `update_plan`, or the agent/collaboration tools.
    #[default]
    Native,
    /// A tool discovered from an MCP server and converted through the MCP tool
    /// pipeline. These tools may advertise an MCP `outputSchema`, which code
    /// mode uses to render shared `mcp_result<T>` aliases.
    Mcp,
    /// A runtime-provided non-MCP tool definition, such as a `DynamicToolSpec`
    /// supplied externally rather than compiled into the binary.
    Dynamic,
}

/// Tool metadata and schemas that downstream crates can adapt into higher-level
/// tool specs.
#[derive(Debug, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: JsonSchema,
    pub output_schema: Option<JsonValue>,
    pub origin: ToolOrigin,
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
