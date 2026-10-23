//! Memory readiness and summary reads using a loaded thread's selected pipeline.

use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadMemoryReadParams {
    /// The thread must already be started or resumed.
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadMemoryReadResponse {
    /// Summary from the thread's selected memory version. Null when memory use is
    /// disabled or the selected summary is missing, unreadable, or blank.
    pub summary: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MemoryStatusParams {
    /// Required distinct consolidated threads. Defaults to 20; supported range is 1..=4096.
    #[ts(optional = nullable)]
    pub min_consolidated_threads: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MemoryStatusResponse {
    pub v2_consolidated_threads: u32,
    pub v2_ready: bool,
}
