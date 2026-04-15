#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::ListResourceTemplatesResult;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::ListResourcesResult;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::PaginatedRequestParams;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::ReadResourceRequestParams;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::ReadResourceResult;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::RequestId;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::Resource;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::ResourceTemplate;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::Tool;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rmcp::model::ToolAnnotations;

#[cfg(target_arch = "wasm32")]
pub(crate) use codex_protocol::mcp::RequestId;
#[cfg(target_arch = "wasm32")]
use schemars::JsonSchema;
#[cfg(target_arch = "wasm32")]
use serde::Deserialize;
#[cfg(target_arch = "wasm32")]
use serde::Serialize;

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolAnnotations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tool {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Resource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
    #[serde(default)]
    pub uri_template: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct PaginatedRequestParams {
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListResourcesResult {
    #[serde(default)]
    pub resources: Vec<Resource>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListResourceTemplatesResult {
    #[serde(default)]
    pub resource_templates: Vec<ResourceTemplate>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadResourceRequestParams {
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadResourceResult {
    #[serde(default)]
    pub contents: Vec<serde_json::Value>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}
