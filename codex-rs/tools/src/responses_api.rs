use crate::JsonSchema;
use crate::ToolName;
use crate::mcp_tool_definition;
use crate::parse_dynamic_tool;
use crate::parse_tool_input_schema;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tool_api::ToolDefinition;
use codex_tool_api::ToolExposure;
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

pub fn default_namespace_description(namespace_name: &str) -> String {
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
    tool_definition_to_responses_api_tool(&parse_dynamic_tool(tool))
}

pub fn dynamic_tool_to_loadable_tool_spec(
    tool: &DynamicToolSpec,
) -> Result<LoadableToolSpec, serde_json::Error> {
    tool_definition_to_loadable_tool_spec(
        &parse_dynamic_tool(tool),
        /*namespace_description*/ None,
    )
}

pub fn coalesce_loadable_tool_specs(
    specs: impl IntoIterator<Item = LoadableToolSpec>,
) -> Vec<LoadableToolSpec> {
    let mut coalesced_specs = Vec::new();
    for spec in specs {
        match spec {
            LoadableToolSpec::Function(tool) => {
                coalesced_specs.push(LoadableToolSpec::Function(tool));
            }
            LoadableToolSpec::Namespace(mut namespace) => {
                if let Some(existing_namespace) =
                    coalesced_specs.iter_mut().find_map(|spec| match spec {
                        LoadableToolSpec::Namespace(existing_namespace)
                            if existing_namespace.name == namespace.name =>
                        {
                            Some(existing_namespace)
                        }
                        LoadableToolSpec::Function(_) | LoadableToolSpec::Namespace(_) => None,
                    })
                {
                    existing_namespace.tools.append(&mut namespace.tools);
                } else {
                    coalesced_specs.push(LoadableToolSpec::Namespace(namespace));
                }
            }
        }
    }
    coalesced_specs
}

pub fn mcp_tool_to_deferred_responses_api_tool(
    tool_name: &ToolName,
    tool: &rmcp::model::Tool,
) -> Result<ResponsesApiTool, serde_json::Error> {
    tool_definition_to_responses_api_tool(&mcp_tool_definition(tool_name.clone(), tool).deferred())
}

pub fn tool_definition_to_responses_api_tool<R>(
    tool_definition: &ToolDefinition<R>,
) -> Result<ResponsesApiTool, serde_json::Error> {
    let spec = tool_definition.spec();
    Ok(ResponsesApiTool {
        name: tool_definition.tool_name().name.clone(),
        description: spec.description.clone(),
        strict: spec.strict,
        defer_loading: matches!(tool_definition.exposure(), ToolExposure::Deferred).then_some(true),
        parameters: parse_tool_input_schema(&spec.parameters)?,
        output_schema: tool_definition.output_schema().cloned(),
    })
}

pub fn tool_definition_to_loadable_tool_spec<R>(
    tool_definition: &ToolDefinition<R>,
    namespace_description: Option<String>,
) -> Result<LoadableToolSpec, serde_json::Error> {
    let output_tool = tool_definition_to_responses_api_tool(tool_definition)?;
    Ok(match tool_definition.tool_name().namespace.as_ref() {
        Some(namespace) => LoadableToolSpec::Namespace(ResponsesApiNamespace {
            name: namespace.clone(),
            description: namespace_description
                .unwrap_or_else(|| default_namespace_description(namespace)),
            tools: vec![ResponsesApiNamespaceTool::Function(output_tool)],
        }),
        None => LoadableToolSpec::Function(output_tool),
    })
}

#[cfg(test)]
#[path = "responses_api_tests.rs"]
mod tests;
