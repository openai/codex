use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase")]
pub enum ImageGenerationFailure {
    UsageLimitExceeded {
        #[serde(rename = "limitId")]
        #[ts(rename = "limitId")]
        limit_id: String,
        #[serde(rename = "resetsAt")]
        #[ts(rename = "resetsAt")]
        #[ts(type = "number | null")]
        resets_at: Option<i64>,
    },
}

// Standalone image-generation item owned by the image extension. This is also
// the field-level representation exposed by app-server; core and rollout
// persistence only carry it inside an ExtensionItem envelope.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct ImageGenerationItem {
    pub id: String,
    pub status: String,
    pub revised_prompt: Option<String>,
    pub result: String,
    #[serde(default)]
    #[ts(optional)]
    pub transparent_background: Option<bool>,
    #[serde(default)]
    pub failure: Option<ImageGenerationFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub saved_path: Option<AbsolutePathBuf>,
}
