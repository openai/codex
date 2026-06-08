use std::collections::HashMap;

use codex_app_server_protocol::AppBranding;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppMetadata;
use codex_app_server_protocol::AppReview;
use codex_app_server_protocol::AppScreenshot;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfoWire {
    id: String,
    name: String,
    description: Option<String>,
    logo_url: Option<String>,
    logo_url_dark: Option<String>,
    distribution_channel: Option<String>,
    branding: Option<AppBrandingWire>,
    app_metadata: Option<AppMetadataWire>,
    labels: Option<HashMap<String, String>>,
    install_url: Option<String>,
    #[serde(default)]
    is_accessible: bool,
    #[serde(default = "default_enabled")]
    is_enabled: bool,
    #[serde(default)]
    plugin_display_names: Vec<String>,
}

impl From<AppInfoWire> for AppInfo {
    fn from(value: AppInfoWire) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            logo_url: value.logo_url,
            logo_url_dark: value.logo_url_dark,
            distribution_channel: value.distribution_channel,
            branding: value.branding.map(Into::into),
            app_metadata: value.app_metadata.map(Into::into),
            labels: value.labels,
            install_url: value.install_url,
            is_accessible: value.is_accessible,
            is_enabled: value.is_enabled,
            plugin_display_names: value.plugin_display_names,
        }
    }
}

impl From<&AppInfo> for AppInfoWire {
    fn from(value: &AppInfo) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            description: value.description.clone(),
            logo_url: value.logo_url.clone(),
            logo_url_dark: value.logo_url_dark.clone(),
            distribution_channel: value.distribution_channel.clone(),
            branding: value.branding.as_ref().map(Into::into),
            app_metadata: value.app_metadata.as_ref().map(Into::into),
            labels: value.labels.clone(),
            install_url: value.install_url.clone(),
            is_accessible: value.is_accessible,
            is_enabled: value.is_enabled,
            plugin_display_names: value.plugin_display_names.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppBrandingWire {
    category: Option<String>,
    developer: Option<String>,
    website: Option<String>,
    privacy_policy: Option<String>,
    terms_of_service: Option<String>,
    is_discoverable_app: bool,
}

impl From<AppBrandingWire> for AppBranding {
    fn from(value: AppBrandingWire) -> Self {
        Self {
            category: value.category,
            developer: value.developer,
            website: value.website,
            privacy_policy: value.privacy_policy,
            terms_of_service: value.terms_of_service,
            is_discoverable_app: value.is_discoverable_app,
        }
    }
}

impl From<&AppBranding> for AppBrandingWire {
    fn from(value: &AppBranding) -> Self {
        Self {
            category: value.category.clone(),
            developer: value.developer.clone(),
            website: value.website.clone(),
            privacy_policy: value.privacy_policy.clone(),
            terms_of_service: value.terms_of_service.clone(),
            is_discoverable_app: value.is_discoverable_app,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppReviewWire {
    status: String,
}

impl From<AppReviewWire> for AppReview {
    fn from(value: AppReviewWire) -> Self {
        Self {
            status: value.status,
        }
    }
}

impl From<&AppReview> for AppReviewWire {
    fn from(value: &AppReview) -> Self {
        Self {
            status: value.status.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppScreenshotWire {
    url: Option<String>,
    #[serde(alias = "file_id")]
    file_id: Option<String>,
    #[serde(alias = "user_prompt")]
    user_prompt: String,
}

impl From<AppScreenshotWire> for AppScreenshot {
    fn from(value: AppScreenshotWire) -> Self {
        Self {
            url: value.url,
            file_id: value.file_id,
            user_prompt: value.user_prompt,
        }
    }
}

impl From<&AppScreenshot> for AppScreenshotWire {
    fn from(value: &AppScreenshot) -> Self {
        Self {
            url: value.url.clone(),
            file_id: value.file_id.clone(),
            user_prompt: value.user_prompt.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppMetadataWire {
    review: Option<AppReviewWire>,
    categories: Option<Vec<String>>,
    sub_categories: Option<Vec<String>>,
    seo_description: Option<String>,
    screenshots: Option<Vec<AppScreenshotWire>>,
    developer: Option<String>,
    version: Option<String>,
    version_id: Option<String>,
    version_notes: Option<String>,
    first_party_type: Option<String>,
    first_party_requires_install: Option<bool>,
    show_in_composer_when_unlinked: Option<bool>,
}

impl From<AppMetadataWire> for AppMetadata {
    fn from(value: AppMetadataWire) -> Self {
        Self {
            review: value.review.map(Into::into),
            categories: value.categories,
            sub_categories: value.sub_categories,
            seo_description: value.seo_description,
            screenshots: value
                .screenshots
                .map(|screenshots| screenshots.into_iter().map(Into::into).collect()),
            developer: value.developer,
            version: value.version,
            version_id: value.version_id,
            version_notes: value.version_notes,
            first_party_type: value.first_party_type,
            first_party_requires_install: value.first_party_requires_install,
            show_in_composer_when_unlinked: value.show_in_composer_when_unlinked,
        }
    }
}

impl From<&AppMetadata> for AppMetadataWire {
    fn from(value: &AppMetadata) -> Self {
        Self {
            review: value.review.as_ref().map(Into::into),
            categories: value.categories.clone(),
            sub_categories: value.sub_categories.clone(),
            seo_description: value.seo_description.clone(),
            screenshots: value
                .screenshots
                .as_ref()
                .map(|screenshots| screenshots.iter().map(Into::into).collect()),
            developer: value.developer.clone(),
            version: value.version.clone(),
            version_id: value.version_id.clone(),
            version_notes: value.version_notes.clone(),
            first_party_type: value.first_party_type.clone(),
            first_party_requires_install: value.first_party_requires_install,
            show_in_composer_when_unlinked: value.show_in_composer_when_unlinked,
        }
    }
}

pub(crate) fn deserialize_app_branding<'de, D>(
    deserializer: D,
) -> Result<Option<AppBranding>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<AppBrandingWire>::deserialize(deserializer).map(|branding| branding.map(Into::into))
}

pub(crate) fn deserialize_app_metadata<'de, D>(
    deserializer: D,
) -> Result<Option<AppMetadata>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<AppMetadataWire>::deserialize(deserializer).map(|metadata| metadata.map(Into::into))
}

fn default_enabled() -> bool {
    true
}
