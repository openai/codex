//! Resolves selected plugin roots through their owning executor's filesystem.
//! Resource locations retain that environment's authority.

use crate::PluginProvider;
use crate::manifest::parse_plugin_manifest_uri;
use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::ReadFileOptions;
use codex_plugin::ResolvedPlugin;
use codex_plugin::ResolvedPluginError;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use codex_utils_path_uri::PathUriParseError;
use codex_utils_plugins::DISCOVERABLE_PLUGIN_MANIFEST_PATHS;
use futures::StreamExt;
use std::io;
use std::sync::Arc;
use thiserror::Error;

const MAX_CONCURRENT_MANIFEST_PROBES: usize = 256;

/// Failure to resolve an environment-owned capability root as a plugin package.
#[derive(Debug, Error)]
pub enum ExecutorPluginProviderError {
    #[error("discovery failed for selected capability root `{root_id}`")]
    DiscoveryFailed { root_id: String },
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

/// Ownership of a selected root, independently of whether its manifest can be parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginRootOwnership {
    Plugin,
    Standalone,
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

    /// Classifies ownership before capability parsing. Failed discovery or metadata
    /// access returns an error, not evidence that a root is standalone.
    pub async fn root_ownership(
        &self,
        selected_root: &SelectedCapabilityRoot,
        discovery: Option<&CapabilityRootDiscovery>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> Result<PluginRootOwnership, ExecutorPluginProviderError> {
        let CapabilityRootLocation::Environment {
            environment_id,
            path,
        } = &selected_root.location;
        if let Some(discovery) = discovery {
            if discovery.error.is_some() {
                return Err(ExecutorPluginProviderError::DiscoveryFailed {
                    root_id: selected_root.id.clone(),
                });
            }
            if discovery.plugin.is_some()
                || discovery.namespace_manifests.iter().any(|manifest| {
                    manifest
                        .path
                        .parent()
                        .and_then(|directory| directory.parent())
                        .is_some_and(|plugin_root| path.starts_with(&plugin_root))
                })
            {
                return Ok(PluginRootOwnership::Plugin);
            }
            if discovery.warnings.is_empty() {
                return Ok(PluginRootOwnership::Standalone);
            }
        }
        let environment = self
            .environment_manager
            .get_environment(environment_id)
            .ok_or_else(|| ExecutorPluginProviderError::UnavailableEnvironment {
                root_id: selected_root.id.clone(),
                environment_id: environment_id.clone(),
            })?;
        let file_system = environment.get_filesystem();
        let manifest = find_manifest(
            selected_root,
            std::iter::successors(Some(path.clone()), PathUri::parent),
            file_system.as_ref(),
            sandbox,
        )
        .await?;
        Ok(if manifest.is_some() {
            PluginRootOwnership::Plugin
        } else {
            PluginRootOwnership::Standalone
        })
    }

    /// Resolves a plugin and retains the exact filesystem used for package access.
    #[tracing::instrument(name = "plugins.executor.package.resolve", skip_all)]
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
}

impl PluginProvider for ExecutorPluginProvider {}

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
        .get_metadata(
            &plugin_root,
            GetMetadataOptions::default(),
            /*sandbox*/ None,
        )
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

    let Some(manifest_uri) = find_manifest(
        selected_root,
        std::iter::once(plugin_root.clone()),
        file_system,
        /*sandbox*/ None,
    )
    .await?
    else {
        return Ok(None);
    };
    let contents = file_system
        .read_file_text(
            &manifest_uri,
            ReadFileOptions::default(),
            /*sandbox*/ None,
        )
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

async fn find_manifest(
    selected_root: &SelectedCapabilityRoot,
    mut plugin_roots: impl Iterator<Item = PathUri> + Send,
    file_system: &dyn ExecutorFileSystem,
    sandbox: Option<&FileSystemSandboxContext>,
) -> Result<Option<PathUri>, ExecutorPluginProviderError> {
    let root_id = &selected_root.id;
    let mut plugin_root = plugin_roots.next();
    let mut marker_index = 0;
    let probes = std::iter::from_fn(move || {
        let root = plugin_root.clone()?;
        let relative_path = *DISCOVERABLE_PLUGIN_MANIFEST_PATHS.get(marker_index)?;
        let candidate_uri = root.join(relative_path).map_err(|source| {
            ExecutorPluginProviderError::InvalidManifestPath {
                root_id: root_id.clone(),
                root,
                relative_path,
                source,
            }
        });
        marker_index += 1;
        if marker_index == DISCOVERABLE_PLUGIN_MANIFEST_PATHS.len() {
            marker_index = 0;
            plugin_root = plugin_roots.next();
        }
        Some(candidate_uri)
    });
    // Pipeline remote metadata calls while consuming results in ancestor/manifest priority order.
    let mut results = futures::stream::iter(probes)
        .map(|candidate_uri| async move {
            let candidate_uri = candidate_uri?;
            match file_system
                .get_metadata(&candidate_uri, GetMetadataOptions::default(), sandbox)
                .await
            {
                Ok(metadata) if metadata.is_file => Ok(Some(candidate_uri)),
                Ok(_) => Ok(None),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(source) => Err(ExecutorPluginProviderError::InspectManifest {
                    root_id: root_id.clone(),
                    path: candidate_uri,
                    source,
                }),
            }
        })
        .buffered(MAX_CONCURRENT_MANIFEST_PROBES);
    while let Some(result) = results.next().await {
        if let Some(manifest) = result? {
            return Ok(Some(manifest));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "executor_provider_tests.rs"]
mod tests;
