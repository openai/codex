use crate::AdditionalProperties;
use crate::JsonSchema;
use crate::JsonSchemaPrimitiveType;
use crate::JsonSchemaType;
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
    /// Whether Responses API strict JSON Schema validation should be enforced.
    ///
    /// When true, request construction fails unless the schema uses the
    /// Responses API strict subset.
    pub strict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    pub parameters: JsonSchema,
    #[serde(skip)]
    pub output_schema: Option<Value>,
}

impl ResponsesApiTool {
    pub(crate) fn validate_for_responses_api(&self) -> Result<(), serde_json::Error> {
        if !self.strict {
            return Ok(());
        }

        if !matches!(
            self.parameters.schema_type,
            Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object))
        ) {
            return Err(strict_schema_error(
                "strict tool parameters must be an object schema",
            ));
        }

        validate_strict_schema(&self.parameters).map_err(strict_schema_error)
    }
}

fn strict_schema_error(message: impl Into<String>) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message.into(),
    ))
}

fn validate_strict_schema(schema: &JsonSchema) -> Result<(), String> {
    if schema_allows_object(schema) {
        let required = schema
            .required
            .as_ref()
            .ok_or_else(|| "strict object schemas must include `required`".to_string())?;

        if !matches!(
            schema.additional_properties,
            Some(AdditionalProperties::Boolean(false))
        ) {
            return Err(
                "strict object schemas must set `additionalProperties` to false".to_string(),
            );
        }

        if let Some(properties) = &schema.properties {
            for name in properties.keys() {
                if !required.contains(name) {
                    return Err(format!(
                        "strict object schemas must list every property in `required`; missing `{name}`"
                    ));
                }
            }
        }
    }

    if let Some(properties) = &schema.properties {
        for schema in properties.values() {
            validate_strict_schema(schema)?;
        }
    }

    if let Some(items) = &schema.items {
        validate_strict_schema(items)?;
    }

    if let Some(any_of) = &schema.any_of {
        for schema in any_of {
            validate_strict_schema(schema)?;
        }
    }

    if let Some(defs) = &schema.defs {
        for schema in defs.values() {
            validate_strict_schema(schema)?;
        }
    }

    if let Some(definitions) = &schema.definitions {
        for schema in definitions.values() {
            validate_strict_schema(schema)?;
        }
    }

    Ok(())
}

fn schema_allows_object(schema: &JsonSchema) -> bool {
    match &schema.schema_type {
        Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)) => true,
        Some(JsonSchemaType::Multiple(types)) => types.contains(&JsonSchemaPrimitiveType::Object),
        Some(JsonSchemaType::Single(_)) | None => false,
    }
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
    Ok(tool_definition_to_responses_api_tool(parse_dynamic_tool(
        tool,
    )?))
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
