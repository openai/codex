use crate::JsonSchema;
use crate::ToolDefinition;
use crate::ToolName;
use crate::parse_dynamic_tool;
use crate::parse_mcp_tool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeformTool {
    pub name: String,
    pub description: String,
    pub format: FreeformToolFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeformToolFormat {
    pub r#type: String,
    pub syntax: String,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ResponsesApiTool {
    pub name: String,
    pub description: String,
    /// TODO: Validation. When strict is set to true, the JSON schema,
    /// `required` and `additional_properties` must be present. All fields in
    /// `properties` must be present in `required`.
    pub strict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    pub parameters: JsonSchema,
    #[serde(skip)]
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum LoadableToolSpec {
    #[allow(dead_code)]
    #[serde(rename = "function")]
    Function(ResponsesApiTool),
    #[serde(rename = "namespace")]
    Namespace(ResponsesApiNamespace),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ResponsesApiNamespace {
    pub name: String,
    pub description: String,
    pub tools: Vec<ResponsesApiNamespaceTool>,
}

pub(crate) fn default_namespace_description(namespace_name: &str) -> String {
    format!("Tools in the {namespace_name} namespace.")
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum ResponsesApiNamespaceTool {
    #[serde(rename = "function")]
    Function(ResponsesApiTool),
}

pub fn dynamic_tool_to_responses_api_tool(
    tool: &DynamicToolSpec,
) -> Result<ResponsesApiTool, serde_json::Error> {
    Ok(tool_definition_to_responses_api_tool(parse_dynamic_tool(
        tool,
    )?))
}

pub fn dynamic_tool_to_loadable_tool_spec(
    tool: &DynamicToolSpec,
) -> Result<LoadableToolSpec, serde_json::Error> {
    let output_tool = dynamic_tool_to_responses_api_tool(tool)?;
    Ok(match tool.namespace.as_ref() {
        Some(namespace) => LoadableToolSpec::Namespace(ResponsesApiNamespace {
            name: namespace.clone(),
            // the user doesn't provide a description for dynamic tools, so we use the default
            description: default_namespace_description(namespace),
            tools: vec![ResponsesApiNamespaceTool::Function(output_tool)],
        }),
        None => LoadableToolSpec::Function(output_tool),
    })
}

pub fn mcp_tool_to_responses_api_tool(
    tool_name: &ToolName,
    tool: &rmcp::model::Tool,
) -> Result<ResponsesApiTool, serde_json::Error> {
    Ok(tool_definition_to_responses_api_tool(
        parse_mcp_tool(tool)?.renamed(tool_name.name.clone()),
    ))
}

pub fn mcp_tool_to_deferred_responses_api_tool(
    tool_name: &ToolName,
    tool: &rmcp::model::Tool,
) -> Result<ResponsesApiTool, serde_json::Error> {
    Ok(tool_definition_to_responses_api_tool(
        parse_mcp_tool(tool)?
            .renamed(tool_name.name.clone())
            .into_deferred(),
    ))
}

pub fn tool_definition_to_responses_api_tool(tool_definition: ToolDefinition) -> ResponsesApiTool {
    ResponsesApiTool {
        name: tool_definition.name,
        description: tool_definition.description,
        strict: false,
        defer_loading: tool_definition.defer_loading.then_some(true),
        parameters: tool_definition.input_schema,
        output_schema: tool_definition.output_schema,
    }
}

#[cfg(test)]
#[path = "responses_api_tests.rs"]
mod tests;
