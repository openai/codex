use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::Error as _;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", export_to = "v2/")]
pub enum DynamicToolSpec {
    Function(DynamicToolFunctionSpec),
    Namespace(DynamicToolNamespaceSpec),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolFunctionSpec {
    pub name: String,
    pub description: String,
    pub input_schema: JsonValue,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub defer_loading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolNamespaceSpec {
    pub name: String,
    pub description: String,
    pub tools: Vec<DynamicToolNamespaceTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", export_to = "v2/")]
pub enum DynamicToolNamespaceTool {
    Function(DynamicToolFunctionSpec),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallRequest {
    pub call_id: String,
    pub turn_id: String,
    #[serde(default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolResponse {
    pub content_items: Vec<DynamicToolCallOutputContentItem>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
pub enum DynamicToolCallOutputContentItem {
    #[serde(rename_all = "camelCase")]
    InputText { text: String },
    #[serde(rename_all = "camelCase")]
    InputImage { image_url: String },
}

/// Former flat `SessionMeta` shape, including the old `exposeToContext` flag.
/// Kept so new builds can resume sessions written before explicit namespaces.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDynamicToolSpec {
    namespace: Option<String>,
    name: String,
    description: String,
    input_schema: JsonValue,
    defer_loading: Option<bool>,
    expose_to_context: Option<bool>,
}

pub fn normalize_dynamic_tool_specs(
    values: Vec<JsonValue>,
) -> Result<Vec<DynamicToolSpec>, serde_json::Error> {
    let has_legacy_format = values.iter().any(|value| {
        value.get("namespace").is_some()
            || value.get("exposeToContext").is_some()
            || value.get("type").is_none()
    });
    let has_canonical_format = values.iter().any(|value| value.get("type").is_some());
    if has_legacy_format && has_canonical_format {
        return Err(serde_json::Error::custom(
            "dynamic tools must use either canonical or legacy format consistently",
        ));
    }
    if !has_legacy_format {
        return values.into_iter().map(serde_json::from_value).collect();
    }

    let tools = values
        .into_iter()
        .map(|value| {
            let tool: LegacyDynamicToolSpec = serde_json::from_value(value)?;
            let function = DynamicToolFunctionSpec {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
                defer_loading: tool.defer_loading.unwrap_or_else(|| {
                    tool.expose_to_context
                        .map(|visible| !visible)
                        .unwrap_or(false)
                }),
            };
            Ok((tool.namespace, function))
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    Ok(group_dynamic_tools_by_namespace(tools))
}

pub fn group_dynamic_tools_by_namespace(
    tools: Vec<(Option<String>, DynamicToolFunctionSpec)>,
) -> Vec<DynamicToolSpec> {
    let mut grouped_tools = Vec::with_capacity(tools.len());
    let mut namespace_indices = HashMap::<String, usize>::new();
    for (namespace, function) in tools {
        let Some(namespace) = namespace else {
            grouped_tools.push(DynamicToolSpec::Function(function));
            continue;
        };
        let function = DynamicToolNamespaceTool::Function(function);
        if let Some(index) = namespace_indices.get(&namespace).copied() {
            let DynamicToolSpec::Namespace(namespace) = &mut grouped_tools[index] else {
                unreachable!("namespace index must point to a namespace");
            };
            namespace.tools.push(function);
            continue;
        }
        namespace_indices.insert(namespace.clone(), grouped_tools.len());
        grouped_tools.push(DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
            name: namespace,
            description: String::new(),
            tools: vec![function],
        }));
    }
    grouped_tools
}

pub fn deserialize_dynamic_tool_specs<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<DynamicToolSpec>>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(values) = Option::<Vec<JsonValue>>::deserialize(deserializer)? else {
        return Ok(None);
    };
    normalize_dynamic_tool_specs(values)
        .map(Some)
        .map_err(D::Error::custom)
}
