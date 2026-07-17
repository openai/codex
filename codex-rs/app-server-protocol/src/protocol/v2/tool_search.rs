use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchInspectParams {
    pub thread_id: String,
    pub query: String,
    /// Optional result limit. Defaults to the runtime `tool_search` default.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchInspectResponse {
    pub indexed_tool_count: u32,
    pub matching_tool_count: u32,
    pub requested_limit: u32,
    pub effective_limit: u32,
    pub top_k_truncated: bool,
    pub results: Vec<ToolSearchInspectResult>,
    pub output_tools: Vec<ToolSearchInspectOutputTool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchInspectResult {
    pub rank: u32,
    pub index: u32,
    pub score: Option<f32>,
    pub source: Option<ToolSearchInspectSource>,
    pub tools: Vec<ToolSearchInspectTool>,
    pub searchable_text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchInspectSource {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchInspectTool {
    pub namespace: Option<String>,
    pub name: String,
    pub canonical_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchInspectOutputTool {
    pub namespace: Option<String>,
    pub tool_names: Vec<String>,
}
