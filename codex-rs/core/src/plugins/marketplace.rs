use super::store::PluginId;
use super::store::PluginIdError;
use crate::git_info::get_git_repo_root;
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
}

#[derive(Debug, thiserror::Error)]
pub enum MarketplaceError {
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("invalid marketplace file `{path}`: {message}")]
    InvalidMarketplaceFile { path: PathBuf, message: String },

    #[error("plugin `{plugin_name}` was not found in marketplace `{marketplace_name}`")]
    PluginNotFound {
        plugin_name: String,
        marketplace_name: String,
    },

    #[error(
        "multiple marketplace plugin entries matched `{plugin_name}` in marketplace `{marketplace_name}`"
    )]
    DuplicatePlugin {
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

// For now, marketplace discovery always reads from disk so installs see the latest
// marketplace.json contents without any in-memory cache invalidation.
pub fn resolve_marketplace_plugin(
    cwd: &Path,
    plugin_name: &str,
    marketplace_name: &str,
) -> Result<ResolvedMarketplacePlugin, MarketplaceError> {
    resolve_marketplace_plugin_from_paths(
        &discover_marketplace_paths(cwd),
        plugin_name,
        marketplace_name,
    )
}

fn resolve_marketplace_plugin_from_paths(
    marketplace_paths: &[PathBuf],
    plugin_name: &str,
    marketplace_name: &str,
) -> Result<ResolvedMarketplacePlugin, MarketplaceError> {
    for marketplace_path in marketplace_paths {
        let marketplace = load_marketplace(marketplace_path)?;
        let discovered_marketplace_name = marketplace.name;
        let mut matches = marketplace
            .plugins
            .into_iter()
            .filter(|plugin| plugin.name == plugin_name)
            .collect::<Vec<_>>();

        if discovered_marketplace_name != marketplace_name || matches.is_empty() {
            continue;
        }

        if matches.len() > 1 {
            return Err(MarketplaceError::DuplicatePlugin {
                plugin_name: plugin_name.to_string(),
                marketplace_name: marketplace_name.to_string(),
            });
        }

        if let Some(plugin) = matches.pop() {
            let plugin_id = PluginId::new(plugin.name, marketplace_name.to_string()).map_err(
                |err| match err {
                    PluginIdError::Invalid(message) => MarketplaceError::InvalidPlugin(message),
                },
            )?;
            return Ok(ResolvedMarketplacePlugin {
                plugin_id,
                source_path: resolve_plugin_source_path(marketplace_path, plugin.source)?,
            });
        }
    }

    Err(MarketplaceError::PluginNotFound {
        plugin_name: plugin_name.to_string(),
        marketplace_name: marketplace_name.to_string(),
    })
}

fn discover_marketplace_paths(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(repo_root) = get_git_repo_root(cwd) {
        let path = repo_root.join(MARKETPLACE_RELATIVE_PATH);
        if path.is_file() {
            paths.push(path);
        }
    }

    if let Some(home) = home_dir() {
        let path = home.join(MARKETPLACE_RELATIVE_PATH);
        if path.is_file() {
            paths.push(path);
        }
    }

    paths
}

fn load_marketplace(path: &Path) -> Result<MarketplaceFile, MarketplaceError> {
    let contents = fs::read_to_string(path)
        .map_err(|err| MarketplaceError::io("failed to read marketplace file", err))?;
    serde_json::from_str(&contents).map_err(|err| MarketplaceError::InvalidMarketplaceFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

fn resolve_plugin_source_path(
    marketplace_path: &Path,
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
                    message: "local plugin source path must stay within the marketplace directory"
                        .to_string(),
                });
            }

            let source_path = marketplace_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(relative_source_path);
            AbsolutePathBuf::try_from(source_path).map_err(|err| {
                MarketplaceError::InvalidMarketplaceFile {
                    path: marketplace_path.to_path_buf(),
                    message: format!("plugin source path must resolve to an absolute path: {err}"),
                }
            })
        }
    }
}

#[derive(Debug, Deserialize)]
struct MarketplaceFile {
    name: String,
    plugins: Vec<MarketplacePlugin>,
}

#[derive(Debug, Deserialize)]
struct MarketplacePlugin {
    name: String,
    source: MarketplacePluginSource,
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

        let resolved =
            resolve_marketplace_plugin(&repo_root.join("nested"), "local-plugin", "codex-curated")
                .unwrap();

        assert_eq!(
            resolved,
            ResolvedMarketplacePlugin {
                plugin_id: PluginId::new("local-plugin".to_string(), "codex-curated".to_string())
                    .unwrap(),
                source_path: AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/plugin-1"))
                    .unwrap(),
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

        let err = resolve_marketplace_plugin(&repo_root, "missing", "codex-curated").unwrap_err();

        assert_eq!(
            err.to_string(),
            "plugin `missing` was not found in marketplace `codex-curated`"
        );
    }

    #[test]
    fn resolve_marketplace_plugin_prefers_repo_over_home_for_same_plugin() {
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

        let resolved = resolve_marketplace_plugin_from_paths(
            &[repo_marketplace, home_marketplace],
            "local-plugin",
            "codex-curated",
        )
        .unwrap();

        assert_eq!(
            resolved,
            ResolvedMarketplacePlugin {
                plugin_id: PluginId::new("local-plugin".to_string(), "codex-curated".to_string())
                    .unwrap(),
                source_path: AbsolutePathBuf::try_from(
                    repo_root.join(".agents/plugins/repo-plugin"),
                )
                .unwrap(),
            }
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

        let err =
            resolve_marketplace_plugin(&repo_root, "local-plugin", "codex-curated").unwrap_err();

        assert_eq!(
            err.to_string(),
            format!(
                "invalid marketplace file `{}`: local plugin source path must start with `./`",
                repo_root.join(".agents/plugins/marketplace.json").display()
            )
        );
    }
}
