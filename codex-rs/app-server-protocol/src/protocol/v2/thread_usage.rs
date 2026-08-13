use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadUsage {
    pub thread_id: String,
    pub estimated_usage_credits_micros: i64,
    pub estimated_usage_usd_micros: Option<i64>,
    pub groups: Vec<ThreadUsageBreakdownGroup>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadUsageBreakdownGroup {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub speed: Option<String>,
    pub estimated_usage_credits_micros: i64,
    pub net_new_input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}
