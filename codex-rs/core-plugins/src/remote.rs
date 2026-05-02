use crate::store::PLUGINS_CACHE_DIR;
use crate::store::PluginStore;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInterface;
use codex_app_server_protocol::SkillInterface;
use codex_login::CodexAuth;
use codex_login::default_client::build_reqwest_client;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use reqwest::RequestBuilder;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

mod remote_installed_plugin_sync;
mod share;

pub use remote_installed_plugin_sync::RemoteInstalledPluginBundleSyncError;
pub use remote_installed_plugin_sync::RemoteInstalledPluginBundleSyncOutcome;
pub use remote_installed_plugin_sync::RemotePluginCacheMutationGuard;
pub use remote_installed_plugin_sync::mark_remote_plugin_cache_mutation_in_flight;
pub use remote_installed_plugin_sync::maybe_start_remote_installed_plugin_bundle_sync;
pub use remote_installed_plugin_sync::sync_remote_installed_plugin_bundles_once;
pub use share::RemotePluginShareSaveResult;
pub use share::delete_remote_plugin_share;
pub use share::list_remote_plugin_shares;
pub use share::save_remote_plugin_share;

pub const REMOTE_GLOBAL_MARKETPLACE_NAME: &str = "chatgpt-global";
pub const REMOTE_WORKSPACE_MARKETPLACE_NAME: &str = "chatgpt-workspace";
pub const REMOTE_GLOBAL_MARKETPLACE_DISPLAY_NAME: &str = "ChatGPT Plugins";
pub const REMOTE_WORKSPACE_MARKETPLACE_DISPLAY_NAME: &str = "ChatGPT Workspace Plugins";

