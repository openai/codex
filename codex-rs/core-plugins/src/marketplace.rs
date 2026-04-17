use crate::manifest::PluginManifestInterface;
use crate::manifest::load_plugin_manifest;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_git_utils::get_git_repo_root;
use codex_plugin::PluginId;
use codex_plugin::PluginIdError;
use codex_protocol::protocol::Product;
use codex_utils_absolute_path::AbsolutePathBuf;
use dirs::home_dir;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

const MARKETPLACE_MANIFEST_RELATIVE_PATHS: &[&str] = &[
    ".agents/plugins/marketplace.json",
    ".claude-plugin/marketplace.json",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMarketplacePlugin {
    pub plugin_id: PluginId,
    pub source: MarketplacePluginSource,
    pub policy: MarketplacePluginPolicy,
    pub interface: Option<PluginManifestInterface>,
    pub manifest: Option<crate::manifest::PluginManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marketplace {
    pub name: String,
    pub path: AbsolutePathBuf,
    pub interface: Option<MarketplaceInterface>,
    pub plugins: Vec<MarketplacePlugin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceListError {
    pub path: AbsolutePathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarketplaceListOutcome {
    pub marketplaces: Vec<Marketplace>,
    pub errors: Vec<MarketplaceListError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceInterface {
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplacePlugin {
    pub name: String,
    pub source: MarketplacePluginSource,
    pub policy: MarketplacePluginPolicy,
    pub interface: Option<PluginManifestInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketplacePluginSource {
    Local {
        path: AbsolutePathBuf,
    },
    Git {
        url: String,
        path: Option<String>,
        ref_name: Option<String>,
        sha: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplacePluginPolicy {
    pub installation: MarketplacePluginInstallPolicy,
    pub authentication: MarketplacePluginAuthPolicy,
    // TODO: Surface or enforce product gating at the Codex/plugin consumer boundary instead of
    // only carrying it through core marketplace metadata.
    pub products: Option<Vec<Product>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum MarketplacePluginInstallPolicy {
    #[serde(rename = "NOT_AVAILABLE")]
    NotAvailable,
    #[default]
    #[serde(rename = "AVAILABLE")]
    Available,
    #[serde(rename = "INSTALLED_BY_DEFAULT")]
    InstalledByDefault,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum MarketplacePluginAuthPolicy {
    #[default]
    #[serde(rename = "ON_INSTALL")]
    OnInstall,
    #[serde(rename = "ON_USE")]
    OnUse,
}

impl From<MarketplacePluginInstallPolicy> for PluginInstallPolicy {
    fn from(value: MarketplacePluginInstallPolicy) -> Self {
        match value {
            MarketplacePluginInstallPolicy::NotAvailable => Self::NotAvailable,
            MarketplacePluginInstallPolicy::Available => Self::Available,
            MarketplacePluginInstallPolicy::InstalledByDefault => Self::InstalledByDefault,
        }
    }
}

impl From<MarketplacePluginAuthPolicy> for PluginAuthPolicy {
    fn from(value: MarketplacePluginAuthPolicy) -> Self {
        match value {
            MarketplacePluginAuthPolicy::OnInstall => Self::OnInstall,
            MarketplacePluginAuthPolicy::OnUse => Self::OnUse,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MarketplaceError {
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("marketplace file `{path}` does not exist")]
    MarketplaceNotFound { path: PathBuf },

    #[error("invalid marketplace file `{path}`: {message}")]
    InvalidMarketplaceFile { path: PathBuf, message: String },

    #[error("plugin `{plugin_name}` was not found in marketplace `{marketplace_name}`")]
    PluginNotFound {
        plugin_name: String,
        marketplace_name: String,
    },

    #[error(
        "plugin `{plugin_name}` is not available for install in marketplace `{marketplace_name}`"
    )]
    PluginNotAvailable {
        plugin_name: String,
        marketplace_name: String,
    },

    #[error("plugins feature is disabled")]
    PluginsDisabled,

    #[error("{0}")]
    InvalidPlugin(String),
}

impl MarketplaceError {
    fn io(context: &'static str, source: io::Error) -> Self {
        Self::Io { context, source }
    }
}

pub fn find_marketplace_plugin(
    marketplace_path: &AbsolutePathBuf,
    plugin_name: &str,
) -> Result<ResolvedMarketplacePlugin, MarketplaceError> {
    let marketplace = load_raw_marketplace_manifest(marketplace_path)?;
    let marketplace_name = marketplace.name;
    let marketplace_name_for_not_found = marketplace_name.clone();
    for plugin in marketplace.plugins {
        if plugin.name != plugin_name {
            continue;
        }

        if let Some(plugin) =
            resolve_marketplace_plugin_entry(marketplace_path, &marketplace_name, plugin)?
        {
            return Ok(plugin);
        }
    }

    Err(MarketplaceError::PluginNotFound {
        plugin_name: plugin_name.to_string(),
        marketplace_name: marketplace_name_for_not_found,
    })
}

pub fn find_installable_marketplace_plugin(
    marketplace_path: &AbsolutePathBuf,
    plugin_name: &str,
    restriction_product: Option<Product>,
) -> Result<ResolvedMarketplacePlugin, MarketplaceError> {
    let resolved = find_marketplace_plugin(marketplace_path, plugin_name)?;
    let product_allowed = match resolved.policy.products.as_deref() {
        None => true,
        Some([]) => false,
        Some(products) => {
            restriction_product.is_some_and(|product| product.matches_product_restriction(products))
        }
    };
    if resolved.policy.installation == MarketplacePluginInstallPolicy::NotAvailable
        || !product_allowed
    {
        return Err(MarketplaceError::PluginNotAvailable {
            plugin_name: resolved.plugin_id.plugin_name,
            marketplace_name: resolved.plugin_id.marketplace_name,
        });
    }

    Ok(resolved)
}

pub fn list_marketplaces(
    additional_roots: &[AbsolutePathBuf],
) -> Result<MarketplaceListOutcome, MarketplaceError> {
    list_marketplaces_with_home(additional_roots, home_dir().as_deref())
}

pub fn validate_marketplace_root(root: &Path) -> Result<String, MarketplaceError> {
    let Some(path) = find_marketplace_manifest_path(root) else {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: root.to_path_buf(),
            message: "marketplace root does not contain a supported manifest".to_string(),
        });
    };
    let marketplace = load_marketplace(&path)?;
    Ok(marketplace.name)
}

pub fn find_marketplace_manifest_path(root: &Path) -> Option<AbsolutePathBuf> {
    MARKETPLACE_MANIFEST_RELATIVE_PATHS
        .iter()
        .find_map(|relative_path| {
            let path = root.join(relative_path);
            if !path.is_file() {
                return None;
            }
            AbsolutePathBuf::try_from(path).ok()
        })
}

fn invalid_marketplace_layout_error(path: &AbsolutePathBuf) -> MarketplaceError {
    MarketplaceError::InvalidMarketplaceFile {
        path: path.to_path_buf(),
        message: "marketplace file is not in a supported location".to_string(),
    }
}

fn marketplace_root_from_layout(marketplace_path: &Path, relative_path: &str) -> Option<PathBuf> {
    let mut current = marketplace_path;
    for component in Path::new(relative_path).components().rev() {
        let expected = match component {
            Component::Normal(expected) => expected,
            _ => return None,
        };
        if current.file_name() != Some(expected) {
            return None;
        }
        current = current.parent()?;
    }
    Some(current.to_path_buf())
}

pub fn load_marketplace(path: &AbsolutePathBuf) -> Result<Marketplace, MarketplaceError> {
    let marketplace = load_raw_marketplace_manifest(path)?;
    let mut plugins = Vec::new();

    for plugin in marketplace.plugins {
        let plugin = match resolve_marketplace_plugin_entry(path, &marketplace.name, plugin) {
            Ok(Some(plugin)) => plugin,
            Ok(None) => continue,
            Err(MarketplaceError::InvalidPlugin(message)) => {
                warn!(
                    path = %path.display(),
                    marketplace = %marketplace.name,
                    error = %message,
                    "skipping invalid marketplace plugin"
                );
                continue;
            }
            Err(err) => return Err(err),
        };

        plugins.push(MarketplacePlugin {
            name: plugin.plugin_id.plugin_name,
            source: plugin.source,
            policy: plugin.policy,
            interface: plugin.interface,
        });
    }

    Ok(Marketplace {
        name: marketplace.name,
        path: path.clone(),
        interface: resolve_marketplace_interface(marketplace.interface),
        plugins,
    })
}

#[doc(hidden)]
pub fn list_marketplaces_with_home(
    additional_roots: &[AbsolutePathBuf],
    home_dir: Option<&Path>,
) -> Result<MarketplaceListOutcome, MarketplaceError> {
    let mut outcome = MarketplaceListOutcome::default();

    for marketplace_path in discover_marketplace_paths_from_roots(additional_roots, home_dir) {
        match load_marketplace(&marketplace_path) {
            Ok(marketplace) => outcome.marketplaces.push(marketplace),
            Err(err) => {
                warn!(
                    path = %marketplace_path.display(),
                    error = %err,
                    "skipping marketplace that failed to load"
                );
                outcome.errors.push(MarketplaceListError {
                    path: marketplace_path,
                    message: err.to_string(),
                });
            }
        }
    }

    Ok(outcome)
}

fn discover_marketplace_paths_from_roots(
    additional_roots: &[AbsolutePathBuf],
    home_dir: Option<&Path>,
) -> Vec<AbsolutePathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = home_dir
        && let Some(path) = find_marketplace_manifest_path(home)
    {
        paths.push(path);
    }

    for root in additional_roots {
        // Curated marketplaces can now come from an HTTP-downloaded directory that is not a git
        // checkout, so check the root directly before falling back to repo-root discovery.
        if let Some(path) = find_marketplace_manifest_path(root.as_path())
            && !paths.contains(&path)
        {
            paths.push(path);
            continue;
        }
        if let Some(repo_root) = get_git_repo_root(root.as_path())
            && let Ok(repo_root) = AbsolutePathBuf::try_from(repo_root)
            && let Some(path) = find_marketplace_manifest_path(repo_root.as_path())
            && !paths.contains(&path)
        {
            paths.push(path);
        }
    }

    paths
}

fn load_raw_marketplace_manifest(
    path: &AbsolutePathBuf,
) -> Result<RawMarketplaceManifest, MarketplaceError> {
    let contents = fs::read_to_string(path.as_path()).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            MarketplaceError::MarketplaceNotFound {
                path: path.to_path_buf(),
            }
        } else {
            MarketplaceError::io("failed to read marketplace file", err)
        }
    })?;
    serde_json::from_str(&contents).map_err(|err| MarketplaceError::InvalidMarketplaceFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

fn resolve_marketplace_plugin_entry(
    marketplace_path: &AbsolutePathBuf,
    marketplace_name: &str,
    plugin: RawMarketplaceManifestPlugin,
) -> Result<Option<ResolvedMarketplacePlugin>, MarketplaceError> {
    let RawMarketplaceManifestPlugin {
        name,
        source,
        policy,
        category,
    } = plugin;
    let Some(source) = resolve_supported_plugin_source(marketplace_path, &name, source) else {
        return Ok(None);
    };

    let manifest = match &source {
        MarketplacePluginSource::Local { path } => load_plugin_manifest(path.as_path()),
        MarketplacePluginSource::Git { .. } => None,
    };
    let interface = plugin_interface_with_marketplace_category(
        manifest
            .as_ref()
            .and_then(|manifest| manifest.interface.clone()),
        category,
    );

    Ok(Some(ResolvedMarketplacePlugin {
        plugin_id: PluginId::new(name, marketplace_name.to_string()).map_err(|err| match err {
            PluginIdError::Invalid(message) => MarketplaceError::InvalidPlugin(message),
        })?,
        source,
        policy: MarketplacePluginPolicy {
            installation: policy.installation,
            authentication: policy.authentication,
            products: policy.products,
        },
        interface,
        manifest,
    }))
}

fn resolve_supported_plugin_source(
    marketplace_path: &AbsolutePathBuf,
    plugin_name: &str,
    source: RawMarketplaceManifestPluginSource,
) -> Option<MarketplacePluginSource> {
    match source {
        RawMarketplaceManifestPluginSource::Unsupported(_) => {
            warn!(
                path = %marketplace_path.display(),
                plugin = plugin_name,
                "skipping marketplace plugin with unsupported source"
            );
            None
        }
        source => match resolve_plugin_source(marketplace_path, source) {
            Ok(source) => Some(source),
            Err(err) => {
                warn!(
                    path = %marketplace_path.display(),
                    plugin = plugin_name,
                    error = %err,
                    "skipping marketplace plugin that failed to resolve"
                );
                None
            }
        },
    }
}

fn resolve_plugin_source(
    marketplace_path: &AbsolutePathBuf,
    source: RawMarketplaceManifestPluginSource,
) -> Result<MarketplacePluginSource, MarketplaceError> {
    match source {
        RawMarketplaceManifestPluginSource::Path(path)
        | RawMarketplaceManifestPluginSource::Object(
            RawMarketplaceManifestPluginSourceObject::Local { path },
        ) => Ok(MarketplacePluginSource::Local {
            path: resolve_local_plugin_source_path(marketplace_path, &path)?,
        }),
        RawMarketplaceManifestPluginSource::Object(
            RawMarketplaceManifestPluginSourceObject::Url {
                url,
                path,
                ref_name,
                sha,
            },
        ) => Ok(MarketplacePluginSource::Git {
            url: normalize_git_plugin_source_url(marketplace_path, &url)?,
            path: path
                .as_deref()
                .map(|path| normalize_remote_plugin_subdir(marketplace_path, path))
                .transpose()?,
            ref_name: normalize_optional_git_selector(&ref_name),
            sha: normalize_optional_git_selector(&sha),
        }),
        RawMarketplaceManifestPluginSource::Object(
            RawMarketplaceManifestPluginSourceObject::GitSubdir {
                url,
                path,
                ref_name,
                sha,
            },
        ) => Ok(MarketplacePluginSource::Git {
            url: normalize_git_plugin_source_url(marketplace_path, &url)?,
            path: Some(normalize_remote_plugin_subdir(marketplace_path, &path)?),
            ref_name: normalize_optional_git_selector(&ref_name),
            sha: normalize_optional_git_selector(&sha),
        }),
        RawMarketplaceManifestPluginSource::Unsupported(_) => {
            unreachable!("unsupported plugin sources should be filtered before resolution")
        }
    }
}

fn resolve_local_plugin_source_path(
    marketplace_path: &AbsolutePathBuf,
    path: &str,
) -> Result<AbsolutePathBuf, MarketplaceError> {
    let Some(path) = path.strip_prefix("./") else {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "local plugin source path must start with `./`".to_string(),
        });
    };
    if path.is_empty() {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "local plugin source path must not be empty".to_string(),
        });
    }

    let relative_source_path = Path::new(path);
    if relative_source_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "local plugin source path must stay within the marketplace root".to_string(),
        });
    }

    // `marketplace.json` lives under a supported marketplace layout beneath `<root>`,
    // but local plugin paths are resolved relative to `<root>`.
    Ok(marketplace_root_dir(marketplace_path)?.join(relative_source_path))
}

