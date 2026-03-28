//! Shared tool definitions and Responses API tool primitives that can live
//! outside `codex-core`.

mod code_mode;
mod dynamic_tool;
mod json_schema;
mod mcp_tool;
mod responses_api;
mod tool_definition;
mod tool_spec;

pub use code_mode::augment_tool_spec_for_code_mode;
pub use code_mode::tool_spec_to_code_mode_tool_definition;
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
pub use tool_spec::ConfiguredToolSpec;
pub use tool_spec::ResponsesApiWebSearchFilters;
pub use tool_spec::ResponsesApiWebSearchUserLocation;
pub use tool_spec::ToolSpec;
pub use tool_spec::create_tools_json_for_responses_api;