const REMOTE_PLUGIN_CATALOG_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_PLUGIN_LIST_PAGE_LIMIT: u32 = 200;
const MAX_REMOTE_DEFAULT_PROMPT_LEN: usize = 128;
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePluginServiceConfig {
    pub chatgpt_base_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteMarketplace {
    pub name: String,
    pub display_name: String,
    pub plugins: Vec<RemotePluginSummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteInstalledPlugin {
    pub marketplace_name: String,
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginSummary {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub enabled: bool,
    pub install_policy: PluginInstallPolicy,
    pub auth_policy: PluginAuthPolicy,
    pub availability: PluginAvailability,
    pub interface: Option<PluginInterface>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginShareSummary {
    pub summary: RemotePluginSummary,
    pub share_url: Option<String>,
    pub local_plugin_path: Option<AbsolutePathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginDetail {
    pub marketplace_name: String,
    pub marketplace_display_name: String,
    pub summary: RemotePluginSummary,
    pub description: Option<String>,
    pub release_version: Option<String>,
    pub bundle_download_url: Option<String>,
    pub skills: Vec<RemotePluginSkill>,
    pub app_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginSkill {
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub interface: Option<SkillInterface>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginSkillDetail {
    pub contents: Option<String>,
}

pub fn is_valid_remote_plugin_id(plugin_id: &str) -> bool {
    !plugin_id.is_empty()
        && plugin_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '~')
}

pub fn validate_remote_plugin_id(plugin_id: &str) -> Result<(), JSONRPCErrorError> {
    if !is_valid_remote_plugin_id(plugin_id) {
        return Err(JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message:
                "invalid remote plugin id: only ASCII letters, digits, `_`, `-`, and `~` are allowed"
                    .to_string(),
            data: None,
        });
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginCatalogError {
    #[error("chatgpt authentication required for remote plugin catalog")]
    AuthRequired,

    #[error(
        "chatgpt authentication required for remote plugin catalog; api key auth is not supported"
    )]
    UnsupportedAuthMode,

    #[error("failed to read auth token for remote plugin catalog: {0}")]
    AuthToken(#[source] std::io::Error),

    #[error("failed to send remote plugin catalog request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin catalog request to {url} failed with status {status}: {body}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to parse remote plugin catalog response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid remote plugin catalog base URL: {0}")]
    InvalidBaseUrl(#[source] url::ParseError),

    #[error("invalid remote plugin catalog base URL path")]
    InvalidBaseUrlPath,

    #[error("remote marketplace `{marketplace_name}` is not supported")]
    UnknownMarketplace { marketplace_name: String },

    #[error(
        "remote plugin mutation returned unexpected plugin id: expected `{expected}`, got `{actual}`"
    )]
    UnexpectedPluginId { expected: String, actual: String },

    #[error(
        "remote plugin skill response returned unexpected skill name: expected `{expected}`, got `{actual}`"
    )]
    UnexpectedSkillName { expected: String, actual: String },

    #[error(
        "remote plugin mutation returned unexpected enabled state for `{plugin_id}`: expected {expected_enabled}, got {actual_enabled}"
    )]
    UnexpectedEnabledState {
        plugin_id: String,
        expected_enabled: bool,
        actual_enabled: bool,
    },

    #[error("invalid plugin path `{path}`: {reason}")]
    InvalidPluginPath { path: PathBuf, reason: String },

    #[error("failed to archive plugin at `{path}`: {source}")]
    Archive {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to join plugin archive task: {0}")]
    ArchiveJoin(#[source] tokio::task::JoinError),

    #[error(
        "plugin archive would be {bytes} bytes, exceeding the maximum upload size of {max_bytes} bytes"
    )]
    ArchiveTooLarge { bytes: usize, max_bytes: usize },

    #[error("workspace plugin upload response did not include an etag")]
    MissingUploadEtag,

    #[error("{0}")]
    UnexpectedResponse(String),

    #[error("{0}")]
    CacheRemove(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
enum RemotePluginScope {
    #[serde(rename = "GLOBAL")]
    Global,
    #[serde(rename = "WORKSPACE")]
    Workspace,
}

impl RemotePluginScope {
    fn all() -> [Self; 2] {
        [Self::Global, Self::Workspace]
    }

    fn api_value(self) -> &'static str {
        match self {
            Self::Global => "GLOBAL",
            Self::Workspace => "WORKSPACE",
        }
    }

    fn marketplace_name(self) -> &'static str {
        match self {
            Self::Global => REMOTE_GLOBAL_MARKETPLACE_NAME,
            Self::Workspace => REMOTE_WORKSPACE_MARKETPLACE_NAME,
        }
    }

    fn marketplace_display_name(self) -> &'static str {
        match self {
            Self::Global => REMOTE_GLOBAL_MARKETPLACE_DISPLAY_NAME,
            Self::Workspace => REMOTE_WORKSPACE_MARKETPLACE_DISPLAY_NAME,
        }
    }

    fn from_marketplace_name(name: &str) -> Option<Self> {
        match name {
            REMOTE_GLOBAL_MARKETPLACE_NAME => Some(Self::Global),
            REMOTE_WORKSPACE_MARKETPLACE_NAME => Some(Self::Workspace),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginPagination {
    next_page_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginSkillInterfaceResponse {
    display_name: Option<String>,
    short_description: Option<String>,
    brand_color: Option<String>,
    default_prompt: Option<String>,
    icon_small_url: Option<String>,
    icon_large_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginSkillResponse {
    name: String,
    description: String,
    interface: Option<RemotePluginSkillInterfaceResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginSkillDetailResponse {
    plugin_id: String,
    name: String,
    skill_md_contents: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginReleaseInterfaceResponse {
    short_description: Option<String>,
    long_description: Option<String>,
    developer_name: Option<String>,
    category: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    website_url: Option<String>,
    privacy_policy_url: Option<String>,
    terms_of_service_url: Option<String>,
    brand_color: Option<String>,
    default_prompt: Option<String>,
    composer_icon_url: Option<String>,
    logo_url: Option<String>,
    #[serde(default)]
    screenshot_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginReleaseResponse {
    #[serde(default)]
    version: Option<String>,
    display_name: String,
    description: String,
    #[serde(default)]
    bundle_download_url: Option<String>,
    #[serde(default)]
    app_ids: Vec<String>,
    interface: RemotePluginReleaseInterfaceResponse,
    #[serde(default)]
    skills: Vec<RemotePluginSkillResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginDirectoryItem {
    id: String,
    name: String,
    scope: RemotePluginScope,
    #[serde(default)]
    share_url: Option<String>,
    installation_policy: PluginInstallPolicy,
    authentication_policy: PluginAuthPolicy,
    #[serde(rename = "status", default)]
    availability: PluginAvailability,
    release: RemotePluginReleaseResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginInstalledItem {
    #[serde(flatten)]
    plugin: RemotePluginDirectoryItem,
    enabled: bool,
    #[serde(default)]
    disabled_skill_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginListResponse {
    plugins: Vec<RemotePluginDirectoryItem>,
    pagination: RemotePluginPagination,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginInstalledResponse {
    plugins: Vec<RemotePluginInstalledItem>,
    pagination: RemotePluginPagination,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginMutationResponse {
    id: String,
    enabled: bool,
}

pub async fn fetch_remote_marketplaces(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<Vec<RemoteMarketplace>, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let mut directory_by_scope =
        BTreeMap::<RemotePluginScope, BTreeMap<String, RemotePluginDirectoryItem>>::new();
    let mut installed_by_scope =
        BTreeMap::<RemotePluginScope, BTreeMap<String, RemotePluginInstalledItem>>::new();

    let global = async {
        let scope = RemotePluginScope::Global;
        let (directory_plugins, installed_plugins) = tokio::try_join!(
            fetch_directory_plugins_for_scope(config, auth, scope),
            fetch_installed_plugins_for_scope(config, auth, scope),
        )?;
        Ok::<_, RemotePluginCatalogError>((scope, directory_plugins, installed_plugins))
    };
    let workspace = async {
        let scope = RemotePluginScope::Workspace;
        let (directory_plugins, installed_plugins) = tokio::try_join!(
            fetch_directory_plugins_for_scope(config, auth, scope),
            fetch_installed_plugins_for_scope(config, auth, scope),
        )?;
        Ok::<_, RemotePluginCatalogError>((scope, directory_plugins, installed_plugins))
    };

    let (global, workspace) = tokio::try_join!(global, workspace)?;
    for (scope, directory_plugins, installed_plugins) in [global, workspace] {
        if !directory_plugins.is_empty() {
            directory_by_scope.insert(
                scope,
                directory_plugins
                    .into_iter()
                    .map(|plugin| (plugin.id.clone(), plugin))
                    .collect(),
            );
        }
        if !installed_plugins.is_empty() {
            installed_by_scope.insert(
                scope,
                installed_plugins
                    .into_iter()
                    .map(|plugin| (plugin.plugin.id.clone(), plugin))
                    .collect(),
            );
        }
    }

    let mut marketplaces = Vec::new();
    for scope in RemotePluginScope::all() {
        let directory_plugins = directory_by_scope.get(&scope);
        let installed_plugins = installed_by_scope.get(&scope);
        let plugin_ids = directory_plugins
            .into_iter()
            .flat_map(|plugins| plugins.keys())
            .chain(
                installed_plugins
                    .into_iter()
                    .flat_map(|plugins| plugins.keys()),
            )
            .cloned()
            .collect::<BTreeSet<_>>();
        if plugin_ids.is_empty() {
            continue;
        }

        let mut plugins = plugin_ids
            .into_iter()
            .filter_map(|plugin_id| {
                let directory_plugin =
                    directory_plugins.and_then(|plugins| plugins.get(&plugin_id));
                let installed_plugin =
                    installed_plugins.and_then(|plugins| plugins.get(&plugin_id));
                directory_plugin
                    .or_else(|| installed_plugin.map(|plugin| &plugin.plugin))
                    .map(|plugin| build_remote_plugin_summary(plugin, installed_plugin))
            })
            .collect::<Vec<_>>();
        plugins.sort_by(|left, right| {
            remote_plugin_display_name(left)
                .to_ascii_lowercase()
                .cmp(&remote_plugin_display_name(right).to_ascii_lowercase())
                .then_with(|| {
                    remote_plugin_display_name(left).cmp(remote_plugin_display_name(right))
                })
                .then_with(|| left.id.cmp(&right.id))
        });
        marketplaces.push(RemoteMarketplace {
            name: scope.marketplace_name().to_string(),
            display_name: scope.marketplace_display_name().to_string(),
            plugins,
        });
    }

    Ok(marketplaces)
}

pub async fn fetch_remote_installed_plugins(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<Vec<RemoteInstalledPlugin>, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let global = async {
        let scope = RemotePluginScope::Global;
        let installed_plugins = fetch_installed_plugins_for_scope(config, auth, scope).await?;
        Ok::<_, RemotePluginCatalogError>((scope, installed_plugins))
    };
    let workspace = async {
        let scope = RemotePluginScope::Workspace;
        let installed_plugins = fetch_installed_plugins_for_scope(config, auth, scope).await?;
        Ok::<_, RemotePluginCatalogError>((scope, installed_plugins))
    };

    let (global, workspace) = tokio::try_join!(global, workspace)?;
    let mut installed_plugins = [global, workspace]
        .into_iter()
        .flat_map(|(scope, plugins)| {
            plugins
                .into_iter()
                .map(move |plugin| remote_installed_plugin_to_info(scope, &plugin))
        })
        .collect::<Vec<_>>();
    installed_plugins.sort_by(|left, right| {
        left.marketplace_name
            .cmp(&right.marketplace_name)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(installed_plugins)
}

pub async fn fetch_remote_plugin_detail(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    marketplace_name: &str,
    plugin_id: &str,
) -> Result<RemotePluginDetail, RemotePluginCatalogError> {
    fetch_remote_plugin_detail_with_download_url_option(
        config,
        auth,
        marketplace_name,
        plugin_id,
        /*include_download_urls*/ false,
    )
    .await
}

pub async fn fetch_remote_plugin_detail_with_download_urls(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    marketplace_name: &str,
    plugin_id: &str,
) -> Result<RemotePluginDetail, RemotePluginCatalogError> {
    fetch_remote_plugin_detail_with_download_url_option(
        config,
        auth,
        marketplace_name,
        plugin_id,
        /*include_download_urls*/ true,
    )
    .await
}

pub async fn fetch_remote_plugin_skill_detail(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    marketplace_name: &str,
    plugin_id: &str,
    skill_name: &str,
) -> Result<RemotePluginSkillDetail, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    if RemotePluginScope::from_marketplace_name(marketplace_name).is_none() {
        return Err(RemotePluginCatalogError::UnknownMarketplace {
            marketplace_name: marketplace_name.to_string(),
        });
    }

    let url = remote_plugin_skill_detail_url(config, plugin_id, skill_name)?;
    let client = build_reqwest_client();
    let request = authenticated_request(client.get(&url), auth)?;
    let response: RemotePluginSkillDetailResponse = send_and_decode(request, &url).await?;
    if response.plugin_id != plugin_id {
        return Err(RemotePluginCatalogError::UnexpectedPluginId {
            expected: plugin_id.to_string(),
            actual: response.plugin_id,
        });
    }
    if response.name != skill_name {
        return Err(RemotePluginCatalogError::UnexpectedSkillName {
            expected: skill_name.to_string(),
            actual: response.name,
        });
    }

    Ok(RemotePluginSkillDetail {
        contents: response.skill_md_contents,
    })
}

async fn fetch_remote_plugin_detail_with_download_url_option(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    _marketplace_name: &str,
    plugin_id: &str,
    include_download_urls: bool,
) -> Result<RemotePluginDetail, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let plugin = fetch_plugin_detail(config, auth, plugin_id, include_download_urls).await?;
    let scope = plugin.scope;
    let marketplace_name = scope.marketplace_name().to_string();
    // Remote plugin IDs uniquely identify remote plugins, so the caller-provided
    // marketplace name is not validated here. The backend detail response is the
    // source of truth for the plugin's actual scope/marketplace.

    build_remote_plugin_detail(config, auth, scope, marketplace_name, plugin_id, plugin).await
}

async fn build_remote_plugin_detail(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    marketplace_name: String,
    plugin_id: &str,
    plugin: RemotePluginDirectoryItem,
) -> Result<RemotePluginDetail, RemotePluginCatalogError> {
    let installed_plugin = fetch_installed_plugins_for_scope(config, auth, scope)
        .await?
        .into_iter()
        .find(|installed_plugin| installed_plugin.plugin.id == plugin_id);
    let disabled_skill_names = installed_plugin
        .as_ref()
        .map(|plugin| {
            plugin
                .disabled_skill_names
                .iter()
                .cloned()
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let skills = plugin
        .release
        .skills
        .iter()
        .map(|skill| RemotePluginSkill {
            name: skill.name.clone(),
            description: skill.description.clone(),
            short_description: skill
                .interface
                .as_ref()
                .and_then(|interface| interface.short_description.clone()),
            interface: remote_skill_interface_to_info(skill.interface.clone()),
            enabled: !disabled_skill_names.contains(&skill.name),
        })
        .collect();

    Ok(RemotePluginDetail {
        marketplace_name,
        marketplace_display_name: scope.marketplace_display_name().to_string(),
        summary: build_remote_plugin_summary(&plugin, installed_plugin.as_ref()),
        description: non_empty_string(Some(&plugin.release.description)),
        release_version: plugin.release.version,
        bundle_download_url: plugin.release.bundle_download_url,
        skills,
        app_ids: plugin.release.app_ids,
    })
}

pub async fn install_remote_plugin(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    _marketplace_name: &str,
    plugin_id: &str,
) -> Result<(), RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    // Remote plugin IDs uniquely identify remote plugins, so the caller-provided
    // marketplace name is not validated before sending the install mutation.

    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/{plugin_id}/install");
    let client = build_reqwest_client();
    let request = authenticated_request(client.post(&url), auth)?;
    let response: RemotePluginMutationResponse = send_and_decode(request, &url).await?;
    if response.id != plugin_id {
        return Err(RemotePluginCatalogError::UnexpectedPluginId {
            expected: plugin_id.to_string(),
            actual: response.id,
        });
    }
    if !response.enabled {
        return Err(RemotePluginCatalogError::UnexpectedEnabledState {
            plugin_id: plugin_id.to_string(),
            expected_enabled: true,
            actual_enabled: response.enabled,
        });
    }

    Ok(())
}

pub async fn uninstall_remote_plugin(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    codex_home: PathBuf,
    plugin_id: &str,
) -> Result<(), RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let plugin = fetch_plugin_detail(
        config, auth, plugin_id, /*include_download_urls*/ false,
    )
    .await?;
    let marketplace_name = plugin.scope.marketplace_name().to_string();
    let plugin_name = plugin.name;

    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/plugins/{plugin_id}/uninstall");
    let client = build_reqwest_client();
    let request = authenticated_request(client.post(&url), auth)?;
    let response: RemotePluginMutationResponse = send_and_decode(request, &url).await?;
    if response.id != plugin_id {
        return Err(RemotePluginCatalogError::UnexpectedPluginId {
            expected: plugin_id.to_string(),
            actual: response.id,
        });
    }
    if response.enabled {
        return Err(RemotePluginCatalogError::UnexpectedEnabledState {
            plugin_id: plugin_id.to_string(),
            expected_enabled: false,
            actual_enabled: response.enabled,
        });
    }

    let legacy_plugin_id = plugin_id.to_string();
    tokio::task::spawn_blocking(move || {
        remove_remote_plugin_cache(codex_home, marketplace_name, plugin_name, legacy_plugin_id)
    })
    .await
    .map_err(|err| {
        RemotePluginCatalogError::CacheRemove(format!(
            "failed to join remote plugin cache removal task: {err}"
        ))
    })?
    .map_err(RemotePluginCatalogError::CacheRemove)?;

    Ok(())
}

fn remove_remote_plugin_cache(
    codex_home: PathBuf,
    marketplace_name: String,
    plugin_name: String,
    legacy_plugin_id: String,
) -> Result<(), String> {
    let store = PluginStore::try_new(codex_home.clone())
        .map_err(|err| format!("failed to resolve remote plugin cache root: {err}"))?;
    let plugin_id =
        PluginId::new(plugin_name.clone(), marketplace_name.clone()).map_err(|err| {
            format!(
                "invalid remote plugin cache id for `{plugin_name}` in `{marketplace_name}`: {err}"
            )
        })?;
    let plugin_cache_root = store.plugin_base_root(&plugin_id);
    store.uninstall(&plugin_id).map_err(|err| {
        format!(
            "failed to remove remote plugin cache entry {}: {err}",
            plugin_cache_root.display()
        )
    })?;

    let legacy_remote_plugin_cache_root = codex_home
        .join(PLUGINS_CACHE_DIR)
        .join(marketplace_name)
        .join(legacy_plugin_id);
    if legacy_remote_plugin_cache_root != plugin_cache_root.as_path()
        && legacy_remote_plugin_cache_root.exists()
    {
        let result = if legacy_remote_plugin_cache_root.is_dir() {
            fs::remove_dir_all(&legacy_remote_plugin_cache_root)
        } else {
            fs::remove_file(&legacy_remote_plugin_cache_root)
        };
        result.map_err(|err| {
            format!(
                "failed to remove remote plugin cache entry {}: {err}",
                legacy_remote_plugin_cache_root.display()
            )
        })?;
    }
    Ok(())
}

fn build_remote_plugin_summary(
    plugin: &RemotePluginDirectoryItem,
    installed_plugin: Option<&RemotePluginInstalledItem>,
) -> RemotePluginSummary {
    RemotePluginSummary {
        id: plugin.id.clone(),
        name: plugin.name.clone(),
        installed: installed_plugin.is_some(),
        enabled: installed_plugin.is_some_and(|plugin| plugin.enabled),
        install_policy: plugin.installation_policy,
        auth_policy: plugin.authentication_policy,
        availability: plugin.availability,
        interface: remote_plugin_interface_to_info(plugin),
    }
}

fn remote_installed_plugin_to_info(
    scope: RemotePluginScope,
    installed_plugin: &RemotePluginInstalledItem,
) -> RemoteInstalledPlugin {
    let plugin = &installed_plugin.plugin;
    // Remote per-skill disabled state (`disabled_skill_names`) is intentionally
    // not projected into skills/list yet; local skills.config remains the
    // supported source for skill enablement.
    RemoteInstalledPlugin {
        marketplace_name: scope.marketplace_name().to_string(),
        id: plugin.id.clone(),
        name: plugin.name.clone(),
        enabled: installed_plugin.enabled,
    }
}

fn remote_plugin_interface_to_info(plugin: &RemotePluginDirectoryItem) -> Option<PluginInterface> {
    let interface = &plugin.release.interface;
    let display_name = non_empty_string(Some(&plugin.release.display_name));
    let default_prompt = interface
        .default_prompt
        .as_ref()
        .and_then(|prompt| normalize_remote_default_prompt(prompt));
    let result = PluginInterface {
        display_name,
        short_description: interface.short_description.clone(),
        long_description: interface.long_description.clone(),
        developer_name: interface.developer_name.clone(),
        category: interface.category.clone(),
        capabilities: interface.capabilities.clone(),
        website_url: interface.website_url.clone(),
        privacy_policy_url: interface.privacy_policy_url.clone(),
        terms_of_service_url: interface.terms_of_service_url.clone(),
        default_prompt,
        brand_color: interface.brand_color.clone(),
        composer_icon: None,
        composer_icon_url: interface.composer_icon_url.clone(),
        logo: None,
        logo_url: interface.logo_url.clone(),
        screenshots: Vec::new(),
        screenshot_urls: interface.screenshot_urls.clone(),
    };
    let has_fields = result.display_name.is_some()
        || result.short_description.is_some()
        || result.long_description.is_some()
        || result.developer_name.is_some()
        || result.category.is_some()
        || !result.capabilities.is_empty()
        || result.website_url.is_some()
        || result.privacy_policy_url.is_some()
        || result.terms_of_service_url.is_some()
        || result.default_prompt.is_some()
        || result.brand_color.is_some()
        || result.composer_icon_url.is_some()
        || result.logo_url.is_some()
        || !result.screenshot_urls.is_empty();
    has_fields.then_some(result)
}

fn remote_skill_interface_to_info(
    interface: Option<RemotePluginSkillInterfaceResponse>,
) -> Option<SkillInterface> {
    interface.and_then(|interface| {
        let result = SkillInterface {
            display_name: interface.display_name,
            short_description: interface.short_description,
            icon_small: None,
            icon_large: None,
            brand_color: interface.brand_color,
            default_prompt: interface.default_prompt,
        };
        let has_fields = result.display_name.is_some()
            || result.short_description.is_some()
            || result.brand_color.is_some()
            || result.default_prompt.is_some();
        has_fields.then_some(result)
    })
}

fn remote_plugin_display_name(plugin: &RemotePluginSummary) -> &str {
    plugin
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
        .unwrap_or(&plugin.name)
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn normalize_remote_default_prompt(prompt: &str) -> Option<Vec<String>> {
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.chars().count() > MAX_REMOTE_DEFAULT_PROMPT_LEN {
        return None;
    }
    Some(vec![prompt.to_string()])
}

async fn fetch_directory_plugins_for_scope(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
) -> Result<Vec<RemotePluginDirectoryItem>, RemotePluginCatalogError> {
    let mut plugins = Vec::new();
    let mut page_token = None;
    loop {
        let response =
            get_remote_plugin_list_page(config, auth, scope, page_token.as_deref()).await?;
        plugins.extend(response.plugins);
        let Some(next_page_token) = response.pagination.next_page_token else {
            break;
        };
        page_token = Some(next_page_token);
    }
    Ok(plugins)
}

async fn fetch_installed_plugins_for_scope(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
) -> Result<Vec<RemotePluginInstalledItem>, RemotePluginCatalogError> {
    fetch_installed_plugins_for_scope_with_download_url(
        config, auth, scope, /*include_download_urls*/ false,
    )
    .await
}

async fn fetch_installed_plugins_for_scope_with_download_url(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    include_download_urls: bool,
) -> Result<Vec<RemotePluginInstalledItem>, RemotePluginCatalogError> {
    let mut plugins = Vec::new();
    let mut page_token = None;
    loop {
        let response = get_remote_plugin_installed_page(
            config,
            auth,
            scope,
            page_token.as_deref(),
            include_download_urls,
        )
        .await?;
        plugins.extend(response.plugins);
        let Some(next_page_token) = response.pagination.next_page_token else {
            break;
        };
        page_token = Some(next_page_token);
    }
    Ok(plugins)
}

async fn get_remote_plugin_list_page(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    page_token: Option<&str>,
) -> Result<RemotePluginListResponse, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/list");
    let client = build_reqwest_client();
    let mut request = authenticated_request(client.get(&url), auth)?;
    request = request.query(&[("scope", scope.api_value())]);
    request = request.query(&[("limit", REMOTE_PLUGIN_LIST_PAGE_LIMIT)]);
    if let Some(page_token) = page_token {
        request = request.query(&[("pageToken", page_token)]);
    }
    send_and_decode(request, &url).await
}

async fn get_remote_plugin_installed_page(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    page_token: Option<&str>,
    include_download_urls: bool,
) -> Result<RemotePluginInstalledResponse, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/installed");
    let client = build_reqwest_client();
    let mut request = authenticated_request(client.get(&url), auth)?;
    request = request.query(&[("scope", scope.api_value())]);
    if include_download_urls {
        request = request.query(&[("includeDownloadUrls", true)]);
    }
    if let Some(page_token) = page_token {
        request = request.query(&[("pageToken", page_token)]);
    }
    send_and_decode(request, &url).await
}

async fn fetch_plugin_detail(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    plugin_id: &str,
    include_download_urls: bool,
) -> Result<RemotePluginDirectoryItem, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/{plugin_id}");
    let client = build_reqwest_client();
    let mut request = authenticated_request(client.get(&url), auth)?;
    if include_download_urls {
        request = request.query(&[("includeDownloadUrls", true)]);
    }
    send_and_decode(request, &url).await
}

fn remote_plugin_skill_detail_url(
    config: &RemotePluginServiceConfig,
    plugin_id: &str,
    skill_name: &str,
) -> Result<String, RemotePluginCatalogError> {
    let mut url = Url::parse(config.chatgpt_base_url.trim_end_matches('/'))
        .map_err(RemotePluginCatalogError::InvalidBaseUrl)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| RemotePluginCatalogError::InvalidBaseUrlPath)?;
        segments.pop_if_empty();
        segments.push("ps");
        segments.push("plugins");
        segments.push(plugin_id);
        segments.push("skills");
        segments.push(skill_name);
    }
    Ok(url.to_string())
}

fn ensure_chatgpt_auth(auth: Option<&CodexAuth>) -> Result<&CodexAuth, RemotePluginCatalogError> {
    let Some(auth) = auth else {
        return Err(RemotePluginCatalogError::AuthRequired);
    };
    if !auth.uses_codex_backend() {
        return Err(RemotePluginCatalogError::UnsupportedAuthMode);
    }
    Ok(auth)
}

fn authenticated_request(
    request: RequestBuilder,
    auth: &CodexAuth,
) -> Result<RequestBuilder, RemotePluginCatalogError> {
    Ok(request
        .timeout(REMOTE_PLUGIN_CATALOG_TIMEOUT)
        .headers(codex_model_provider::auth_provider_from_auth(auth).to_auth_headers()))
}

async fn send_and_decode<T: for<'de> Deserialize<'de>>(
    request: RequestBuilder,
    url: &str,
) -> Result<T, RemotePluginCatalogError> {
    let response = request
        .send()
        .await
        .map_err(|source| RemotePluginCatalogError::Request {
            url: url.to_string(),
            source,
        })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(RemotePluginCatalogError::UnexpectedStatus {
            url: url.to_string(),
            status,
            body,
        });
    }

    serde_json::from_str(&body).map_err(|source| RemotePluginCatalogError::Decode {
        url: url.to_string(),
        source,
    })
}
