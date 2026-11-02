//! Sparse model-catalog wording for generic, selected, and recommended plugins.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Sparse plugin wording families, independent of runtime availability and authorization.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct PluginMessages {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generic: Option<GenericPluginMessages>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<SelectedPluginMessages>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommendations: Option<PluginRecommendationMessages>,
}

/// Literal generic plugin wording. Missing, null, blank, or invalid values use bundled text.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct GenericPluginMessages {
    /// Literal text, up to 500 UTF-8 bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    /// Up to 2,000 UTF-8 bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_instructions: Option<String>,
}

/// Selected-plugin wording. Fact-bearing fields require exactly one documented placeholder.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct SelectedPluginMessages {
    /// Up to 500 UTF-8 bytes. Selected intro requires `{{ plugin_name }}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    /// Up to 500 UTF-8 bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_tools: Option<String>,
    /// Up to 500 UTF-8 bytes; requires `{{ skill_namespace }}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_namespace: Option<String>,
    /// Up to 500 UTF-8 bytes; requires `{{ apps }}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apps: Option<String>,
    /// Up to 500 UTF-8 bytes; requires `{{ mcp_servers }}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<String>,
    /// Up to 500 UTF-8 bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closing: Option<String>,
    /// Nonblank omission notice, up to 500 UTF-8 bytes, including any desired separator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted: Option<String>,
}

/// Literal recommendation introduction; plugin identities remain supplied by the harness.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct PluginRecommendationMessages {
    /// Literal text, up to 500 UTF-8 bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
