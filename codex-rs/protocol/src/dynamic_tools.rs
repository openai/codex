use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

use crate::models::FunctionCallOutputContentItem;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallRequest {
    pub call_id: String,
    pub turn_id: String,
    pub tool: String,
    pub arguments: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolResponse {
    pub call_id: String,
    pub output: Option<String>,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub content_items: Option<Vec<FunctionCallOutputContentItem>>,
}
