use crate::JsonSchema;
use crate::ToolDefinition;
use crate::dynamic_tool_to_tool_definition;
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
pub enum ToolSearchOutputTool {
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
    Ok(tool_definition_to_responses_api_tool(
        &dynamic_tool_to_tool_definition(tool)?,
    ))
}

/// Converts the leaf function shape of a canonical tool definition.
///
/// If the tool is namespaced, callers are still responsible for wrapping the
/// returned function in a Responses API namespace tool.
pub fn tool_definition_to_responses_api_tool(tool_definition: &ToolDefinition) -> ResponsesApiTool {
    ResponsesApiTool {
        name: tool_definition.name.name.clone(),
        description: tool_definition.description.clone(),
        strict: false,
        defer_loading: tool_definition.defer_loading().then_some(true),
        parameters: tool_definition.input_schema.clone(),
        output_schema: tool_definition.output_schema.clone(),
    }
}

pub fn tool_definition_to_tool_search_output_tool(
    tool_definition: &ToolDefinition,
) -> ToolSearchOutputTool {
    let function_tool = tool_definition_to_responses_api_tool(tool_definition);
    let Some(namespace) = tool_definition.name.namespace.as_ref() else {
        return ToolSearchOutputTool::Function(function_tool);
    };

    let description = tool_definition
        .presentation
        .as_ref()
        .and_then(|presentation| presentation.namespace_description.as_deref())
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_namespace_description(namespace));

    ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
        name: namespace.clone(),
        description,
        tools: vec![ResponsesApiNamespaceTool::Function(function_tool)],
    })
}

#[cfg(test)]
#[path = "responses_api_tests.rs"]
mod tests;