fn normalize_remote_plugin_subdir(
    marketplace_path: &AbsolutePathBuf,
    path: &str,
) -> Result<String, MarketplaceError> {
    let path = path.trim();
    let path = path.strip_prefix("./").unwrap_or(path);
    if path.is_empty() {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "git plugin source path must not be empty".to_string(),
        });
    }
    let relative_path = Path::new(path);
    if relative_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "git plugin source path must stay within the repository root".to_string(),
        });
    }
    Ok(path.to_string())
}

fn normalize_git_plugin_source_url(
    marketplace_path: &AbsolutePathBuf,
    url: &str,
) -> Result<String, MarketplaceError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "git plugin source url must not be empty".to_string(),
        });
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return Ok(normalize_github_git_url(url));
    }
    if url.starts_with("./")
        || url.starts_with("../")
        || url.starts_with(".\\")
        || url.starts_with("..\\")
    {
        return Ok(normalize_relative_git_plugin_source_url(
            marketplace_path,
            url,
        )?);
    }
    if url.starts_with("file://") || url.starts_with('/') {
        return Ok(url.to_string());
    }
    if url.starts_with("ssh://") || url.starts_with("git@") && url.contains(':') {
        return Ok(url.to_string());
    }
    if let Some(url) = normalize_github_shorthand_url(url) {
        return Ok(url);
    }

    Err(MarketplaceError::InvalidMarketplaceFile {
        path: marketplace_path.to_path_buf(),
        message: format!("invalid git plugin source url: {url}"),
    })
}

