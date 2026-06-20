//! Raw plugin manifest document types.
//!
//! Resource references stay as strings here so each caller can resolve them for its file system.

use codex_config::HooksFile;
use serde::Deserialize;
use serde_json::Value as JsonValue;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawPluginManifest {
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) version: Option<String>,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) keywords: Vec<String>,
    #[serde(default)]
    pub(super) skills: Option<RawPluginManifestPaths>,
    #[serde(default)]
    pub(super) mcp_servers: Option<RawPluginManifestMcpServers>,
    #[serde(default)]
    pub(super) apps: Option<String>,
    #[serde(default)]
    pub(super) hooks: Option<RawPluginManifestHooks>,
    #[serde(default)]
    pub(super) interface: Option<RawPluginManifestInterface>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawPluginManifestInterface {
    #[serde(default)]
    pub(super) display_name: Option<String>,
    #[serde(default)]
    pub(super) short_description: Option<String>,
    #[serde(default)]
    pub(super) long_description: Option<String>,
    #[serde(default)]
    pub(super) developer_name: Option<String>,
    #[serde(default)]
    pub(super) category: Option<String>,
    #[serde(default)]
    pub(super) capabilities: Vec<String>,
    #[serde(default)]
    #[serde(alias = "websiteURL")]
    pub(super) website_url: Option<String>,
    #[serde(default)]
    #[serde(alias = "privacyPolicyURL")]
    pub(super) privacy_policy_url: Option<String>,
    #[serde(default)]
    #[serde(alias = "termsOfServiceURL")]
    pub(super) terms_of_service_url: Option<String>,
    #[serde(default)]
    pub(super) default_prompt: Option<RawPluginManifestDefaultPrompt>,
    #[serde(default)]
    pub(super) brand_color: Option<String>,
    #[serde(default)]
    pub(super) composer_icon: Option<String>,
    #[serde(default)]
    pub(super) logo: Option<String>,
    #[serde(default)]
    pub(super) screenshots: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum RawPluginManifestDefaultPrompt {
    String(String),
    List(Vec<RawPluginManifestDefaultPromptEntry>),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum RawPluginManifestDefaultPromptEntry {
    String(String),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum RawPluginManifestPaths {
    Path(String),
    Paths(Vec<String>),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum RawPluginManifestMcpServers {
    Path(String),
    Object(std::collections::BTreeMap<String, JsonValue>),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum RawPluginManifestHooks {
    Path(String),
    Paths(Vec<String>),
    Inline(Box<HooksFile>),
    InlineList(Vec<HooksFile>),
    Invalid(JsonValue),
}

pub(super) fn parse(contents: &str) -> Result<RawPluginManifest, serde_json::Error> {
    serde_json::from_str(contents)
}
