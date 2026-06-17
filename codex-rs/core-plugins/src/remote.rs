use crate::app_mcp_routing::apply_app_mcp_routing_policy;
use crate::loader::plugin_app_declarations_from_value;
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
use codex_plugin::AppConnectorId;
use codex_plugin::AppDeclaration;
use codex_plugin::PluginId;
use codex_plugin::app_connector_ids_from_declarations;
use codex_utils_absolute_path::AbsolutePathBuf;
use reqwest::RequestBuilder;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

mod catalog_cache;
mod remote_installed_plugin_sync;
mod share;

pub(crate) use catalog_cache::RemotePluginCatalogCacheKey;

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;

pub use remote_installed_plugin_sync::RemoteInstalledPluginBundleSyncError;
pub use remote_installed_plugin_sync::RemoteInstalledPluginBundleSyncOutcome;
pub use remote_installed_plugin_sync::RemotePluginCacheMutationGuard;
pub use remote_installed_plugin_sync::mark_remote_plugin_cache_mutation_in_flight;
pub(crate) use remote_installed_plugin_sync::maybe_start_remote_installed_plugin_bundle_sync;
pub use remote_installed_plugin_sync::sync_remote_installed_plugin_bundles_once;
pub use share::RemotePluginShareAccessPolicy;
pub use share::RemotePluginShareDiscoverability;
pub use share::RemotePluginSharePrincipal;
pub use share::RemotePluginSharePrincipalRole;
pub use share::RemotePluginSharePrincipalType;
pub use share::RemotePluginShareSaveResult;
pub use share::RemotePluginShareTarget;
pub use share::RemotePluginShareTargetRole;
pub use share::RemotePluginShareUpdateDiscoverability;
pub use share::RemotePluginShareUpdateTargetsResult;
pub use share::checkout_remote_plugin_share;
pub use share::delete_remote_plugin_share;
pub use share::list_remote_plugin_shares;
pub use share::load_plugin_share_remote_ids_by_local_path;
pub use share::save_remote_plugin_share;
pub use share::update_remote_plugin_share_targets;

pub const REMOTE_GLOBAL_MARKETPLACE_NAME: &str = "openai-curated-remote";
pub const REMOTE_CREATED_BY_ME_MARKETPLACE_NAME: &str = "created-by-me-remote";
pub const REMOTE_WORKSPACE_MARKETPLACE_NAME: &str = "workspace-directory";
pub const REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME: &str = "workspace-shared-with-me";
pub const REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME: &str =
    "workspace-shared-with-me-private";
pub const REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME: &str =
    "workspace-shared-with-me-unlisted";
pub const REMOTE_GLOBAL_MARKETPLACE_DISPLAY_NAME: &str = "OpenAI Curated Remote";
pub const REMOTE_CREATED_BY_ME_MARKETPLACE_DISPLAY_NAME: &str = "Created by me";
pub const REMOTE_WORKSPACE_MARKETPLACE_DISPLAY_NAME: &str = "Workspace Directory";
pub const REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_DISPLAY_NAME: &str = "Shared with me";
pub const REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_DISPLAY_NAME: &str =
    "Shared with me (unlisted)";

