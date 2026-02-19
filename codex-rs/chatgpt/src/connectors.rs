use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;

use codex_core::config::Config;
use codex_core::default_client::is_first_party_chat_originator;
use codex_core::default_client::originator;
use codex_core::features::Feature;
use codex_core::token_data::TokenData;
use serde::Deserialize;
use std::time::Duration;
use std::time::Instant;

use crate::chatgpt_client::chatgpt_get_request_with_timeout;
use crate::chatgpt_token::get_chatgpt_token_data;
use crate::chatgpt_token::init_chatgpt_token_from_auth;

use codex_core::connectors::AppBranding;
pub use codex_core::connectors::AppInfo;
use codex_core::connectors::AppMetadata;
use codex_core::connectors::CONNECTORS_CACHE_TTL;
pub use codex_core::connectors::connector_display_label;
use codex_core::connectors::connector_install_url;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools_with_options;
pub use codex_core::connectors::list_cached_accessible_connectors_from_mcp_tools;
use codex_core::connectors::merge_connectors;
pub use codex_core::connectors::with_app_enabled_state;

#[derive(Debug, Deserialize)]
struct DirectoryListResponse {
    apps: Vec<DirectoryApp>,
    #[serde(alias = "nextToken")]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct DirectoryApp {
    id: String,
    name: String,
    description: Option<String>,
    #[serde(alias = "appMetadata")]
    app_metadata: Option<AppMetadata>,
    branding: Option<AppBranding>,
    labels: Option<HashMap<String, String>>,
    #[serde(alias = "logoUrl")]
    logo_url: Option<String>,
    #[serde(alias = "logoUrlDark")]
    logo_url_dark: Option<String>,
    #[serde(alias = "distributionChannel")]
    distribution_channel: Option<String>,
    visibility: Option<String>,
}

const DIRECTORY_CONNECTORS_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, PartialEq, Eq)]
struct AllConnectorsCacheKey {
    chatgpt_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

#[derive(Clone)]
struct CachedAllConnectors {
    key: AllConnectorsCacheKey,
    expires_at: Instant,
    connectors: Vec<AppInfo>,
}

static ALL_CONNECTORS_CACHE: LazyLock<StdMutex<Option<CachedAllConnectors>>> =
    LazyLock::new(|| StdMutex::new(None));

pub async fn list_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    if !config.features.enabled(Feature::Apps) {
        return Ok(Vec::new());
    }
    let (connectors_result, accessible_result) = tokio::join!(
        list_all_connectors(config),
        list_accessible_connectors_from_mcp_tools(config),
    );
    let connectors = connectors_result?;
    let accessible = accessible_result?;
    Ok(with_app_enabled_state(
        merge_connectors_with_accessible(connectors, accessible, true),
        config,
    ))
}

pub async fn list_all_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    list_all_connectors_with_options(config, false).await
}

pub async fn list_cached_all_connectors(config: &Config) -> Option<Vec<AppInfo>> {
    if !config.features.enabled(Feature::Apps) {
        return Some(Vec::new());
    }

    if init_chatgpt_token_from_auth(&config.codex_home, config.cli_auth_credentials_store_mode)
        .await
        .is_err()
    {
        return None;
    }
    let token_data = get_chatgpt_token_data()?;
    let cache_key = all_connectors_cache_key(config, &token_data);
    read_cached_all_connectors(&cache_key)
}

pub async fn list_all_connectors_with_options(
    config: &Config,
    force_refetch: bool,
) -> anyhow::Result<Vec<AppInfo>> {
    if !config.features.enabled(Feature::Apps) {
        return Ok(Vec::new());
    }
    init_chatgpt_token_from_auth(&config.codex_home, config.cli_auth_credentials_store_mode)
        .await?;

    let token_data =
        get_chatgpt_token_data().ok_or_else(|| anyhow::anyhow!("ChatGPT token not available"))?;
    let cache_key = all_connectors_cache_key(config, &token_data);
    if !force_refetch && let Some(cached_connectors) = read_cached_all_connectors(&cache_key) {
        return Ok(cached_connectors);
    }

    let mut apps = list_directory_connectors(config).await?;
    if token_data.id_token.is_workspace_account() {
        apps.extend(list_workspace_connectors(config).await?);
    }
    let mut connectors = merge_directory_apps(apps)
        .into_iter()
        .map(directory_app_to_app_info)
        .collect::<Vec<_>>();
    for connector in &mut connectors {
        let install_url = match connector.install_url.take() {
            Some(install_url) => install_url,
            None => connector_install_url(&connector.name, &connector.id),
        };
        connector.name = normalize_connector_name(&connector.name, &connector.id);
        connector.description = normalize_connector_value(connector.description.as_deref());
        connector.install_url = Some(install_url);
        connector.is_accessible = false;
    }
    connectors.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    write_cached_all_connectors(cache_key, &connectors);
    Ok(connectors)
}

