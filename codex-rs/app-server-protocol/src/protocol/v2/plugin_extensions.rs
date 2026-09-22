//! Hosted plugin extension declarations carried by installed-plugin discovery.

use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginExtensions {
    #[serde(default)]
    pub entrypoints: Option<Vec<PluginEntrypoint>>,
    #[serde(default, alias = "settings_entrypoints")]
    pub settings_entrypoints: Vec<PluginEntrypoint>,
    #[serde(default)]
    pub settings: Vec<PluginSettings>,
    #[serde(default, alias = "thread_entrypoints")]
    pub thread_entrypoints: Vec<PluginEntrypoint>,
    #[serde(default, alias = "file_handlers")]
    pub file_handlers: Vec<PluginEntrypoint>,
    #[serde(default, alias = "search_mention_providers")]
    pub search_mention_providers: Vec<PluginSearchProvider>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase", export_to = "v2/")]
pub enum PluginEntrypoint {
    Global {
        #[serde(flatten)]
        presentation: PluginEntrypointPresentation,
        #[serde(default, rename = "quickAction", alias = "quick_action")]
        #[ts(rename = "quickAction")]
        quick_action: Option<PluginQuickAction>,
    },
    Settings {
        #[serde(flatten)]
        presentation: PluginEntrypointPresentation,
        #[serde(default, rename = "searchTerms", alias = "search_terms")]
        #[ts(rename = "searchTerms")]
        search_terms: Vec<String>,
    },
    Thread {
        #[serde(flatten)]
        presentation: PluginEntrypointPresentation,
    },
    File {
        #[serde(flatten)]
        presentation: PluginEntrypointPresentation,
        extensions: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginEntrypointPresentation {
    #[serde(alias = "app_id")]
    pub app_id: String,
    #[serde(alias = "tool_name")]
    pub tool_name: String,
    pub title: String,
    #[serde(alias = "resource_uri")]
    pub resource_uri: String,
    pub icons: Vec<PluginIcon>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginIcon {
    pub src: String,
    pub mime_type: Option<String>,
    pub sizes: Option<Vec<String>>,
    pub theme: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginQuickAction {
    pub title: String,
    pub icons: Vec<PluginIcon>,
    pub target: PluginQuickActionTarget,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase", export_to = "v2/")]
pub enum PluginQuickActionTarget {
    Tool {
        name: String,
        arguments: Option<serde_json::Value>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginSettings {
    #[serde(alias = "app_id")]
    pub app_id: String,
    #[serde(alias = "read_tool_name")]
    pub read_tool_name: String,
    #[serde(alias = "update_tool_name")]
    pub update_tool_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginSearchProvider {
    #[serde(alias = "app_id")]
    pub app_id: String,
    #[serde(alias = "tool_name")]
    pub tool_name: String,
    #[serde(alias = "link_id")]
    pub link_id: String,
    pub title: String,
    pub call: Option<PluginSearchProviderCall>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginSearchProviderCall {
    pub name: String,
    pub arguments: serde_json::Value,
    #[serde(rename = "_meta")]
    #[ts(rename = "_meta")]
    pub meta: serde_json::Value,
}
