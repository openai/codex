//! Connector-domain app metadata used by directory discovery, caching, and tool selection.
//!
//! The Serde implementations decode connector-directory response metadata and persist normalized
//! app information in the connector-directory disk cache. They do not define the app-server wire
//! format; `codex-app-server-protocol` owns separate API types for that boundary.

use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Branding supplied by the connector directory for an app.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppBranding {
    pub category: Option<String>,
    pub developer: Option<String>,
    pub website: Option<String>,
    pub privacy_policy: Option<String>,
    pub terms_of_service: Option<String>,
    pub is_discoverable_app: bool,
}

/// Review state supplied by the connector directory for an app.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppReview {
    pub status: String,
}

/// Screenshot metadata supplied by the connector directory for an app.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppScreenshot {
    pub url: Option<String>,
    #[serde(alias = "file_id")]
    pub file_id: Option<String>,
    #[serde(alias = "user_prompt")]
    pub user_prompt: String,
}

/// Extended metadata supplied by the connector directory for an app.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
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
    pub first_party_requires_install: Option<bool>,
    pub show_in_composer_when_unlinked: Option<bool>,
}

/// Connector metadata used by connector discovery, caching, and tool selection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    pub icon_assets: Option<HashMap<String, String>>,
    pub icon_dark_assets: Option<HashMap<String, String>>,
    pub distribution_channel: Option<String>,
    pub branding: Option<AppBranding>,
    pub app_metadata: Option<AppMetadata>,
    pub labels: Option<HashMap<String, String>>,
    pub install_url: Option<String>,
    #[serde(default)]
    pub is_accessible: bool,
    #[serde(default = "default_enabled")]
    pub is_enabled: bool,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
}

impl AppInfo {
    pub fn category(&self) -> Option<String> {
        self.branding
            .as_ref()
            .and_then(|branding| non_empty_category(branding.category.as_deref()))
            .or_else(|| {
                self.app_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.categories.as_ref())
                    .and_then(|categories| {
                        categories
                            .iter()
                            .find_map(|category| non_empty_category(Some(category.as_str())))
                    })
            })
    }
}

const fn default_enabled() -> bool {
    true
}

fn non_empty_category(category: Option<&str>) -> Option<String> {
    let category = category?.trim();
    if category.is_empty() {
        None
    } else {
        Some(category.to_string())
    }
}