fn all_connectors_cache_key(config: &Config, token_data: &TokenData) -> AllConnectorsCacheKey {
    AllConnectorsCacheKey {
        chatgpt_base_url: config.chatgpt_base_url.clone(),
        account_id: token_data.account_id.clone(),
        chatgpt_user_id: token_data.id_token.chatgpt_user_id.clone(),
        is_workspace_account: token_data.id_token.is_workspace_account(),
    }
}

fn read_cached_all_connectors(cache_key: &AllConnectorsCacheKey) -> Option<Vec<AppInfo>> {
    let mut cache_guard = ALL_CONNECTORS_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = Instant::now();

    if let Some(cached) = cache_guard.as_ref() {
        if now < cached.expires_at && cached.key == *cache_key {
            return Some(cached.connectors.clone());
        }
        if now >= cached.expires_at {
            *cache_guard = None;
        }
    }

    None
}

fn write_cached_all_connectors(cache_key: AllConnectorsCacheKey, connectors: &[AppInfo]) {
    let mut cache_guard = ALL_CONNECTORS_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *cache_guard = Some(CachedAllConnectors {
        key: cache_key,
        expires_at: Instant::now() + CONNECTORS_CACHE_TTL,
        connectors: connectors.to_vec(),
    });
}

pub fn merge_connectors_with_accessible(
    connectors: Vec<AppInfo>,
    accessible_connectors: Vec<AppInfo>,
    all_connectors_loaded: bool,
) -> Vec<AppInfo> {
    let accessible_connectors = if all_connectors_loaded {
        let connector_ids: HashSet<&str> = connectors
            .iter()
            .map(|connector| connector.id.as_str())
            .collect();
        accessible_connectors
            .into_iter()
            .filter(|connector| connector_ids.contains(connector.id.as_str()))
            .collect()
    } else {
        accessible_connectors
    };
    let merged = merge_connectors(connectors, accessible_connectors);
    filter_disallowed_connectors(merged)
}

async fn list_directory_connectors(config: &Config) -> anyhow::Result<Vec<DirectoryApp>> {
    let mut apps = Vec::new();
    let mut next_token: Option<String> = None;
    loop {
        let path = match next_token.as_deref() {
            Some(token) => {
                let encoded_token = urlencoding::encode(token);
                format!("/connectors/directory/list?tier=categorized&token={encoded_token}")
            }
            None => "/connectors/directory/list?tier=categorized".to_string(),
        };
        let response: DirectoryListResponse =
            chatgpt_get_request_with_timeout(config, path, Some(DIRECTORY_CONNECTORS_TIMEOUT))
                .await?;
        apps.extend(
            response
                .apps
                .into_iter()
                .filter(|app| !is_hidden_directory_app(app)),
        );
        next_token = response
            .next_token
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());
        if next_token.is_none() {
            break;
        }
    }
    Ok(apps)
}

async fn list_workspace_connectors(config: &Config) -> anyhow::Result<Vec<DirectoryApp>> {
    let response: anyhow::Result<DirectoryListResponse> = chatgpt_get_request_with_timeout(
        config,
        "/connectors/directory/list_workspace".to_string(),
        Some(DIRECTORY_CONNECTORS_TIMEOUT),
    )
    .await;
    match response {
        Ok(response) => Ok(response
            .apps
            .into_iter()
            .filter(|app| !is_hidden_directory_app(app))
            .collect()),
        Err(_) => Ok(Vec::new()),
    }
}

fn merge_directory_apps(apps: Vec<DirectoryApp>) -> Vec<DirectoryApp> {
    let mut merged: HashMap<String, DirectoryApp> = HashMap::new();
    for app in apps {
        if let Some(existing) = merged.get_mut(&app.id) {
            merge_directory_app(existing, app);
        } else {
            merged.insert(app.id.clone(), app);
        }
    }
    merged.into_values().collect()
}

