//! One model response on an ephemeral fork that inherits the source's configuration and context.

use super::ThreadSource;
use crate::JsonSchema;
use crate::RequestId;
use crate::TS;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadSampleParams {
    pub thread_id: String,
    /// The loaded source's latest completed, failed, or interrupted turn.
    /// Stale or active sources are rejected.
    pub expected_turn_id: String,
    pub prompt: String,
    /// Applied only when the provider and model support cache-preserving reasoning-effort updates.
    #[ts(optional = nullable)]
    pub effort: Option<ReasoningEffort>,
    #[ts(optional = nullable)]
    pub thread_source: Option<ThreadSource>,
    #[ts(optional = nullable)]
    pub turn_trigger: Option<String>,
    /// Optional metadata to enrich Codex's ResponsesAPI turn metadata.
    #[ts(optional = nullable)]
    pub responsesapi_client_metadata: Option<HashMap<String, String>>,
    /// Disable MCP servers when the inherited window uses incremental tools on Responses Lite.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_mcp: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadSampleResponse {
    pub source_turn_id: String,
    pub response_id: String,
    /// Assistant text from the response, or null if it attempted a tool call or completed without text.
    pub text: Option<String>,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct ThreadSampleCancelParams {
    pub request_id: RequestId,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadSampleCancelResponse {}
