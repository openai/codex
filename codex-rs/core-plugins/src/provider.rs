use crate::manifest::parse_plugin_manifest;
use codex_exec_server::EnvironmentManager;
use codex_plugin::PluginProvider;
use codex_plugin::ResolvedPlugin;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::DISCOVERABLE_PLUGIN_MANIFEST_PATHS;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

/// Failure to resolve an environment-owned capability root as a plugin package.
#[derive(Debug, Error)]
pub enum ExecutorPluginProviderError {
    #[error("selected capability root `{root_id}` has invalid path `{path}`: {message}")]
    InvalidRootPath {
        root_id: String,
        path: String,
        message: String,
    },
    #[error(
        "selected capability root `{root_id}` references unavailable environment `{environment_id}`"
    )]
    UnavailableEnvironment {
        root_id: String,
        environment_id: String,
    },
    #[error("failed to inspect selected capability root `{root_id}` at {path}: {source}")]
    InspectRoot {
        root_id: String,
        path: AbsolutePathBuf,
        #[source]
        source: io::Error,
    },
    #[error("selected capability root `{root_id}` path {path} is not a directory")]
    RootNotDirectory {
        root_id: String,
        path: AbsolutePathBuf,
    },
    #[error("failed to inspect plugin manifest for `{root_id}` at {path}: {source}")]
    InspectManifest {
        root_id: String,
        path: AbsolutePathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read plugin manifest for `{root_id}` at {path}: {source}")]
    ReadManifest {
        root_id: String,
        path: AbsolutePathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse plugin manifest for `{root_id}` at {path}: {source}")]
    ParseManifest {
        root_id: String,
        path: AbsolutePathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// Resolves plugin packages through the filesystem owned by an execution environment.
#[derive(Clone, Debug)]
pub struct ExecutorPluginProvider {
    environment_manager: Arc<EnvironmentManager>,
}

impl ExecutorPluginProvider {
    /// Creates a provider backed by the active execution environments.
    pub fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            environment_manager,
        }
    }
}

impl PluginProvider for ExecutorPluginProvider {
    type Error = ExecutorPluginProviderError;

    async fn resolve(
        &self,
        selected_root: &SelectedCapabilityRoot,
    ) -> Result<Option<ResolvedPlugin>, Self::Error> {
        let root_id = &selected_root.id;
        let CapabilityRootLocation::Environment {
            environment_id,
            path,
        } = &selected_root.location;
        let plugin_root = PathBuf::from(path);
        if !plugin_root.is_absolute() {
            return Err(ExecutorPluginProviderError::InvalidRootPath {
                root_id: root_id.clone(),
                path: path.clone(),
                message: "executor path must be absolute".to_string(),
            });
        }
        let plugin_root =
            AbsolutePathBuf::from_absolute_path_checked(plugin_root).map_err(|err| {
                ExecutorPluginProviderError::InvalidRootPath {
                    root_id: root_id.clone(),
                    path: path.clone(),
                    message: err.to_string(),
                }
            })?;
        let environment = self
            .environment_manager
            .get_environment(environment_id)
            .ok_or_else(|| ExecutorPluginProviderError::UnavailableEnvironment {
                root_id: root_id.clone(),
                environment_id: environment_id.clone(),
            })?;
        let file_system = environment.get_filesystem();
        let root_uri = PathUri::from_abs_path(&plugin_root).map_err(|err| {
            ExecutorPluginProviderError::InvalidRootPath {
                root_id: root_id.clone(),
                path: path.clone(),
                message: err.to_string(),
            }
        })?;
        let root_metadata = file_system
            .get_metadata(&root_uri, /*sandbox*/ None)
            .await
            .map_err(|source| ExecutorPluginProviderError::InspectRoot {
                root_id: root_id.clone(),
                path: plugin_root.clone(),
                source,
            })?;
        if !root_metadata.is_directory {
            return Err(ExecutorPluginProviderError::RootNotDirectory {
                root_id: root_id.clone(),
                path: plugin_root,
            });
        }

        let mut manifest_path = None;
        for relative_path in DISCOVERABLE_PLUGIN_MANIFEST_PATHS {
            let candidate = plugin_root.join(relative_path);
            let candidate_uri = PathUri::from_abs_path(&candidate).map_err(|err| {
                ExecutorPluginProviderError::InvalidRootPath {
                    root_id: root_id.clone(),
                    path: candidate.as_path().to_string_lossy().into_owned(),
                    message: err.to_string(),
                }
            })?;
            match file_system
                .get_metadata(&candidate_uri, /*sandbox*/ None)
                .await
            {
                Ok(metadata) if metadata.is_file => {
                    manifest_path = Some((candidate, candidate_uri));
                    break;
                }
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(ExecutorPluginProviderError::InspectManifest {
                        root_id: root_id.clone(),
                        path: candidate,
                        source,
                    });
                }
            }
        }
        let Some((manifest_path, manifest_uri)) = manifest_path else {
            return Ok(None);
        };
        let contents = file_system
            .read_file_text(&manifest_uri, /*sandbox*/ None)
            .await
            .map_err(|source| ExecutorPluginProviderError::ReadManifest {
                root_id: root_id.clone(),
                path: manifest_path.clone(),
                source,
            })?;
        let manifest =
            parse_plugin_manifest(&plugin_root, &manifest_path, &contents).map_err(|source| {
                ExecutorPluginProviderError::ParseManifest {
                    root_id: root_id.clone(),
                    path: manifest_path.clone(),
                    source,
                }
            })?;

        Ok(Some(ResolvedPlugin::from_environment(
            root_id.clone(),
            environment_id.clone(),
            plugin_root,
            manifest_path,
            manifest,
        )))
    }
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
