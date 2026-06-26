use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchSearchParams {
    pub thread_id: String,
    pub query: String,
    /// Optional result limit. Defaults to the runtime `tool_search` default.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchSearchResponse {
    pub indexed_tool_count: u32,
    pub matching_tool_count: u32,
    pub requested_limit: u32,
    pub effective_limit: u32,
    pub top_k_truncated: bool,
    /// The exact coalesced tool payload current `tool_search` returns for this query.
    pub tools: Vec<JsonValue>,
    pub results: Vec<ToolSearchSearchResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ToolSearchSearchResult {
    pub rank: u32,
    pub index: u32,
    pub score: Option<f32>,
    pub source_name: Option<String>,
    pub source_description: Option<String>,
    pub searchable_text: String,
    pub tools: Vec<JsonValue>,
}