fn normalize_relative_git_plugin_source_url(
    marketplace_path: &AbsolutePathBuf,
    url: &str,
) -> Result<String, MarketplaceError> {
    let mut normalized = marketplace_root_dir(marketplace_path)?
        .as_path()
        .to_path_buf();
    for segment in url.split(['/', '\\']) {
        match segment {
            "" | "." => {}
            ".." => {
                normalized.pop();
            }
            segment => normalized.push(segment),
        }
    }
    Ok(normalized.display().to_string())
}

fn normalize_optional_git_selector(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalize_github_git_url(url: &str) -> String {
    if url.starts_with("https://github.com/") && !url.ends_with(".git") {
        format!("{url}.git")
    } else {
        url.to_string()
    }
}

fn normalize_github_shorthand_url(source: &str) -> Option<String> {
    if !looks_like_github_shorthand(source) {
        return None;
    }
    let mut segments = source.split('/');
    let owner = segments.next()?;
    let repo = segments.next()?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if repo.is_empty() {
        return None;
    }
    Some(format!("https://github.com/{owner}/{repo}.git"))
}

fn looks_like_github_shorthand(source: &str) -> bool {
    let mut segments = source.split('/');
    let owner = segments.next();
    let repo = segments.next();
    let extra = segments.next();
    owner.is_some_and(is_github_shorthand_segment)
        && repo.is_some_and(is_github_shorthand_segment)
        && extra.is_none()
}

fn is_github_shorthand_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

pub fn plugin_interface_with_marketplace_category(
    mut interface: Option<PluginManifestInterface>,
    category: Option<String>,
) -> Option<PluginManifestInterface> {
    if let Some(category) = category {
        // Marketplace taxonomy wins when both sources provide a category.
        interface
            .get_or_insert_with(PluginManifestInterface::default)
            .category = Some(category);
    }
    interface
}

fn marketplace_root_dir(
    marketplace_path: &AbsolutePathBuf,
) -> Result<AbsolutePathBuf, MarketplaceError> {
    for relative_path in MARKETPLACE_MANIFEST_RELATIVE_PATHS {
        if let Some(marketplace_root) =
            marketplace_root_from_layout(marketplace_path.as_path(), relative_path)
        {
            return AbsolutePathBuf::try_from(marketplace_root)
                .map_err(|_| invalid_marketplace_layout_error(marketplace_path));
        }
    }

    Err(invalid_marketplace_layout_error(marketplace_path))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMarketplaceManifest {
    name: String,
    #[serde(default)]
    interface: Option<RawMarketplaceManifestInterface>,
    plugins: Vec<RawMarketplaceManifestPlugin>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMarketplaceManifestInterface {
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMarketplaceManifestPlugin {
    name: String,
    source: RawMarketplaceManifestPluginSource,
    #[serde(default)]
    policy: RawMarketplaceManifestPluginPolicy,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMarketplaceManifestPluginPolicy {
    #[serde(default)]
    installation: MarketplacePluginInstallPolicy,
    #[serde(default)]
    authentication: MarketplacePluginAuthPolicy,
    products: Option<Vec<Product>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawMarketplaceManifestPluginSource {
    Path(String),
    Object(RawMarketplaceManifestPluginSourceObject),
    #[allow(dead_code)]
    Unsupported(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
enum RawMarketplaceManifestPluginSourceObject {
    Local {
        path: String,
    },
    Url {
        url: String,
        path: Option<String>,
        #[serde(rename = "ref")]
        ref_name: Option<String>,
        sha: Option<String>,
    },
    #[serde(rename = "git-subdir")]
    GitSubdir {
        url: String,
        path: String,
        #[serde(rename = "ref")]
        ref_name: Option<String>,
        sha: Option<String>,
    },
}

fn resolve_marketplace_interface(
    interface: Option<RawMarketplaceManifestInterface>,
) -> Option<MarketplaceInterface> {
    let interface = interface?;
    if interface.display_name.is_some() {
        Some(MarketplaceInterface {
            display_name: interface.display_name,
        })
    } else {
        None
    }
}

#[cfg(test)]
#[path = "marketplace_tests.rs"]
mod tests;
