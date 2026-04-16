use std::collections::HashMap;
use std::future::Future;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use std::time::Instant;

use codex_app_server_protocol::AppBranding;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppMetadata;
use serde::Deserialize;

pub mod accessible;
pub mod filter;
pub mod merge;
pub mod metadata;

pub const CONNECTORS_CACHE_TTL: Duration = Duration::from_secs(3600);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllConnectorsCacheKey {
    chatgpt_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

impl AllConnectorsCacheKey {
    pub fn new(
        chatgpt_base_url: String,
        account_id: Option<String>,
        chatgpt_user_id: Option<String>,
        is_workspace_account: bool,
    ) -> Self {
        Self {
            chatgpt_base_url,
            account_id,
            chatgpt_user_id,
            is_workspace_account,
        }
    }
}

#[derive(Clone)]
struct CachedAllConnectors {
    key: AllConnectorsCacheKey,
    expires_at: Instant,
    connectors: Vec<AppInfo>,
}

static ALL_CONNECTORS_CACHE: LazyLock<StdMutex<Option<CachedAllConnectors>>> =
    LazyLock::new(|| StdMutex::new(None));

#[derive(Debug, Deserialize)]
pub struct DirectoryListResponse {
    apps: Vec<DirectoryApp>,
    #[serde(alias = "nextToken")]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DirectoryApp {
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

pub fn cached_all_connectors(cache_key: &AllConnectorsCacheKey) -> Option<Vec<AppInfo>> {
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

pub async fn list_all_connectors_with_options<F, Fut>(
    cache_key: AllConnectorsCacheKey,
    is_workspace_account: bool,
    force_refetch: bool,
    mut fetch_page: F,
) -> anyhow::Result<Vec<AppInfo>>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = anyhow::Result<DirectoryListResponse>>,
{
    if !force_refetch && let Some(cached_connectors) = cached_all_connectors(&cache_key) {
        return Ok(cached_connectors);
    }

    let mut apps = list_directory_connectors(&mut fetch_page).await?;
    if is_workspace_account {
        apps.extend(list_workspace_connectors(&mut fetch_page).await?);
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

async fn list_directory_connectors<F, Fut>(fetch_page: &mut F) -> anyhow::Result<Vec<DirectoryApp>>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = anyhow::Result<DirectoryListResponse>>,
{
    let mut apps = Vec::new();
    let mut next_token: Option<String> = None;
    loop {
        let path = match next_token.as_deref() {
            Some(token) => {
                let encoded_token = urlencoding::encode(token);
                format!(
                    "/connectors/directory/list?tier=categorized&token={encoded_token}&external_logos=true"
                )
            }
            None => "/connectors/directory/list?tier=categorized&external_logos=true".to_string(),
        };
        let response = fetch_page(path).await?;
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

async fn list_workspace_connectors<F, Fut>(fetch_page: &mut F) -> anyhow::Result<Vec<DirectoryApp>>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = anyhow::Result<DirectoryListResponse>>,
{
    let response =
        fetch_page("/connectors/directory/list_workspace?external_logos=true".to_string()).await;
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
    if incoming_description_present {
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
        plugin_display_names: Vec::new(),
    }
}

fn connector_install_url(name: &str, connector_id: &str) -> String {
    let slug = connector_name_slug(name);
    format!("https://chatgpt.com/apps/{slug}/{connector_id}")
}

fn connector_name_slug(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.push('-');
        }
    }
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        "app".to_string()
    } else {
        normalized.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    fn cache_key(id: &str) -> AllConnectorsCacheKey {
        AllConnectorsCacheKey::new(
            "https://chatgpt.example".to_string(),
            Some(format!("account-{id}")),
            Some(format!("user-{id}")),
            /*is_workspace_account*/ true,
        )
    }

    fn app(id: &str, name: &str) -> DirectoryApp {
        DirectoryApp {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            app_metadata: None,
            branding: None,
            labels: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            visibility: None,
        }
    }

    #[tokio::test]
    async fn list_all_connectors_uses_shared_cache() -> anyhow::Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let call_counter = Arc::clone(&calls);
        let key = cache_key("shared");

        let first = list_all_connectors_with_options(
            key.clone(),
            /*is_workspace_account*/ false,
            /*force_refetch*/ false,
            move |_path| {
                let call_counter = Arc::clone(&call_counter);
                async move {
                    call_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(DirectoryListResponse {
                        apps: vec![app("alpha", "Alpha")],
                        next_token: None,
                    })
                }
            },
        )
        .await?;

        let second = list_all_connectors_with_options(
            key,
            /*is_workspace_account*/ false,
            /*force_refetch*/ false,
            move |_path| async move {
                anyhow::bail!("cache should have been used");
            },
        )
        .await?;

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first, second);
        Ok(())
    }

    #[tokio::test]
    async fn list_all_connectors_merges_and_normalizes_directory_apps() -> anyhow::Result<()> {
        let key = cache_key("merged");
        let calls = Arc::new(AtomicUsize::new(0));
        let call_counter = Arc::clone(&calls);

        let connectors = list_all_connectors_with_options(
            key,
            /*is_workspace_account*/ true,
            /*force_refetch*/ true,
            move |path| {
                let call_counter = Arc::clone(&call_counter);
                async move {
                    call_counter.fetch_add(1, Ordering::SeqCst);
                    if path.starts_with("/connectors/directory/list_workspace") {
                        Ok(DirectoryListResponse {
                            apps: vec![
                                DirectoryApp {
                                    description: Some("Merged description".to_string()),
                                    branding: Some(AppBranding {
                                        category: Some("calendar".to_string()),
                                        developer: None,
                                        website: None,
                                        privacy_policy: None,
                                        terms_of_service: None,
                                        is_discoverable_app: true,
                                    }),
                                    ..app("alpha", "")
                                },
                                DirectoryApp {
                                    visibility: Some("HIDDEN".to_string()),
                                    ..app("hidden", "Hidden")
                                },
                            ],
                            next_token: None,
                        })
                    } else {
                        Ok(DirectoryListResponse {
                            apps: vec![app("alpha", " Alpha "), app("beta", "Beta")],
                            next_token: None,
                        })
                    }
                }
            },
        )
        .await?;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(connectors.len(), 2);
        assert_eq!(connectors[0].id, "alpha");
        assert_eq!(connectors[0].name, "Alpha");
        assert_eq!(
            connectors[0].description.as_deref(),
            Some("Merged description")
        );
        assert_eq!(
            connectors[0].install_url.as_deref(),
            Some("https://chatgpt.com/apps/alpha/alpha")
        );
        assert_eq!(
            connectors[0]
                .branding
                .as_ref()
                .and_then(|branding| branding.category.as_deref()),
            Some("calendar")
        );
        assert_eq!(connectors[1].id, "beta");
        assert_eq!(connectors[1].name, "Beta");
        Ok(())
    }
}
