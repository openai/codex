use crate::JsonSchema;
use crate::TS;
use codex_utils_path_uri::LegacyAppPathString;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FeedbackUploadParams {
    pub classification: String,
    #[ts(optional = nullable)]
    pub reason: Option<String>,
    #[ts(optional = nullable)]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_logs: bool,
    #[ts(optional = nullable)]
    pub extra_log_files: Option<Vec<PathBuf>>,
    #[ts(optional = nullable)]
    pub tags: Option<BTreeMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FeedbackUploadResponse {
    pub thread_id: String,
    /// Whitespace-normalized SHA-256 of the session base instructions, matching the
    /// uploaded `prompt_hash` tag. Does not include later developer messages.
    /// Null when the reported rollout has no prompt metadata.
    #[serde(default)]
    pub prompt_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FeedbackSaveParams {
    #[serde(flatten)]
    pub feedback: FeedbackUploadParams,
    /// Absolute, previously unused ZIP path on the app-server host.
    /// The caller owns this file, including after a disconnected RPC.
    pub save_path: LegacyAppPathString,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FeedbackSaveResponse {
    pub saved_path: LegacyAppPathString,
    pub thread_id: String,
    /// Same prompt hash as feedback/upload; null when prompt metadata is absent.
    pub prompt_hash: Option<String>,
}
