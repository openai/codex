use super::PluginManifestInterfaceSummary;
use super::load_plugin_manifest;
use super::plugin_manifest_interface;
use super::store::PluginId;
use super::store::PluginIdError;
use crate::git_info::get_git_repo_root;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use dirs::home_dir;
use serde::Deserialize;
use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

const MARKETPLACE_RELATIVE_PATH: &str = ".agents/plugins/marketplace.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMarketplacePlugin {
    pub plugin_id: PluginId,
    pub source_path: AbsolutePathBuf,
    pub auth_policy: MarketplacePluginAuthPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceSummary {
    pub name: String,
    pub path: AbsolutePathBuf,
    pub plugins: Vec<MarketplacePluginSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplacePluginSummary {
    pub name: String,
    pub source: MarketplacePluginSourceSummary,
    pub install_policy: MarketplacePluginInstallPolicy,
    pub auth_policy: MarketplacePluginAuthPolicy,
    pub interface: Option<PluginManifestInterfaceSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketplacePluginSourceSummary {
    Local { path: AbsolutePathBuf },
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

    #[error("{0}")]
    InvalidPlugin(String),
}

impl MarketplaceError {
    fn io(context: &'static str, source: io::Error) -> Self {
        Self::Io { context, source }
    }
}

// Always read the specified marketplace file from disk so installs see the
// latest marketplace.json contents without any in-memory cache invalidation.
pub fn resolve_marketplace_plugin(
    marketplace_path: &AbsolutePathBuf,
    plugin_name: &str,
) -> Result<ResolvedMarketplacePlugin, MarketplaceError> {
    let marketplace = load_marketplace(marketplace_path)?;
    let marketplace_name = marketplace.name;
    let plugin = marketplace
        .plugins
        .into_iter()
        .find(|plugin| plugin.name == plugin_name);

    let Some(plugin) = plugin else {
        return Err(MarketplaceError::PluginNotFound {
            plugin_name: plugin_name.to_string(),
            marketplace_name,
        });
    };

    let MarketplacePlugin {
        name,
        source,
        install_policy,
        auth_policy,
        ..
    } = plugin;
    if install_policy == MarketplacePluginInstallPolicy::NotAvailable {
        return Err(MarketplaceError::PluginNotAvailable {
            plugin_name: name,
            marketplace_name,
        });
    }

    let plugin_id = PluginId::new(name, marketplace_name).map_err(|err| match err {
        PluginIdError::Invalid(message) => MarketplaceError::InvalidPlugin(message),
    })?;
    Ok(ResolvedMarketplacePlugin {
        plugin_id,
        source_path: resolve_plugin_source_path(marketplace_path, source)?,
        auth_policy,
    })
}

pub fn list_marketplaces(
    additional_roots: &[AbsolutePathBuf],
) -> Result<Vec<MarketplaceSummary>, MarketplaceError> {
    list_marketplaces_with_home(additional_roots, home_dir().as_deref())
}

pub(crate) fn load_marketplace_summary(
    path: &AbsolutePathBuf,
) -> Result<MarketplaceSummary, MarketplaceError> {
    let marketplace = load_marketplace(path)?;
    let mut plugins = Vec::new();

    for plugin in marketplace.plugins {
        let MarketplacePlugin {
            name,
            source,
            install_policy,
            auth_policy,
            category,
        } = plugin;
        let source_path = resolve_plugin_source_path(path, source)?;
        let source = MarketplacePluginSourceSummary::Local {
            path: source_path.clone(),
        };
        let mut interface = load_plugin_manifest(source_path.as_path())
            .and_then(|manifest| plugin_manifest_interface(&manifest, source_path.as_path()));
        if let Some(category) = category {
            // Marketplace taxonomy wins when both sources provide a category.
            interface
                .get_or_insert_with(PluginManifestInterfaceSummary::default)
                .category = Some(category);
        }

        plugins.push(MarketplacePluginSummary {
            name,
            source,
            install_policy,
            auth_policy,
            interface,
        });
    }

    Ok(MarketplaceSummary {
        name: marketplace.name,
        path: path.clone(),
        plugins,
    })
}

fn list_marketplaces_with_home(
    additional_roots: &[AbsolutePathBuf],
    home_dir: Option<&Path>,
) -> Result<Vec<MarketplaceSummary>, MarketplaceError> {
    let mut marketplaces = Vec::new();

    for marketplace_path in discover_marketplace_paths_from_roots(additional_roots, home_dir) {
        marketplaces.push(load_marketplace_summary(&marketplace_path)?);
    }

    Ok(marketplaces)
}

fn discover_marketplace_paths_from_roots(
    additional_roots: &[AbsolutePathBuf],
    home_dir: Option<&Path>,
) -> Vec<AbsolutePathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = home_dir {
        let path = home.join(MARKETPLACE_RELATIVE_PATH);
        if path.is_file()
            && let Ok(path) = AbsolutePathBuf::try_from(path)
        {
            paths.push(path);
        }
    }

    for root in additional_roots {
        // Curated marketplaces can now come from an HTTP-downloaded directory that is not a git
        // checkout, so check the root directly before falling back to repo-root discovery.
        if let Ok(path) = root.join(MARKETPLACE_RELATIVE_PATH)
            && path.as_path().is_file()
            && !paths.contains(&path)
        {
            paths.push(path);
            continue;
        }
        if let Some(repo_root) = get_git_repo_root(root.as_path())
            && let Ok(repo_root) = AbsolutePathBuf::try_from(repo_root)
            && let Ok(path) = repo_root.join(MARKETPLACE_RELATIVE_PATH)
            && path.as_path().is_file()
            && !paths.contains(&path)
        {
            paths.push(path);
        }
    }

    paths
}

fn load_marketplace(path: &AbsolutePathBuf) -> Result<MarketplaceFile, MarketplaceError> {
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

fn resolve_plugin_source_path(
    marketplace_path: &AbsolutePathBuf,
    source: MarketplacePluginSource,
) -> Result<AbsolutePathBuf, MarketplaceError> {
    match source {
        MarketplacePluginSource::Local { path } => {
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
                    message: "local plugin source path must stay within the marketplace root"
                        .to_string(),
                });
            }

            // `marketplace.json` lives under `<root>/.agents/plugins/`, but local plugin paths
            // are resolved relative to `<root>`, not relative to the `plugins/` directory.
            marketplace_root_dir(marketplace_path)?
                .join(relative_source_path)
                .map_err(|err| MarketplaceError::InvalidMarketplaceFile {
                    path: marketplace_path.to_path_buf(),
                    message: format!("plugin source path must resolve to an absolute path: {err}"),
                })
        }
    }
}

