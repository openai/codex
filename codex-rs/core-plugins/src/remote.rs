use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInterface;
use codex_app_server_protocol::SkillInterface;
use codex_login::CodexAuth;
use codex_login::default_client::build_reqwest_client;
use reqwest::RequestBuilder;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::time::Duration;

pub const REMOTE_GLOBAL_MARKETPLACE_NAME: &str = "chatgpt-global";
pub const REMOTE_WORKSPACE_MARKETPLACE_NAME: &str = "chatgpt-workspace";
pub const REMOTE_GLOBAL_MARKETPLACE_DISPLAY_NAME: &str = "ChatGPT Plugins";
pub const REMOTE_WORKSPACE_MARKETPLACE_DISPLAY_NAME: &str = "ChatGPT Workspace Plugins";

const REMOTE_PLUGIN_CATALOG_TIMEOUT: Duration = Duration::from_secs(30);

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
pub struct RemotePluginSummary {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub enabled: bool,
    pub install_policy: PluginInstallPolicy,
    pub auth_policy: PluginAuthPolicy,
    pub interface: Option<PluginInterface>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginDetail {
    pub marketplace_name: String,
    pub marketplace_display_name: String,
    pub summary: RemotePluginSummary,
    pub description: Option<String>,
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

    #[error("remote marketplace `{marketplace_name}` is not supported")]
    UnknownMarketplace { marketplace_name: String },

    #[error(
        "remote plugin `{plugin_id}` belongs to marketplace `{actual_marketplace_name}`, not `{expected_marketplace_name}`"
    )]
    MarketplaceMismatch {
        plugin_id: String,
        expected_marketplace_name: String,
        actual_marketplace_name: String,
    },
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
    #[serde(alias = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginSkillInterfaceResponse {
    #[serde(alias = "displayName")]
    display_name: Option<String>,
    #[serde(alias = "shortDescription")]
    short_description: Option<String>,
    #[serde(alias = "brandColor")]
    brand_color: Option<String>,
    #[serde(alias = "defaultPrompt")]
    default_prompt: Option<String>,
    #[serde(alias = "iconSmallUrl")]
    icon_small_url: Option<String>,
    #[serde(alias = "iconLargeUrl")]
    icon_large_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginSkillResponse {
    name: String,
    description: String,
    interface: Option<RemotePluginSkillInterfaceResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginReleaseInterfaceResponse {
    #[serde(alias = "shortDescription")]
    short_description: Option<String>,
    #[serde(alias = "longDescription")]
    long_description: Option<String>,
    #[serde(alias = "developerName")]
    developer_name: Option<String>,
    category: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(alias = "websiteUrl")]
    website_url: Option<String>,
    #[serde(alias = "privacyPolicyUrl")]
    privacy_policy_url: Option<String>,
    #[serde(alias = "termsOfServiceUrl")]
    terms_of_service_url: Option<String>,
    #[serde(alias = "brandColor")]
    brand_color: Option<String>,
    #[serde(alias = "defaultPrompt")]
    default_prompt: Option<String>,
    #[serde(alias = "composerIconUrl")]
    composer_icon_url: Option<String>,
    #[serde(alias = "logoUrl")]
    logo_url: Option<String>,
    #[serde(default)]
    #[serde(alias = "screenshotUrls")]
    screenshot_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginReleaseResponse {
    #[serde(alias = "displayName")]
    display_name: String,
    description: String,
    #[serde(default)]
    #[serde(alias = "appIds")]
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
    #[serde(alias = "installationPolicy")]
    installation_policy: PluginInstallPolicy,
    #[serde(alias = "authenticationPolicy")]
    authentication_policy: PluginAuthPolicy,
    release: RemotePluginReleaseResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemotePluginInstalledItem {
    #[serde(flatten)]
    plugin: RemotePluginDirectoryItem,
    enabled: bool,
    #[serde(default)]
    #[serde(alias = "disabledSkillNames")]
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

pub async fn fetch_remote_plugin_detail(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    marketplace_name: &str,
    plugin_id: &str,
) -> Result<RemotePluginDetail, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let scope = RemotePluginScope::from_marketplace_name(marketplace_name).ok_or_else(|| {
        RemotePluginCatalogError::UnknownMarketplace {
            marketplace_name: marketplace_name.to_string(),
        }
    })?;
    let plugin = fetch_plugin_detail(config, auth, plugin_id).await?;
    let actual_marketplace_name = plugin.scope.marketplace_name();
    if actual_marketplace_name != marketplace_name {
        return Err(RemotePluginCatalogError::MarketplaceMismatch {
            plugin_id: plugin_id.to_string(),
            expected_marketplace_name: marketplace_name.to_string(),
            actual_marketplace_name: actual_marketplace_name.to_string(),
        });
    }

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
        marketplace_name: marketplace_name.to_string(),
        marketplace_display_name: scope.marketplace_display_name().to_string(),
        summary: build_remote_plugin_summary(&plugin, installed_plugin.as_ref()),
        description: non_empty_string(Some(&plugin.release.description)),
        skills,
        app_ids: plugin.release.app_ids,
    })
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
        interface: remote_plugin_interface_to_info(plugin),
    }
}

fn remote_plugin_interface_to_info(plugin: &RemotePluginDirectoryItem) -> Option<PluginInterface> {
    let interface = &plugin.release.interface;
    let display_name = non_empty_string(Some(&plugin.release.display_name));
    let default_prompt = interface
        .default_prompt
        .as_ref()
        .map(|prompt| vec![prompt.clone()]);
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
    let mut plugins = Vec::new();
    let mut page_token = None;
    loop {
        let response =
            get_remote_plugin_installed_page(config, auth, scope, page_token.as_deref()).await?;
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
) -> Result<RemotePluginInstalledResponse, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/installed");
    let client = build_reqwest_client();
    let mut request = authenticated_request(client.get(&url), auth)?;
    request = request.query(&[("scope", scope.api_value())]);
    if let Some(page_token) = page_token {
        request = request.query(&[("pageToken", page_token)]);
    }
    send_and_decode(request, &url).await
}

async fn fetch_plugin_detail(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    plugin_id: &str,
) -> Result<RemotePluginDirectoryItem, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/{plugin_id}");
    let client = build_reqwest_client();
    let request = authenticated_request(client.get(&url), auth)?;
    send_and_decode(request, &url).await
}

fn ensure_chatgpt_auth(auth: Option<&CodexAuth>) -> Result<&CodexAuth, RemotePluginCatalogError> {
    let Some(auth) = auth else {
        return Err(RemotePluginCatalogError::AuthRequired);
    };
    if !auth.is_chatgpt_auth() {
        return Err(RemotePluginCatalogError::UnsupportedAuthMode);
    }
    Ok(auth)
}

fn authenticated_request(
    request: RequestBuilder,
    auth: &CodexAuth,
) -> Result<RequestBuilder, RemotePluginCatalogError> {
    let token = auth
        .get_token()
        .map_err(RemotePluginCatalogError::AuthToken)?;
    let mut request = request
        .timeout(REMOTE_PLUGIN_CATALOG_TIMEOUT)
        .bearer_auth(token);
    if let Some(account_id) = auth.get_account_id() {
        request = request.header("chatgpt-account-id", account_id);
    }
    Ok(request)
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
