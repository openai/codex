//! Recoverable provider failures shared by resume and inference admission.

use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

/// Structured JSON-RPC error data. Unknown errors must retain the client's normal fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    export_to = "v2/"
)]
pub enum ModelProviderError {
    ModelProviderMismatch {
        #[schemars(rename = "modelProvider")]
        model_provider: String,
        #[schemars(rename = "currentModelProvider")]
        current_model_provider: String,
    },
    ModelProviderNotConfigured {
        #[schemars(rename = "modelProvider")]
        model_provider: String,
    },
    ModelProviderPolicyConflict {
        #[schemars(rename = "modelProvider")]
        model_provider: String,
        #[schemars(rename = "requiredModelProvider")]
        required_model_provider: String,
    },
}

impl ModelProviderError {
    pub fn title(&self) -> &'static str {
        match self {
            Self::ModelProviderMismatch { .. } => "Model provider mismatch",
            Self::ModelProviderNotConfigured { .. } => "Model provider unavailable",
            Self::ModelProviderPolicyConflict { .. } => "Model provider blocked by policy",
        }
    }
}

impl std::fmt::Display for ModelProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelProviderMismatch { model_provider, current_model_provider } => write!(f,
                "This chat uses {model_provider}, but your current provider is {current_model_provider}. Switch to {model_provider} to continue."),
            Self::ModelProviderNotConfigured { model_provider } => write!(f,
                "This chat uses model provider “{model_provider}”, which is no longer configured. Restore this provider’s configuration to continue."),
            Self::ModelProviderPolicyConflict { model_provider, required_model_provider }
                if model_provider != required_model_provider => write!(f,
                "This chat uses model provider “{model_provider}”, but your organization requires “{required_model_provider}”. Contact your administrator or start a new chat using the required provider."),
            Self::ModelProviderPolicyConflict { .. } => f.write_str(
                "This chat’s model provider settings no longer match your organization’s requirements. Contact your administrator or start a new chat using the current provider settings."),
        }
    }
}

impl std::error::Error for ModelProviderError {}

#[cfg(test)]
#[path = "model_provider_error_tests.rs"]
mod tests;