fn marketplace_root_dir(
    marketplace_path: &AbsolutePathBuf,
) -> Result<AbsolutePathBuf, MarketplaceError> {
    let Some(plugins_dir) = marketplace_path.parent() else {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "marketplace file must live under `<root>/.agents/plugins/`".to_string(),
        });
    };
    let Some(dot_agents_dir) = plugins_dir.parent() else {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "marketplace file must live under `<root>/.agents/plugins/`".to_string(),
        });
    };
    let Some(marketplace_root) = dot_agents_dir.parent() else {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "marketplace file must live under `<root>/.agents/plugins/`".to_string(),
        });
    };

    if plugins_dir.as_path().file_name().and_then(|s| s.to_str()) != Some("plugins")
        || dot_agents_dir
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            != Some(".agents")
    {
        return Err(MarketplaceError::InvalidMarketplaceFile {
            path: marketplace_path.to_path_buf(),
            message: "marketplace file must live under `<root>/.agents/plugins/`".to_string(),
        });
    }

    Ok(marketplace_root)
}

#[derive(Debug, Deserialize)]
struct MarketplaceFile {
    name: String,
    plugins: Vec<MarketplacePlugin>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarketplacePlugin {
    name: String,
    source: MarketplacePluginSource,
    #[serde(default)]
    install_policy: MarketplacePluginInstallPolicy,
    #[serde(default)]
    auth_policy: MarketplacePluginAuthPolicy,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
enum MarketplacePluginSource {
    Local { path: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn resolve_marketplace_plugin_finds_repo_marketplace_plugin() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::create_dir_all(repo_root.join("nested")).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./plugin-1"
      }
    }
  ]
}"#,
        )
        .unwrap();

        let resolved = resolve_marketplace_plugin(
            &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
            "local-plugin",
        )
        .unwrap();

        assert_eq!(
            resolved,
            ResolvedMarketplacePlugin {
                plugin_id: PluginId::new("local-plugin".to_string(), "codex-curated".to_string())
                    .unwrap(),
                source_path: AbsolutePathBuf::try_from(repo_root.join("plugin-1")).unwrap(),
                auth_policy: MarketplacePluginAuthPolicy::OnInstall,
            }
        );
    }

    #[test]
    fn resolve_marketplace_plugin_reports_missing_plugin() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{"name":"codex-curated","plugins":[]}"#,
        )
        .unwrap();

        let err = resolve_marketplace_plugin(
            &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
            "missing",
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "plugin `missing` was not found in marketplace `codex-curated`"
        );
    }

    #[test]
    fn list_marketplaces_returns_home_and_repo_marketplaces() {
        let tmp = tempdir().unwrap();
        let home_root = tmp.path().join("home");
        let repo_root = tmp.path().join("repo");

        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(home_root.join(".agents/plugins")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::write(
            home_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "shared-plugin",
      "source": {
        "source": "local",
        "path": "./home-shared"
      }
    },
    {
      "name": "home-only",
      "source": {
        "source": "local",
        "path": "./home-only"
      }
    }
  ]
}"#,
        )
        .unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "shared-plugin",
      "source": {
        "source": "local",
        "path": "./repo-shared"
      }
    },
    {
      "name": "repo-only",
      "source": {
        "source": "local",
        "path": "./repo-only"
      }
    }
  ]
}"#,
        )
        .unwrap();

        let marketplaces = list_marketplaces_with_home(
            &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
            Some(&home_root),
        )
        .unwrap();

        assert_eq!(
            marketplaces,
            vec![
                MarketplaceSummary {
                    name: "codex-curated".to_string(),
                    path: AbsolutePathBuf::try_from(
                        home_root.join(".agents/plugins/marketplace.json"),
                    )
                    .unwrap(),
                    plugins: vec![
                        MarketplacePluginSummary {
                            name: "shared-plugin".to_string(),
                            source: MarketplacePluginSourceSummary::Local {
                                path: AbsolutePathBuf::try_from(home_root.join("home-shared"))
                                    .unwrap(),
                            },
                            install_policy: MarketplacePluginInstallPolicy::Available,
                            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
                            interface: None,
                        },
                        MarketplacePluginSummary {
                            name: "home-only".to_string(),
                            source: MarketplacePluginSourceSummary::Local {
                                path: AbsolutePathBuf::try_from(home_root.join("home-only"))
                                    .unwrap(),
                            },
                            install_policy: MarketplacePluginInstallPolicy::Available,
                            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
                            interface: None,
                        },
                    ],
                },
                MarketplaceSummary {
                    name: "codex-curated".to_string(),
                    path: AbsolutePathBuf::try_from(
                        repo_root.join(".agents/plugins/marketplace.json"),
                    )
                    .unwrap(),
                    plugins: vec![
                        MarketplacePluginSummary {
                            name: "shared-plugin".to_string(),
                            source: MarketplacePluginSourceSummary::Local {
                                path: AbsolutePathBuf::try_from(repo_root.join("repo-shared"))
                                    .unwrap(),
                            },
                            install_policy: MarketplacePluginInstallPolicy::Available,
                            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
                            interface: None,
                        },
                        MarketplacePluginSummary {
                            name: "repo-only".to_string(),
                            source: MarketplacePluginSourceSummary::Local {
                                path: AbsolutePathBuf::try_from(repo_root.join("repo-only"))
                                    .unwrap(),
                            },
                            install_policy: MarketplacePluginInstallPolicy::Available,
                            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
                            interface: None,
                        },
                    ],
                },
            ]
        );
    }

    #[test]
    fn list_marketplaces_keeps_distinct_entries_for_same_name() {
        let tmp = tempdir().unwrap();
        let home_root = tmp.path().join("home");
        let repo_root = tmp.path().join("repo");
        let home_marketplace = home_root.join(".agents/plugins/marketplace.json");
        let repo_marketplace = repo_root.join(".agents/plugins/marketplace.json");

        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(home_root.join(".agents/plugins")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();

        fs::write(
            home_marketplace.clone(),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./home-plugin"
      }
    }
  ]
}"#,
        )
        .unwrap();
        fs::write(
            repo_marketplace.clone(),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./repo-plugin"
      }
    }
  ]
}"#,
        )
        .unwrap();

        let marketplaces = list_marketplaces_with_home(
            &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
            Some(&home_root),
        )
        .unwrap();

        assert_eq!(
            marketplaces,
            vec![
                MarketplaceSummary {
                    name: "codex-curated".to_string(),
                    path: AbsolutePathBuf::try_from(home_marketplace).unwrap(),
                    plugins: vec![MarketplacePluginSummary {
                        name: "local-plugin".to_string(),
                        source: MarketplacePluginSourceSummary::Local {
                            path: AbsolutePathBuf::try_from(home_root.join("home-plugin")).unwrap(),
                        },
                        install_policy: MarketplacePluginInstallPolicy::Available,
                        auth_policy: MarketplacePluginAuthPolicy::OnInstall,
                        interface: None,
                    }],
                },
                MarketplaceSummary {
                    name: "codex-curated".to_string(),
                    path: AbsolutePathBuf::try_from(repo_marketplace.clone()).unwrap(),
                    plugins: vec![MarketplacePluginSummary {
                        name: "local-plugin".to_string(),
                        source: MarketplacePluginSourceSummary::Local {
                            path: AbsolutePathBuf::try_from(repo_root.join("repo-plugin")).unwrap(),
                        },
                        install_policy: MarketplacePluginInstallPolicy::Available,
                        auth_policy: MarketplacePluginAuthPolicy::OnInstall,
                        interface: None,
                    }],
                },
            ]
        );

        let resolved = resolve_marketplace_plugin(
            &AbsolutePathBuf::try_from(repo_marketplace).unwrap(),
            "local-plugin",
        )
        .unwrap();

        assert_eq!(
            resolved.source_path,
            AbsolutePathBuf::try_from(repo_root.join("repo-plugin")).unwrap()
        );
    }

    #[test]
    fn list_marketplaces_dedupes_multiple_roots_in_same_repo() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        let nested_root = repo_root.join("nested/project");

        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::create_dir_all(&nested_root).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./plugin"
      }
    }
  ]
}"#,
        )
        .unwrap();

        let marketplaces = list_marketplaces_with_home(
            &[
                AbsolutePathBuf::try_from(repo_root.clone()).unwrap(),
                AbsolutePathBuf::try_from(nested_root).unwrap(),
            ],
            None,
        )
        .unwrap();

        assert_eq!(
            marketplaces,
            vec![MarketplaceSummary {
                name: "codex-curated".to_string(),
                path: AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json"))
                    .unwrap(),
                plugins: vec![MarketplacePluginSummary {
                    name: "local-plugin".to_string(),
                    source: MarketplacePluginSourceSummary::Local {
                        path: AbsolutePathBuf::try_from(repo_root.join("plugin")).unwrap(),
                    },
                    install_policy: MarketplacePluginInstallPolicy::Available,
                    auth_policy: MarketplacePluginAuthPolicy::OnInstall,
                    interface: None,
                }],
            }]
        );
    }

    #[test]
    fn list_marketplaces_resolves_plugin_interface_paths_to_absolute() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        let plugin_root = repo_root.join("plugins/demo-plugin");
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "demo-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/demo-plugin"
      },
      "installPolicy": "AVAILABLE",
      "authPolicy": "ON_INSTALL",
      "category": "Design"
    }
  ]
}"#,
        )
        .unwrap();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{
  "name": "demo-plugin",
  "interface": {
    "displayName": "Demo",
    "category": "Productivity",
    "capabilities": ["Interactive", "Write"],
    "composerIcon": "./assets/icon.png",
    "logo": "./assets/logo.png",
    "screenshots": ["./assets/shot1.png"]
  }
}"#,
        )
        .unwrap();

        let marketplaces =
            list_marketplaces_with_home(&[AbsolutePathBuf::try_from(repo_root).unwrap()], None)
                .unwrap();

        assert_eq!(
            marketplaces[0].plugins[0].install_policy,
            MarketplacePluginInstallPolicy::Available
        );
        assert_eq!(
            marketplaces[0].plugins[0].auth_policy,
            MarketplacePluginAuthPolicy::OnInstall
        );
        assert_eq!(
            marketplaces[0].plugins[0].interface,
            Some(PluginManifestInterfaceSummary {
                display_name: Some("Demo".to_string()),
                short_description: None,
                long_description: None,
                developer_name: None,
                category: Some("Design".to_string()),
                capabilities: vec!["Interactive".to_string(), "Write".to_string()],
                website_url: None,
                privacy_policy_url: None,
                terms_of_service_url: None,
                default_prompt: None,
                brand_color: None,
                composer_icon: Some(
                    AbsolutePathBuf::try_from(plugin_root.join("assets/icon.png")).unwrap(),
                ),
                logo: Some(AbsolutePathBuf::try_from(plugin_root.join("assets/logo.png")).unwrap()),
                screenshots: vec![
                    AbsolutePathBuf::try_from(plugin_root.join("assets/shot1.png")).unwrap(),
                ],
            })
        );
    }

    #[test]
    fn list_marketplaces_ignores_plugin_interface_assets_without_dot_slash() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        let plugin_root = repo_root.join("plugins/demo-plugin");

        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "demo-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/demo-plugin"
      }
    }
  ]
}"#,
        )
        .unwrap();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{
  "name": "demo-plugin",
  "interface": {
    "displayName": "Demo",
    "capabilities": ["Interactive"],
    "composerIcon": "assets/icon.png",
    "logo": "/tmp/logo.png",
    "screenshots": ["assets/shot1.png"]
  }
}"#,
        )
        .unwrap();

        let marketplaces =
            list_marketplaces_with_home(&[AbsolutePathBuf::try_from(repo_root).unwrap()], None)
                .unwrap();

        assert_eq!(
            marketplaces[0].plugins[0].interface,
            Some(PluginManifestInterfaceSummary {
                display_name: Some("Demo".to_string()),
                short_description: None,
                long_description: None,
                developer_name: None,
                category: None,
                capabilities: vec!["Interactive".to_string()],
                website_url: None,
                privacy_policy_url: None,
                terms_of_service_url: None,
                default_prompt: None,
                brand_color: None,
                composer_icon: None,
                logo: None,
                screenshots: Vec::new(),
            })
        );
        assert_eq!(
            marketplaces[0].plugins[0].install_policy,
            MarketplacePluginInstallPolicy::Available
        );
        assert_eq!(
            marketplaces[0].plugins[0].auth_policy,
            MarketplacePluginAuthPolicy::OnInstall
        );
    }

    #[test]
    fn resolve_marketplace_plugin_rejects_non_relative_local_paths() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "../plugin-1"
      }
    }
  ]
}"#,
        )
        .unwrap();

        let err = resolve_marketplace_plugin(
            &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
            "local-plugin",
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            format!(
                "invalid marketplace file `{}`: local plugin source path must start with `./`",
                repo_root.join(".agents/plugins/marketplace.json").display()
            )
        );
    }

    #[test]
    fn resolve_marketplace_plugin_uses_first_duplicate_entry() {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./first"
      }
    },
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./second"
      }
    }
  ]
}"#,
        )
        .unwrap();

        let resolved = resolve_marketplace_plugin(
            &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
            "local-plugin",
        )
        .unwrap();

        assert_eq!(
            resolved.source_path,
            AbsolutePathBuf::try_from(repo_root.join("first")).unwrap()
        );
    }
}
