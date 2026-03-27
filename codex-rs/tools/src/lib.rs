//! Shared tool definitions and Responses API tool primitives that can live
//! outside `codex-core`.

mod dynamic_tool;
mod json_schema;
mod mcp_tool;
mod responses_api;
mod tool_definition;

pub use dynamic_tool::parse_dynamic_tool;
pub use json_schema::AdditionalProperties;
pub use json_schema::JsonSchema;
pub use json_schema::parse_tool_input_schema;
pub use mcp_tool::mcp_call_tool_result_output_schema;
pub use mcp_tool::parse_mcp_tool;
pub use responses_api::FreeformTool;
pub use responses_api::FreeformToolFormat;
pub use responses_api::ResponsesApiNamespace;
pub use responses_api::ResponsesApiNamespaceTool;
pub use responses_api::ResponsesApiTool;
pub use responses_api::ToolSearchOutputTool;
pub use responses_api::dynamic_tool_to_responses_api_tool;
pub use responses_api::mcp_tool_to_deferred_responses_api_tool;
pub use responses_api::mcp_tool_to_responses_api_tool;
pub use responses_api::tool_definition_to_responses_api_tool;
pub use tool_definition::ToolDefinition;