fn merge_directory_app(existing: &mut DirectoryApp, incoming: DirectoryApp) {
    let DirectoryApp {
        id: _,
        name,
        description,
        app_metadata,
        branding,
        labels,
        logo_url,
        logo_url_dark,
        distribution_channel,
        visibility: _,
    } = incoming;

    let incoming_name_is_empty = name.trim().is_empty();
    if existing.name.trim().is_empty() && !incoming_name_is_empty {
        existing.name = name;
    }

    let incoming_description_present = description
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let existing_description_present = existing
        .description
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if !existing_description_present && incoming_description_present {
        existing.description = description;
    }

    if existing.logo_url.is_none() && logo_url.is_some() {
        existing.logo_url = logo_url;
    }
    if existing.logo_url_dark.is_none() && logo_url_dark.is_some() {
        existing.logo_url_dark = logo_url_dark;
    }
    if existing.distribution_channel.is_none() && distribution_channel.is_some() {
        existing.distribution_channel = distribution_channel;
    }

    if let Some(incoming_branding) = branding {
        if let Some(existing_branding) = existing.branding.as_mut() {
            if existing_branding.category.is_none() && incoming_branding.category.is_some() {
                existing_branding.category = incoming_branding.category;
            }
            if existing_branding.developer.is_none() && incoming_branding.developer.is_some() {
                existing_branding.developer = incoming_branding.developer;
            }
            if existing_branding.website.is_none() && incoming_branding.website.is_some() {
                existing_branding.website = incoming_branding.website;
            }
            if existing_branding.privacy_policy.is_none()
                && incoming_branding.privacy_policy.is_some()
            {
                existing_branding.privacy_policy = incoming_branding.privacy_policy;
            }
            if existing_branding.terms_of_service.is_none()
                && incoming_branding.terms_of_service.is_some()
            {
                existing_branding.terms_of_service = incoming_branding.terms_of_service;
            }
            if !existing_branding.is_discoverable_app && incoming_branding.is_discoverable_app {
                existing_branding.is_discoverable_app = true;
            }
        } else {
            existing.branding = Some(incoming_branding);
        }
    }

    if let Some(incoming_app_metadata) = app_metadata {
        if let Some(existing_app_metadata) = existing.app_metadata.as_mut() {
            if existing_app_metadata.review.is_none() && incoming_app_metadata.review.is_some() {
                existing_app_metadata.review = incoming_app_metadata.review;
            }
            if existing_app_metadata.categories.is_none()
                && incoming_app_metadata.categories.is_some()
            {
                existing_app_metadata.categories = incoming_app_metadata.categories;
            }
            if existing_app_metadata.sub_categories.is_none()
                && incoming_app_metadata.sub_categories.is_some()
            {
                existing_app_metadata.sub_categories = incoming_app_metadata.sub_categories;
            }
            if existing_app_metadata.seo_description.is_none()
                && incoming_app_metadata.seo_description.is_some()
            {
                existing_app_metadata.seo_description = incoming_app_metadata.seo_description;
            }
            if existing_app_metadata.screenshots.is_none()
                && incoming_app_metadata.screenshots.is_some()
            {
                existing_app_metadata.screenshots = incoming_app_metadata.screenshots;
            }
            if existing_app_metadata.developer.is_none()
                && incoming_app_metadata.developer.is_some()
            {
                existing_app_metadata.developer = incoming_app_metadata.developer;
            }
            if existing_app_metadata.version.is_none() && incoming_app_metadata.version.is_some() {
                existing_app_metadata.version = incoming_app_metadata.version;
            }
            if existing_app_metadata.version_id.is_none()
                && incoming_app_metadata.version_id.is_some()
            {
                existing_app_metadata.version_id = incoming_app_metadata.version_id;
            }
            if existing_app_metadata.version_notes.is_none()
                && incoming_app_metadata.version_notes.is_some()
            {
                existing_app_metadata.version_notes = incoming_app_metadata.version_notes;
            }
            if existing_app_metadata.first_party_type.is_none()
                && incoming_app_metadata.first_party_type.is_some()
            {
                existing_app_metadata.first_party_type = incoming_app_metadata.first_party_type;
            }
            if existing_app_metadata.first_party_requires_install.is_none()
                && incoming_app_metadata.first_party_requires_install.is_some()
            {
                existing_app_metadata.first_party_requires_install =
                    incoming_app_metadata.first_party_requires_install;
            }
            if existing_app_metadata
                .show_in_composer_when_unlinked
                .is_none()
                && incoming_app_metadata
                    .show_in_composer_when_unlinked
                    .is_some()
            {
                existing_app_metadata.show_in_composer_when_unlinked =
                    incoming_app_metadata.show_in_composer_when_unlinked;
            }
        } else {
            existing.app_metadata = Some(incoming_app_metadata);
        }
    }

    if existing.labels.is_none() && labels.is_some() {
        existing.labels = labels;
    }
}

fn is_hidden_directory_app(app: &DirectoryApp) -> bool {
    matches!(app.visibility.as_deref(), Some("HIDDEN"))
}

fn directory_app_to_app_info(app: DirectoryApp) -> AppInfo {
    AppInfo {
        id: app.id,
        name: app.name,
        description: app.description,
        logo_url: app.logo_url,
        logo_url_dark: app.logo_url_dark,
        distribution_channel: app.distribution_channel,
        branding: app.branding,
        app_metadata: app.app_metadata,
        labels: app.labels,
        install_url: None,
        is_accessible: false,
        is_enabled: true,
    }
}

