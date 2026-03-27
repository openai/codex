//! Shared tool-schema parsing primitives that can live outside `codex-core`.

mod json_schema;
mod mcp_tool;

pub use json_schema::AdditionalProperties;
pub use json_schema::JsonSchema;
pub use json_schema::parse_tool_input_schema;
pub use mcp_tool::ParsedMcpTool;
pub use mcp_tool::mcp_call_tool_result_output_schema;
pub use mcp_tool::parse_mcp_tool;
