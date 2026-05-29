use super::shared::default_enabled;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - list available apps/connectors.
pub struct AppsListParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    /// Optional thread id used to evaluate app feature gating from that thread's config.
    #[ts(optional = nullable)]
    pub thread_id: Option<String>,
    /// When true, bypass app caches and fetch the latest data from sources.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force_refetch: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - app metadata returned by app-list APIs.
pub struct AppBranding {
    pub category: Option<String>,
    pub developer: Option<String>,
    pub website: Option<String>,
    pub privacy_policy: Option<String>,
    pub terms_of_service: Option<String>,
    pub is_discoverable_app: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppReview {
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppScreenshot {
    pub url: Option<String>,
    #[serde(alias = "file_id")]
    pub file_id: Option<String>,
    #[serde(alias = "user_prompt")]
    pub user_prompt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppMetadata {
    pub review: Option<AppReview>,
    pub categories: Option<Vec<String>>,
    pub sub_categories: Option<Vec<String>>,
    pub seo_description: Option<String>,
    pub screenshots: Option<Vec<AppScreenshot>>,
    pub developer: Option<String>,
    pub version: Option<String>,
    pub version_id: Option<String>,
    pub version_notes: Option<String>,
    pub first_party_type: Option<String>,
    pub first_party_requires_install: Option<bool>,
    pub show_in_composer_when_unlinked: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - app metadata returned by app-list APIs.
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    pub distribution_channel: Option<String>,
    pub branding: Option<AppBranding>,
    pub app_metadata: Option<AppMetadata>,
    pub labels: Option<HashMap<String, String>>,
    pub install_url: Option<String>,
    #[serde(default)]
    pub is_accessible: bool,
    /// Whether this app is enabled in config.toml.
    /// Example:
    /// ```toml
    /// [apps.bad_app]
    /// enabled = false
    /// ```
    #[serde(default = "default_enabled")]
    pub is_enabled: bool,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - app metadata summary for plugin responses.
pub struct AppSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub install_url: Option<String>,
    pub needs_auth: bool,
}

impl From<AppInfo> for AppSummary {
    fn from(value: AppInfo) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            install_url: value.install_url,
            needs_auth: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - app list response.
pub struct AppsListResponse {
    pub data: Vec<AppInfo>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// If None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - notification emitted when the app list changes.
pub struct AppListUpdatedNotification {
    pub data: Vec<AppInfo>,
}