const OPENAI_CURATED_REMOTE_COLLECTION_KEY: &str = "vertical";
const OAI_PRODUCT_SKU_HEADER: &str = "OAI-Product-Sku";
const CODEX_PRODUCT_SKU: &str = "codex";
const REMOTE_PLUGIN_CATALOG_TIMEOUT: Duration = Duration::from_secs(30);
const RECOMMENDED_PLUGINS_TIMEOUT: Duration = Duration::from_secs(5);
const REMOTE_PLUGIN_LIST_PAGE_LIMIT: u32 = 200;
const MAX_RECOMMENDED_PLUGINS: usize = 50;
const MAX_RECOMMENDED_PLUGIN_NAME_LEN: usize = 64;
const MAX_RECOMMENDED_PLUGIN_DISPLAY_NAME_LEN: usize = 64;
const MAX_REMOTE_DEFAULT_PROMPT_COUNT: usize = 3;
const MAX_REMOTE_DEFAULT_PROMPT_LEN: usize = 128;
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;
const REMOTE_INSTALLED_MARKETPLACE_DISPLAY_ORDER: [(&str, &str); 6] = [
    (
        REMOTE_GLOBAL_MARKETPLACE_NAME,
        REMOTE_GLOBAL_MARKETPLACE_DISPLAY_NAME,
    ),
    (
        REMOTE_CREATED_BY_ME_MARKETPLACE_NAME,
        REMOTE_CREATED_BY_ME_MARKETPLACE_DISPLAY_NAME,
    ),
    (
        REMOTE_WORKSPACE_MARKETPLACE_NAME,
        REMOTE_WORKSPACE_MARKETPLACE_DISPLAY_NAME,
    ),
    (
        REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME,
        REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_DISPLAY_NAME,
    ),
    (
        REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME,
        REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_DISPLAY_NAME,
    ),
    (
        REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME,
        REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_DISPLAY_NAME,
    ),
];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteMarketplaceSource {
    Global,
    CreatedByMeRemote,
    WorkspaceDirectory,
    SharedWithMe,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteInstalledPlugin {
    pub marketplace_name: String,
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub install_policy: PluginInstallPolicy,
    pub auth_policy: PluginAuthPolicy,
    pub availability: PluginAvailability,
    pub interface: Option<PluginInterface>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginSummary {
    pub id: String,
    pub remote_plugin_id: String,
    pub name: String,
    pub share_context: Option<RemotePluginShareContext>,
    pub installed: bool,
    pub enabled: bool,
    pub install_policy: PluginInstallPolicy,
    pub auth_policy: PluginAuthPolicy,
    pub availability: PluginAvailability,
    pub interface: Option<PluginInterface>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePluginShareContext {
    pub remote_plugin_id: String,
    pub remote_version: Option<String>,
    pub discoverability: RemotePluginShareDiscoverability,
    pub share_url: Option<String>,
    pub creator_account_user_id: Option<String>,
    pub creator_name: Option<String>,
    pub share_principals: Option<Vec<RemotePluginSharePrincipal>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginShareSummary {
    pub summary: RemotePluginSummary,
    pub local_plugin_path: Option<AbsolutePathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginDetail {
    pub marketplace_name: String,
    pub marketplace_display_name: String,
    pub summary: RemotePluginSummary,
    pub share_url: Option<String>,
    pub description: Option<String>,
    pub release_version: Option<String>,
    pub bundle_download_url: Option<String>,
    pub app_manifest: Option<JsonValue>,
    pub skills: Vec<RemotePluginSkill>,
    pub app_ids: Vec<String>,
    pub app_templates: Vec<RemoteAppTemplate>,
    pub mcp_servers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAppTemplate {
    pub template_id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub canonical_connector_id: Option<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    pub materialized_app_ids: Vec<String>,
    pub reason: Option<RemoteAppTemplateUnavailableReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RemoteAppTemplateUnavailableReason {
    NotConfiguredForWorkspace,
    NoActiveWorkspace,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDiscoverablePlugin {
    pub config_id: String,
    pub remote_plugin_id: String,
    pub name: String,
    pub description: Option<String>,
    pub has_skills: bool,
    pub app_ids: Vec<String>,
    pub install_policy: PluginInstallPolicy,
    pub availability: PluginAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendedPlugin {
    pub config_id: String,
    pub remote_plugin_id: String,
    pub display_name: String,
    pub app_connector_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecommendedPluginsMode {
    Legacy,
    Endpoint { plugins: Vec<RecommendedPlugin> },
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

    #[error("remote plugin `{remote_plugin_id}` is not available for plugin/share/checkout")]
    PluginShareCheckoutNotAvailable { remote_plugin_id: String },

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum RemotePluginScope {
    #[serde(rename = "GLOBAL")]
    Global,
    #[serde(rename = "USER")]
    User,
    #[serde(rename = "WORKSPACE")]
    Workspace,
}

impl RemotePluginScope {
    fn api_value(self) -> &'static str {
        match self {
            Self::Global => "GLOBAL",
            Self::User => "USER",
            Self::Workspace => "WORKSPACE",
        }
    }

    fn marketplace_name(self) -> &'static str {
        match self {
            Self::Global => REMOTE_GLOBAL_MARKETPLACE_NAME,
            Self::User => REMOTE_CREATED_BY_ME_MARKETPLACE_NAME,
            Self::Workspace => REMOTE_WORKSPACE_MARKETPLACE_NAME,
        }
    }

    fn marketplace_display_name(self) -> &'static str {
        match self {
            Self::Global => REMOTE_GLOBAL_MARKETPLACE_DISPLAY_NAME,
            Self::User => REMOTE_CREATED_BY_ME_MARKETPLACE_DISPLAY_NAME,
            Self::Workspace => REMOTE_WORKSPACE_MARKETPLACE_DISPLAY_NAME,
        }
    }

    fn from_marketplace_name(name: &str) -> Option<Self> {
        match name {
            REMOTE_GLOBAL_MARKETPLACE_NAME => Some(Self::Global),
            REMOTE_CREATED_BY_ME_MARKETPLACE_NAME => Some(Self::User),
            REMOTE_WORKSPACE_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME => Some(Self::Workspace),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginPagination {
    next_page_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct RemotePluginSkillInterfaceResponse {
    display_name: Option<String>,
    short_description: Option<String>,
    brand_color: Option<String>,
    default_prompt: Option<String>,
    icon_small_url: Option<String>,
    icon_large_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
    default_prompts: Option<Vec<String>>,
    composer_icon_url: Option<String>,
    logo_url: Option<String>,
    #[serde(default)]
    screenshot_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct RemotePluginReleaseResponse {
    #[serde(default)]
    version: Option<String>,
    display_name: String,
    description: String,
    #[serde(default)]
    bundle_download_url: Option<String>,
    #[serde(default)]
    app_ids: Vec<String>,
    #[serde(default)]
    app_manifest: Option<JsonValue>,
    #[serde(default, alias = "unavailable_app_templates")]
    app_templates: Vec<RemoteAppTemplateResponse>,
    #[serde(default)]
    keywords: Vec<String>,
    interface: RemotePluginReleaseInterfaceResponse,
    #[serde(default)]
    skills: Vec<RemotePluginSkillResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mcp_servers: Vec<RemotePluginMcpServerResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct RemotePluginMcpServerResponse {
    key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct RemoteAppTemplateResponse {
    template_id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    canonical_connector_id: Option<String>,
    #[serde(default)]
    logo_url: Option<String>,
    #[serde(default)]
    logo_url_dark: Option<String>,
    #[serde(default)]
    materialized_app_ids: Vec<String>,
    #[serde(default)]
    reason: Option<RemoteAppTemplateUnavailableReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct RemotePluginDirectoryItem {
    id: String,
    name: String,
    scope: RemotePluginScope,
    #[serde(default)]
    discoverability: Option<RemotePluginShareDiscoverability>,
    #[serde(default)]
    creator_account_user_id: Option<String>,
    #[serde(default)]
    creator_name: Option<String>,
    #[serde(default)]
    share_url: Option<String>,
    #[serde(default)]
    share_principals: Option<Vec<RemotePluginDirectorySharePrincipal>>,
    installation_policy: PluginInstallPolicy,
    authentication_policy: PluginAuthPolicy,
    #[serde(rename = "status", default)]
    availability: PluginAvailability,
    release: RemotePluginReleaseResponse,
}

fn remote_plugin_canonical_marketplace_name(
    plugin: &RemotePluginDirectoryItem,
) -> Result<&'static str, RemotePluginCatalogError> {
    match plugin.scope {
        RemotePluginScope::Global => Ok(REMOTE_GLOBAL_MARKETPLACE_NAME),
        RemotePluginScope::User => Ok(REMOTE_CREATED_BY_ME_MARKETPLACE_NAME),
        RemotePluginScope::Workspace => match workspace_plugin_discoverability(plugin)? {
            RemotePluginShareDiscoverability::Listed => Ok(REMOTE_WORKSPACE_MARKETPLACE_NAME),
            RemotePluginShareDiscoverability::Private
            | RemotePluginShareDiscoverability::Unlisted => {
                Ok(REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME)
            }
        },
    }
}

fn workspace_plugin_discoverability(
    plugin: &RemotePluginDirectoryItem,
) -> Result<RemotePluginShareDiscoverability, RemotePluginCatalogError> {
    plugin.discoverability.ok_or_else(|| {
        RemotePluginCatalogError::UnexpectedResponse(format!(
            "workspace plugin `{}` did not include discoverability",
            plugin.id
        ))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct RemotePluginDirectorySharePrincipal {
    principal_type: RemotePluginSharePrincipalType,
    principal_id: String,
    role: RemotePluginSharePrincipalRole,
    name: String,
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
struct RecommendedPluginsResponse {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    plugins: Vec<RecommendedPluginItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RecommendedPluginItem {
    id: String,
    name: String,
    #[serde(default)]
    status: Option<PluginAvailability>,
    #[serde(default)]
    installation_policy: Option<PluginInstallPolicy>,
    release: RecommendedPluginRelease,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RecommendedPluginRelease {
    display_name: String,
    #[serde(default)]
    app_ids: Vec<String>,
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
    app_ids_needing_auth: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePluginInstallResult {
    pub app_ids_needing_auth: Option<Vec<String>>,
}

pub async fn fetch_remote_marketplaces(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    sources: &[RemoteMarketplaceSource],
    global_catalog_cache_path: Option<&Path>,
) -> Result<Vec<RemoteMarketplace>, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let mut marketplaces = Vec::new();
    let needs_workspace_installed = sources.iter().any(|source| {
        matches!(
            source,
            RemoteMarketplaceSource::WorkspaceDirectory | RemoteMarketplaceSource::SharedWithMe
        )
    });
    let workspace_installed_plugins = if needs_workspace_installed {
        Some(fetch_installed_plugins_for_scope(config, auth, RemotePluginScope::Workspace).await?)
    } else {
        None
    };

    for source in sources {
        match source {
            RemoteMarketplaceSource::Global => {
                let scope = RemotePluginScope::Global;
                if let Some(codex_home) = global_catalog_cache_path
                    && let Some(directory_plugins) =
                        catalog_cache::load_cached_global_directory_plugins(
                            codex_home, config, auth,
                        )
                {
                    let installed_plugins =
                        fetch_installed_plugins_for_scope(config, auth, scope).await?;
                    if let Some(marketplace) = build_remote_marketplace(
                        scope.marketplace_name(),
                        scope.marketplace_display_name(),
                        directory_plugins,
                        installed_plugins,
                        /*include_installed_only*/ true,
                    )? {
                        marketplaces.push(marketplace);
                    }
                    continue;
                }
                let (directory_plugins, installed_plugins) = tokio::try_join!(
                    fetch_directory_plugins_for_scope(config, auth, scope),
                    fetch_installed_plugins_for_scope(config, auth, scope),
                )?;
                let directory_plugins_for_cache =
                    global_catalog_cache_path.map(|_| directory_plugins.clone());
                if let Some(marketplace) = build_remote_marketplace(
                    scope.marketplace_name(),
                    scope.marketplace_display_name(),
                    directory_plugins,
                    installed_plugins,
                    /*include_installed_only*/ true,
                )? {
                    marketplaces.push(marketplace);
                }
                if let (Some(codex_home), Some(directory_plugins)) =
                    (global_catalog_cache_path, directory_plugins_for_cache)
                {
                    catalog_cache::write_cached_global_directory_plugins(
                        codex_home,
                        config,
                        auth,
                        &directory_plugins,
                    );
                }
            }
            RemoteMarketplaceSource::CreatedByMeRemote => {
                let scope = RemotePluginScope::User;
                let (directory_plugins, installed_plugins) = tokio::try_join!(
                    fetch_directory_plugins_for_scope(config, auth, scope),
                    fetch_installed_plugins_for_scope(config, auth, scope),
                )?;
                if let Some(marketplace) = build_remote_marketplace(
                    scope.marketplace_name(),
                    scope.marketplace_display_name(),
                    directory_plugins,
                    installed_plugins,
                    /*include_installed_only*/ false,
                )? {
                    marketplaces.push(marketplace);
                }
            }
            RemoteMarketplaceSource::WorkspaceDirectory => {
                let scope = RemotePluginScope::Workspace;
                let directory_plugins =
                    fetch_directory_plugins_for_scope(config, auth, scope).await?;
                if let Some(marketplace) = build_remote_marketplace(
                    scope.marketplace_name(),
                    scope.marketplace_display_name(),
                    directory_plugins,
                    workspace_installed_plugins.clone().unwrap_or_default(),
                    /*include_installed_only*/ false,
                )? {
                    marketplaces.push(marketplace);
                }
            }
            RemoteMarketplaceSource::SharedWithMe => {
                // The shared endpoint is the source of truth for plugins explicitly shared
                // with the user. Installed unlisted plugins that are not returned there are
                // link-installed and stay in the separate unlisted bucket.
                let shared_plugins = fetch_shared_workspace_plugins(config, auth).await?;
                let shared_plugin_ids = shared_plugins
                    .iter()
                    .map(|plugin| plugin.id.clone())
                    .collect::<BTreeSet<_>>();
                let directly_shared_plugins = shared_plugins
                    .into_iter()
                    .filter_map(|plugin| match workspace_plugin_discoverability(&plugin) {
                        Ok(
                            RemotePluginShareDiscoverability::Private
                            | RemotePluginShareDiscoverability::Unlisted,
                        ) => Some(Ok(plugin)),
                        Ok(RemotePluginShareDiscoverability::Listed) => None,
                        Err(err) => Some(Err(err)),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(marketplace) = build_remote_marketplace(
                    REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME,
                    REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_DISPLAY_NAME,
                    directly_shared_plugins,
                    workspace_installed_plugins.clone().unwrap_or_default(),
                    /*include_installed_only*/ false,
                )? {
                    marketplaces.push(marketplace);
                }

                let unlisted_installed_plugins = workspace_installed_plugins
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(
                        |plugin| match workspace_plugin_discoverability(&plugin.plugin) {
                            Ok(RemotePluginShareDiscoverability::Unlisted)
                                if !shared_plugin_ids.contains(&plugin.plugin.id) =>
                            {
                                Some(Ok(plugin))
                            }
                            Ok(RemotePluginShareDiscoverability::Unlisted) => None,
                            Ok(RemotePluginShareDiscoverability::Listed)
                            | Ok(RemotePluginShareDiscoverability::Private) => None,
                            Err(err) => Some(Err(err)),
                        },
                    )
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(marketplace) = build_remote_marketplace(
                    REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME,
                    REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_DISPLAY_NAME,
                    Vec::new(),
                    unlisted_installed_plugins,
                    /*include_installed_only*/ true,
                )? {
                    marketplaces.push(marketplace);
                }
            }
        }
    }

    Ok(marketplaces)
}

pub async fn fetch_and_cache_global_remote_plugin_catalog(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<(), RemotePluginCatalogError> {
    fetch_and_cache_global_remote_plugin_catalog_snapshot(codex_home, config, auth)
        .await
        .map(drop)
}

pub(crate) async fn fetch_and_cache_global_remote_plugin_catalog_snapshot(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<Vec<RemoteDiscoverablePlugin>, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let plugins =
        fetch_directory_plugins_for_scope(config, auth, RemotePluginScope::Global).await?;
    catalog_cache::write_cached_global_directory_plugins(codex_home, config, auth, &plugins);
    Ok(remote_discoverable_plugins_from_directory_items(plugins))
}

pub async fn fetch_recommended_plugins(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<RecommendedPluginsMode, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/suggested");
    let client = build_reqwest_client();
    let request = authenticated_request(client.get(&url), auth)?
        .timeout(RECOMMENDED_PLUGINS_TIMEOUT)
        .query(&[("scope", "GLOBAL")]);
    let response: RecommendedPluginsResponse = send_and_decode(request, &url).await?;
    Ok(recommended_plugins_mode(response))
}

fn recommended_plugins_mode(response: RecommendedPluginsResponse) -> RecommendedPluginsMode {
    if response.enabled != Some(true) {
        return RecommendedPluginsMode::Legacy;
    }

    let mut plugins = BTreeMap::new();
    for plugin in response.plugins {
        if !is_valid_remote_plugin_id(&plugin.id)
            || plugin.name.chars().count() > MAX_RECOMMENDED_PLUGIN_NAME_LEN
            || plugin
                .status
                .is_some_and(|status| status != PluginAvailability::Available)
            || plugin
                .installation_policy
                .is_some_and(|policy| policy != PluginInstallPolicy::Available)
        {
            continue;
        }
        let plugin_id = match PluginId::new(
            plugin.name.clone(),
            REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(),
        ) {
            Ok(plugin_id) => plugin_id,
            Err(err) => {
                tracing::warn!(
                    plugin_name = plugin.name,
                    error = %err,
                    "ignoring invalid recommended plugin"
                );
                continue;
            }
        };
        let RecommendedPluginRelease {
            display_name,
            app_ids,
        } = plugin.release;
        let display_name = non_empty_string(Some(&display_name))
            .unwrap_or_else(|| plugin.name.clone())
            .chars()
            .take(MAX_RECOMMENDED_PLUGIN_DISPLAY_NAME_LEN)
            .collect();
        let mut seen_app_ids = HashSet::new();
        let app_connector_ids = app_ids
            .into_iter()
            .filter(|app_id| !app_id.is_empty() && seen_app_ids.insert(app_id.clone()))
            .collect();
        let config_id = plugin_id.as_key();
        plugins
            .entry(config_id.clone())
            .or_insert(RecommendedPlugin {
                config_id,
                remote_plugin_id: plugin.id,
                display_name,
                app_connector_ids,
            });
    }

    RecommendedPluginsMode::Endpoint {
        plugins: plugins
            .into_values()
            .take(MAX_RECOMMENDED_PLUGINS)
            .collect(),
    }
}

pub fn has_cached_global_remote_plugin_catalog(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> bool {
    let Ok(auth) = ensure_chatgpt_auth(auth) else {
        return false;
    };
    catalog_cache::load_cached_global_directory_plugins(codex_home, config, auth).is_some()
}

pub fn cached_global_remote_discoverable_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
) -> Vec<RemoteDiscoverablePlugin> {
    load_cached_global_remote_discoverable_plugins(codex_home, config, auth).unwrap_or_default()
}

pub(crate) fn load_cached_global_remote_discoverable_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
) -> Option<Vec<RemoteDiscoverablePlugin>> {
    catalog_cache::load_cached_global_directory_plugins(codex_home, config, auth)
        .map(remote_discoverable_plugins_from_directory_items)
}

fn remote_discoverable_plugins_from_directory_items(
    plugins: Vec<RemotePluginDirectoryItem>,
) -> Vec<RemoteDiscoverablePlugin> {
    plugins
        .into_iter()
        .filter_map(|plugin| match remote_discoverable_plugin_from_directory_item(&plugin) {
            Ok(plugin) => Some(plugin),
            Err(err) => {
                tracing::warn!(error = %err, "ignoring cached remote plugin recommendation entry");
                None
            }
        })
        .collect()
}

pub async fn fetch_openai_curated_remote_collection_marketplace(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<Option<RemoteMarketplace>, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let scope = RemotePluginScope::Global;
    let (directory_plugins, installed_plugins) = tokio::try_join!(
        fetch_directory_plugins_for_scope_with_collection(
            config,
            auth,
            scope,
            OPENAI_CURATED_REMOTE_COLLECTION_KEY,
        ),
        fetch_installed_plugins_for_scope(config, auth, scope),
    )?;

    build_remote_marketplace(
        REMOTE_GLOBAL_MARKETPLACE_NAME,
        REMOTE_GLOBAL_MARKETPLACE_DISPLAY_NAME,
        directory_plugins,
        installed_plugins,
        /*include_installed_only*/ false,
    )
}

fn build_remote_marketplace(
    name: &str,
    display_name: &str,
    directory_plugins: Vec<RemotePluginDirectoryItem>,
    installed_plugins: Vec<RemotePluginInstalledItem>,
    include_installed_only: bool,
) -> Result<Option<RemoteMarketplace>, RemotePluginCatalogError> {
    let mut installed_plugins = installed_plugins
        .into_iter()
        .map(|plugin| (plugin.plugin.id.clone(), plugin))
        .collect::<BTreeMap<_, _>>();
    let mut plugins = directory_plugins
        .into_iter()
        .map(|plugin| {
            let installed_plugin = installed_plugins.remove(&plugin.id);
            build_remote_plugin_summary(&plugin, installed_plugin.as_ref())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if include_installed_only {
        plugins.extend(
            installed_plugins
                .into_values()
                .map(|plugin| build_remote_plugin_summary(&plugin.plugin, Some(&plugin)))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if plugins.is_empty() {
        return Ok(None);
    }

    Ok(Some(RemoteMarketplace {
        name: name.to_string(),
        display_name: display_name.to_string(),
        plugins,
    }))
}

pub(crate) async fn fetch_remote_installed_plugins(
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
    let user = async {
        let scope = RemotePluginScope::User;
        let installed_plugins = fetch_installed_plugins_for_scope(config, auth, scope).await?;
        Ok::<_, RemotePluginCatalogError>((scope, installed_plugins))
    };

    let (global, workspace, user) = tokio::try_join!(global, workspace, user)?;
    let mut installed_plugins = [global, workspace, user]
        .into_iter()
        .flat_map(|(_scope, plugins)| plugins)
        .map(|plugin| remote_installed_plugin_to_cache_entry(&plugin))
        .collect::<Result<Vec<_>, _>>()?;
    installed_plugins.sort_by(|left, right| {
        left.marketplace_name
            .cmp(&right.marketplace_name)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(installed_plugins)
}

pub fn group_remote_installed_plugins_by_marketplaces(
    plugins: &[RemoteInstalledPlugin],
    visible_marketplaces: &[&str],
) -> Vec<RemoteMarketplace> {
    let mut plugins_by_marketplace = BTreeMap::<String, Vec<RemotePluginSummary>>::new();

    for plugin in plugins {
        if !visible_marketplaces.contains(&plugin.marketplace_name.as_str()) {
            continue;
        }
        let Ok(plugin_id) = PluginId::new(plugin.name.clone(), plugin.marketplace_name.clone())
        else {
            continue;
        };
        let plugin_summary = RemotePluginSummary {
            id: plugin_id.as_key(),
            remote_plugin_id: plugin.id.clone(),
            name: plugin.name.clone(),
            share_context: None,
            installed: true,
            enabled: plugin.enabled,
            install_policy: plugin.install_policy,
            auth_policy: plugin.auth_policy,
            availability: plugin.availability,
            interface: plugin.interface.clone(),
            keywords: plugin.keywords.clone(),
        };
        plugins_by_marketplace
            .entry(plugin.marketplace_name.clone())
            .or_default()
            .push(plugin_summary);
    }

    REMOTE_INSTALLED_MARKETPLACE_DISPLAY_ORDER
        .into_iter()
        .filter_map(|(marketplace_name, display_name)| {
            let mut marketplace_plugins = plugins_by_marketplace.remove(marketplace_name)?;
            sort_remote_plugin_summaries_by_display_name(&mut marketplace_plugins);
            Some(RemoteMarketplace {
                name: marketplace_name.to_string(),
                display_name: display_name.to_string(),
                plugins: marketplace_plugins,
            })
        })
        .collect()
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

pub async fn fetch_remote_plugin_share_context(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    plugin_id: &str,
) -> Result<Option<RemotePluginShareContext>, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let plugin = fetch_plugin_detail(
        config, auth, plugin_id, /*include_download_urls*/ false,
    )
    .await?;
    remote_plugin_share_context(&plugin)
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
    let marketplace_name = remote_plugin_canonical_marketplace_name(&plugin)?.to_string();
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
    let mut app_declarations = plugin
        .release
        .app_manifest
        .as_ref()
        .map(plugin_app_declarations_from_value)
        .unwrap_or_else(|| app_declarations_from_remote_app_ids(&plugin.release.app_ids));
    let mut mcp_servers = plugin
        .release
        .mcp_servers
        .iter()
        .map(|server| (server.key.clone(), ()))
        .collect::<HashMap<_, _>>();
    apply_app_mcp_routing_policy(
        &mut app_declarations,
        &mut mcp_servers,
        Some(auth.api_auth_mode()),
        /*plugin_active*/ true,
    );
    let app_ids = app_connector_ids_from_declarations(&app_declarations)
        .into_iter()
        .map(|app_id| app_id.0)
        .collect();
    let mut mcp_servers = mcp_servers.into_keys().collect::<Vec<_>>();
    mcp_servers.sort_unstable();
    mcp_servers.dedup();

    Ok(RemotePluginDetail {
        marketplace_name,
        marketplace_display_name: scope.marketplace_display_name().to_string(),
        summary: build_remote_plugin_summary(&plugin, installed_plugin.as_ref())?,
        share_url: plugin.share_url,
        description: non_empty_string(Some(&plugin.release.description)),
        release_version: plugin.release.version,
        bundle_download_url: plugin.release.bundle_download_url,
        app_manifest: plugin.release.app_manifest,
        skills,
        app_ids,
        app_templates: plugin
            .release
            .app_templates
            .into_iter()
            .map(|template| RemoteAppTemplate {
                template_id: template.template_id,
                name: template.name,
                description: template.description,
                category: template.category,
                canonical_connector_id: template.canonical_connector_id,
                logo_url: template.logo_url,
                logo_url_dark: template.logo_url_dark,
                materialized_app_ids: template.materialized_app_ids,
                reason: template.reason,
            })
            .collect(),
        mcp_servers,
    })
}

fn app_declarations_from_remote_app_ids(app_ids: &[String]) -> Vec<AppDeclaration> {
    app_ids
        .iter()
        .map(|app_id| AppDeclaration {
            name: app_id.clone(),
            connector_id: AppConnectorId(app_id.clone()),
            category: None,
        })
        .collect()
}

pub async fn install_remote_plugin(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    _marketplace_name: &str,
    plugin_id: &str,
) -> Result<RemotePluginInstallResult, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    // Remote plugin IDs uniquely identify remote plugins, so the caller-provided
    // marketplace name is not validated before sending the install mutation.

    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/{plugin_id}/install");
    let client = build_reqwest_client();
    let request = authenticated_request(
        client
            .post(&url)
            .query(&[("includeAppsNeedingAuth", "true")]),
        auth,
    )?;
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

    Ok(RemotePluginInstallResult {
        app_ids_needing_auth: response.app_ids_needing_auth,
    })
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
    let marketplace_name = remote_plugin_canonical_marketplace_name(&plugin)?.to_string();
    let plugin_name = plugin.name;

    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/{plugin_id}/uninstall");
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
) -> Result<RemotePluginSummary, RemotePluginCatalogError> {
    let marketplace_name = remote_plugin_canonical_marketplace_name(plugin)?;
    let plugin_id =
        PluginId::new(plugin.name.clone(), marketplace_name.to_string()).map_err(|err| {
            RemotePluginCatalogError::UnexpectedResponse(format!(
                "invalid remote plugin config id for `{}` in `{marketplace_name}`: {err}",
                plugin.name
            ))
        })?;
    Ok(RemotePluginSummary {
        id: plugin_id.as_key(),
        remote_plugin_id: plugin.id.clone(),
        name: plugin.name.clone(),
        share_context: remote_plugin_share_context(plugin)?,
        installed: installed_plugin.is_some(),
        enabled: installed_plugin.is_some_and(|plugin| plugin.enabled),
        install_policy: plugin.installation_policy,
        auth_policy: plugin.authentication_policy,
        availability: plugin.availability,
        interface: remote_plugin_interface_to_info(plugin),
        keywords: plugin.release.keywords.clone(),
    })
}

fn remote_discoverable_plugin_from_directory_item(
    plugin: &RemotePluginDirectoryItem,
) -> Result<RemoteDiscoverablePlugin, RemotePluginCatalogError> {
    let marketplace_name = remote_plugin_canonical_marketplace_name(plugin)?;
    let plugin_id =
        PluginId::new(plugin.name.clone(), marketplace_name.to_string()).map_err(|err| {
            RemotePluginCatalogError::UnexpectedResponse(format!(
                "invalid remote plugin config id for `{}` in `{marketplace_name}`: {err}",
                plugin.name
            ))
        })?;
    let display_name =
        non_empty_string(Some(&plugin.release.display_name)).unwrap_or_else(|| plugin.name.clone());
    let description = non_empty_string(plugin.release.interface.short_description.as_deref())
        .or_else(|| non_empty_string(Some(&plugin.release.description)));

    Ok(RemoteDiscoverablePlugin {
        config_id: plugin_id.as_key(),
        remote_plugin_id: plugin.id.clone(),
        name: display_name,
        description,
        has_skills: !plugin.release.skills.is_empty(),
        app_ids: plugin.release.app_ids.clone(),
        install_policy: plugin.installation_policy,
        availability: plugin.availability,
    })
}

fn remote_plugin_share_context(
    plugin: &RemotePluginDirectoryItem,
) -> Result<Option<RemotePluginShareContext>, RemotePluginCatalogError> {
    match plugin.scope {
        RemotePluginScope::Global | RemotePluginScope::User => Ok(None),
        RemotePluginScope::Workspace => {
            let discoverability = workspace_plugin_discoverability(plugin)?;
            Ok(Some(RemotePluginShareContext {
                remote_plugin_id: plugin.id.clone(),
                remote_version: plugin.release.version.clone(),
                discoverability,
                share_url: plugin.share_url.clone(),
                creator_account_user_id: plugin.creator_account_user_id.clone(),
                creator_name: plugin.creator_name.clone(),
                share_principals: plugin.share_principals.as_ref().map(|share_principals| {
                    share_principals
                        .iter()
                        .map(|principal| RemotePluginSharePrincipal {
                            principal_type: principal.principal_type,
                            principal_id: principal.principal_id.clone(),
                            role: principal.role,
                            name: principal.name.clone(),
                        })
                        .collect()
                }),
            }))
        }
    }
}

fn remote_installed_plugin_to_cache_entry(
    installed_plugin: &RemotePluginInstalledItem,
) -> Result<RemoteInstalledPlugin, RemotePluginCatalogError> {
    let plugin = &installed_plugin.plugin;
    // Remote per-skill disabled state (`disabled_skill_names`) is intentionally
    // not projected into skills/list yet; local skills.config remains the
    // supported source for skill enablement.
    Ok(RemoteInstalledPlugin {
        marketplace_name: remote_plugin_canonical_marketplace_name(plugin)?.to_string(),
        id: plugin.id.clone(),
        name: plugin.name.clone(),
        enabled: installed_plugin.enabled,
        install_policy: plugin.installation_policy,
        auth_policy: plugin.authentication_policy,
        availability: plugin.availability,
        interface: remote_plugin_interface_to_info(plugin),
        keywords: plugin.release.keywords.clone(),
    })
}

fn remote_plugin_interface_to_info(plugin: &RemotePluginDirectoryItem) -> Option<PluginInterface> {
    let interface = &plugin.release.interface;
    let display_name = non_empty_string(Some(&plugin.release.display_name));
    let default_prompt = interface
        .default_prompts
        .as_deref()
        .and_then(normalize_remote_default_prompts)
        .or_else(|| {
            interface
                .default_prompt
                .as_deref()
                .and_then(normalize_remote_default_prompt)
                .map(|prompt| vec![prompt])
        });
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

fn sort_remote_plugin_summaries_by_display_name(plugins: &mut [RemotePluginSummary]) {
    plugins.sort_by(|left, right| {
        let left_display_name = remote_plugin_display_name(left);
        let right_display_name = remote_plugin_display_name(right);
        left_display_name
            .to_ascii_lowercase()
            .cmp(&right_display_name.to_ascii_lowercase())
            .then_with(|| left_display_name.cmp(right_display_name))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn normalize_remote_default_prompts(prompts: &[String]) -> Option<Vec<String>> {
    let prompts = prompts
        .iter()
        .filter_map(|prompt| normalize_remote_default_prompt(prompt))
        .take(MAX_REMOTE_DEFAULT_PROMPT_COUNT)
        .collect::<Vec<_>>();
    (!prompts.is_empty()).then_some(prompts)
}

fn normalize_remote_default_prompt(prompt: &str) -> Option<String> {
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.chars().count() > MAX_REMOTE_DEFAULT_PROMPT_LEN {
        return None;
    }
    Some(prompt.to_string())
}

async fn fetch_directory_plugins_for_scope(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
) -> Result<Vec<RemotePluginDirectoryItem>, RemotePluginCatalogError> {
    fetch_directory_plugins_for_scope_with_optional_collection(
        config, auth, scope, /*collection*/ None,
    )
    .await
}

async fn fetch_directory_plugins_for_scope_with_collection(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    collection: &str,
) -> Result<Vec<RemotePluginDirectoryItem>, RemotePluginCatalogError> {
    fetch_directory_plugins_for_scope_with_optional_collection(
        config,
        auth,
        scope,
        Some(collection),
    )
    .await
}

async fn fetch_directory_plugins_for_scope_with_optional_collection(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    collection: Option<&str>,
) -> Result<Vec<RemotePluginDirectoryItem>, RemotePluginCatalogError> {
    let mut plugins = Vec::new();
    let mut page_token = None;
    loop {
        let response =
            get_remote_plugin_list_page(config, auth, scope, page_token.as_deref(), collection)
                .await?;
        plugins.extend(response.plugins);
        let Some(next_page_token) = response.pagination.next_page_token else {
            break;
        };
        page_token = Some(next_page_token);
    }
    Ok(plugins)
}

async fn fetch_shared_workspace_plugins(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
) -> Result<Vec<RemotePluginDirectoryItem>, RemotePluginCatalogError> {
    let mut plugins = Vec::new();
    let mut page_token = None;
    loop {
        let response =
            get_remote_shared_workspace_plugins_page(config, auth, page_token.as_deref()).await?;
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
    collection: Option<&str>,
) -> Result<RemotePluginListResponse, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/list");
    let client = build_reqwest_client();
    let mut request = authenticated_request(client.get(&url), auth)?;
    request = request.query(&[("scope", scope.api_value())]);
    request = request.query(&[("limit", REMOTE_PLUGIN_LIST_PAGE_LIMIT)]);
    if let Some(collection) = collection {
        request = request.query(&[("collection", collection)]);
    }
    if let Some(page_token) = page_token {
        request = request.query(&[("pageToken", page_token)]);
    }
    send_and_decode(request, &url).await
}

async fn get_remote_shared_workspace_plugins_page(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    page_token: Option<&str>,
) -> Result<RemotePluginListResponse, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/workspace/shared");
    let client = build_reqwest_client();
    let mut request = authenticated_request(client.get(&url), auth)?;
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
        .headers(codex_model_provider::auth_provider_from_auth(auth).to_auth_headers())
        .header(OAI_PRODUCT_SKU_HEADER, CODEX_PRODUCT_SKU))
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
