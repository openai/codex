use crate::manifest::parse_plugin_manifest_uri;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_plugin::PluginProvider;
use codex_plugin::ResolvedPlugin;
use codex_plugin::ResolvedPluginError;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use codex_utils_path_uri::PathUriParseError;
use codex_utils_plugins::DISCOVERABLE_PLUGIN_MANIFEST_PATHS;
use std::io;
use std::sync::Arc;
use thiserror::Error;

/// Failure to resolve an environment-owned capability root as a plugin package.
#[derive(Debug, Error)]
pub enum ExecutorPluginProviderError {
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
        path: PathUri,
        #[source]
        source: io::Error,
    },
    #[error("selected capability root `{root_id}` path {path} is not a directory")]
    RootNotDirectory { root_id: String, path: PathUri },
    #[error(
        "failed to resolve plugin manifest path `{relative_path}` below selected capability root `{root_id}` at {root}: {source}"
    )]
    InvalidManifestPath {
        root_id: String,
        root: PathUri,
        relative_path: &'static str,
        #[source]
        source: PathUriParseError,
    },
    #[error("failed to inspect plugin manifest for `{root_id}` at {path}: {source}")]
    InspectManifest {
        root_id: String,
        path: PathUri,
        #[source]
        source: io::Error,
    },
    #[error("failed to read plugin manifest for `{root_id}` at {path}: {source}")]
    ReadManifest {
        root_id: String,
        path: PathUri,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse plugin manifest for `{root_id}` at {path}: {source}")]
    ParseManifest {
        root_id: String,
        path: PathUri,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to construct plugin descriptor for `{root_id}`: {source}")]
    ConstructDescriptor {
        root_id: String,
        #[source]
        source: ResolvedPluginError,
    },
}

/// Resolves plugin packages through the filesystem owned by an execution environment.
#[derive(Clone, Debug)]
pub struct ExecutorPluginProvider {
    environment_manager: Arc<EnvironmentManager>,
}

/// A resolved plugin paired with the concrete filesystem used to read it.
#[derive(Clone)]
pub struct ResolvedExecutorPlugin {
    plugin: ResolvedPlugin,
    file_system: Arc<dyn ExecutorFileSystem>,
}

impl ResolvedExecutorPlugin {
    /// Returns the source-neutral plugin descriptor.
    pub fn plugin(&self) -> &ResolvedPlugin {
        &self.plugin
    }

    /// Returns the concrete filesystem that resolved the descriptor.
    pub fn file_system(&self) -> &dyn ExecutorFileSystem {
        self.file_system.as_ref()
    }
}

impl ExecutorPluginProvider {
    /// Creates a provider backed by the active execution environments.
    pub fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            environment_manager,
        }
    }

    /// Resolves a plugin and retains the exact filesystem used for package access.
    pub async fn resolve_bound(
        &self,
        selected_root: &SelectedCapabilityRoot,
    ) -> Result<Option<ResolvedExecutorPlugin>, ExecutorPluginProviderError> {
        let root_id = &selected_root.id;
        let plugin_root = selected_plugin_root(selected_root);
        let CapabilityRootLocation::Environment { environment_id, .. } = &selected_root.location;
        let environment = self
            .environment_manager
            .get_environment(environment_id)
            .ok_or_else(|| ExecutorPluginProviderError::UnavailableEnvironment {
                root_id: root_id.clone(),
                environment_id: environment_id.clone(),
            })?;
        let file_system = environment.get_filesystem();
        let plugin = resolve_plugin_root(selected_root, plugin_root, file_system.as_ref()).await?;

        Ok(plugin.map(|plugin| ResolvedExecutorPlugin {
            plugin,
            file_system,
        }))
    }

    /// Resolves a plugin through the exact executor captured for one model step.
    pub async fn resolve_pinned(
        resolved_root: &ResolvedSelectedCapabilityRoot,
    ) -> Result<Option<ResolvedExecutorPlugin>, ExecutorPluginProviderError> {
        let selected_root = resolved_root.selected_root();
        let plugin_root = selected_plugin_root(selected_root);
        let file_system = resolved_root.file_system();
        let plugin = resolve_plugin_root(selected_root, plugin_root, file_system.as_ref()).await?;
        Ok(plugin.map(|plugin| ResolvedExecutorPlugin {
            plugin,
            file_system,
        }))
    }
}

impl PluginProvider for ExecutorPluginProvider {
    type Error = ExecutorPluginProviderError;

    async fn resolve(
        &self,
        selected_root: &SelectedCapabilityRoot,
    ) -> Result<Option<ResolvedPlugin>, Self::Error> {
        self.resolve_bound(selected_root)
            .await
            .map(|plugin| plugin.map(|plugin| plugin.plugin))
    }
}

fn selected_plugin_root(selected_root: &SelectedCapabilityRoot) -> PathUri {
    let CapabilityRootLocation::Environment { path, .. } = &selected_root.location;
    path.clone()
}

async fn resolve_plugin_root(
    selected_root: &SelectedCapabilityRoot,
    plugin_root: PathUri,
    file_system: &dyn ExecutorFileSystem,
) -> Result<Option<ResolvedPlugin>, ExecutorPluginProviderError> {
    let root_id = &selected_root.id;
    let CapabilityRootLocation::Environment { environment_id, .. } = &selected_root.location;
    let root_metadata = file_system
        .get_metadata(&plugin_root, /*sandbox*/ None)
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
        let candidate_uri = plugin_root.join(relative_path).map_err(|source| {
            ExecutorPluginProviderError::InvalidManifestPath {
                root_id: root_id.clone(),
                root: plugin_root.clone(),
                relative_path,
                source,
            }
        })?;
        match file_system
            .get_metadata(&candidate_uri, /*sandbox*/ None)
            .await
        {
            Ok(metadata) if metadata.is_file => {
                manifest_path = Some(candidate_uri);
                break;
            }
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ExecutorPluginProviderError::InspectManifest {
                    root_id: root_id.clone(),
                    path: candidate_uri,
                    source,
                });
            }
        }
    }
    let Some(manifest_uri) = manifest_path else {
        return Ok(None);
    };
    let contents = file_system
        .read_file_text(&manifest_uri, /*sandbox*/ None)
        .await
        .map_err(|source| ExecutorPluginProviderError::ReadManifest {
            root_id: root_id.clone(),
            path: manifest_uri.clone(),
            source,
        })?;
    let manifest =
        parse_plugin_manifest_uri(&plugin_root, &manifest_uri, &contents).map_err(|source| {
            ExecutorPluginProviderError::ParseManifest {
                root_id: root_id.clone(),
                path: manifest_uri.clone(),
                source,
            }
        })?;

    let plugin = ResolvedPlugin::from_environment(
        root_id.clone(),
        environment_id.clone(),
        plugin_root,
        manifest_uri,
        manifest,
    )
    .map_err(|source| ExecutorPluginProviderError::ConstructDescriptor {
        root_id: root_id.clone(),
        source,
    })?;

    Ok(Some(plugin))
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
