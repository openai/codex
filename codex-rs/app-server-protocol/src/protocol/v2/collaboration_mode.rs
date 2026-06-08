use codex_protocol::config_types::CollaborationModeMask as CoreCollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ReasoningEffort;
use schemars::JsonSchema;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Deserialize;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Serialize;
use ts_rs::TS;

/// EXPERIMENTAL - list collaboration mode presets.
#[derive(Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct CollaborationModeListParams {}

/// EXPERIMENTAL - collaboration mode preset metadata for clients.
#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct CollaborationModeMask {
    pub name: String,
    pub mode: Option<ModeKind>,
    pub model: Option<String>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(rename = "reasoning_effort")
    )]
    #[ts(rename = "reasoning_effort")]
    pub reasoning_effort: Option<Option<ReasoningEffort>>,
}

impl From<CoreCollaborationModeMask> for CollaborationModeMask {
    fn from(value: CoreCollaborationModeMask) -> Self {
        Self {
            name: value.name,
            mode: value.mode,
            model: value.model,
            reasoning_effort: value.reasoning_effort,
        }
    }
}

/// EXPERIMENTAL - collaboration mode presets response.
#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct CollaborationModeListResponse {
    pub data: Vec<CollaborationModeMask>,
}