fn normalize_connector_name(name: &str, connector_id: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        connector_id.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_connector_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

const DISALLOWED_CONNECTOR_IDS: &[&str] = &[
    "asdk_app_6938a94a61d881918ef32cb999ff937c",
    "connector_2b0a9009c9c64bf9933a3dae3f2b1254",
    "connector_68de829bf7648191acd70a907364c67c",
    "connector_68e004f14af881919eb50893d3d9f523",
    "connector_69272cb413a081919685ec3c88d1744e",
    "connector_0f9c9d4592e54d0a9a12b3f44a1e2010",
];
const FIRST_PARTY_CHAT_DISALLOWED_CONNECTOR_IDS: &[&str] =
    &["connector_0f9c9d4592e54d0a9a12b3f44a1e2010"];
const DISALLOWED_CONNECTOR_PREFIX: &str = "connector_openai_";

fn filter_disallowed_connectors(connectors: Vec<AppInfo>) -> Vec<AppInfo> {
    filter_disallowed_connectors_for_originator(connectors, originator().value.as_str())
}

fn filter_disallowed_connectors_for_originator(
    connectors: Vec<AppInfo>,
    originator_value: &str,
) -> Vec<AppInfo> {
    let disallowed_connector_ids = if is_first_party_chat_originator(originator_value) {
        FIRST_PARTY_CHAT_DISALLOWED_CONNECTOR_IDS
    } else {
        DISALLOWED_CONNECTOR_IDS
    };

    connectors
        .into_iter()
        .filter(|connector| is_connector_allowed(connector, disallowed_connector_ids))
        .collect()
}

fn is_connector_allowed(connector: &AppInfo, disallowed_connector_ids: &[&str]) -> bool {
    let connector_id = connector.id.as_str();
    !connector_id.starts_with(DISALLOWED_CONNECTOR_PREFIX)
        && !disallowed_connector_ids.contains(&connector_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn app(id: &str) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
        }
    }

    #[test]
    fn allows_asdk_connectors() {
        let filtered = filter_disallowed_connectors(vec![app("asdk_app_hidden"), app("alpha")]);
        assert_eq!(filtered, vec![app("asdk_app_hidden"), app("alpha")]);
    }

    #[test]
    fn allows_whitelisted_asdk_connectors() {
        let filtered = filter_disallowed_connectors(vec![
            app("asdk_app_69781557cc1481919cf5e9824fa2e792"),
            app("beta"),
        ]);
        assert_eq!(
            filtered,
            vec![
                app("asdk_app_69781557cc1481919cf5e9824fa2e792"),
                app("beta")
            ]
        );
    }

    #[test]
    fn filters_openai_prefixed_connectors() {
        let filtered = filter_disallowed_connectors(vec![
            app("connector_openai_foo"),
            app("connector_openai_bar"),
            app("gamma"),
        ]);
        assert_eq!(filtered, vec![app("gamma")]);
    }

    #[test]
    fn filters_disallowed_connector_ids() {
        let filtered = filter_disallowed_connectors(vec![
            app("asdk_app_6938a94a61d881918ef32cb999ff937c"),
            app("delta"),
        ]);
        assert_eq!(filtered, vec![app("delta")]);
    }

    #[test]
    fn first_party_chat_originator_filters_target_and_openai_prefixed_connectors() {
        let filtered = filter_disallowed_connectors_for_originator(
            vec![
                app("connector_openai_foo"),
                app("asdk_app_6938a94a61d881918ef32cb999ff937c"),
                app("connector_0f9c9d4592e54d0a9a12b3f44a1e2010"),
            ],
            "codex_atlas",
        );
        assert_eq!(
            filtered,
            vec![app("asdk_app_6938a94a61d881918ef32cb999ff937c"),]
        );
    }

    fn merged_app(id: &str, is_accessible: bool) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some(connector_install_url(id, id)),
            is_accessible,
            is_enabled: true,
        }
    }

    #[test]
    fn excludes_accessible_connectors_not_in_all_when_all_loaded() {
        let merged = merge_connectors_with_accessible(
            vec![app("alpha")],
            vec![app("alpha"), app("beta")],
            true,
        );
        assert_eq!(merged, vec![merged_app("alpha", true)]);
    }

    #[test]
    fn keeps_accessible_connectors_not_in_all_while_all_loading() {
        let merged = merge_connectors_with_accessible(
            vec![app("alpha")],
            vec![app("alpha"), app("beta")],
            false,
        );
        assert_eq!(
            merged,
            vec![merged_app("alpha", true), merged_app("beta", true)]
        );
    }
}
