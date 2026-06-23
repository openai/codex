use crate::manifest::PluginManifest;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use std::error::Error as StdError;
use std::future::Future;
use thiserror::Error;

/// A plugin resource paired with the environment that owns its filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginResourceLocator {
    Environment {
        /// Environment whose filesystem owns the resource.
        environment_id: String,
        /// Resource URI within that filesystem.
        path: PathUri,
    },
}

/// Authority-bound location of a resolved plugin package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedPluginLocation {
    Environment {
        /// Environment whose filesystem owns the package.
        environment_id: String,
        /// Package root URI within that filesystem.
        root: PathUri,
    },
}

/// An inert plugin descriptor whose resources retain their source authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPlugin {
    selected_root_id: String,
    location: ResolvedPluginLocation,
    manifest_path: PluginResourceLocator,
    manifest: PluginManifest<PluginResourceLocator>,
}

/// Failure to construct a resolved plugin with internally consistent resources.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ResolvedPluginError {
    #[error("plugin resource path `{path}` is outside package root `{root}`")]
    ResourceOutsideRoot { root: PathUri, path: PathUri },
}

impl ResolvedPlugin {
    /// Creates an environment-owned descriptor from a validated plugin manifest.
    pub fn from_environment(
        selected_root_id: String,
        environment_id: String,
        root: PathUri,
        manifest_path: PathUri,
        manifest: PluginManifest<PathUri>,
    ) -> Result<Self, ResolvedPluginError> {
        let manifest_path = environment_resource(&environment_id, &root, manifest_path)?;
        let manifest = manifest
            .try_map_resources(|path| environment_resource(&environment_id, &root, path))?;
        Ok(Self {
            selected_root_id,
            location: ResolvedPluginLocation::Environment {
                environment_id,
                root,
            },
            manifest_path,
            manifest,
        })
    }

    /// Returns the opaque ID supplied for the selected capability root.
    pub fn selected_root_id(&self) -> &str {
        &self.selected_root_id
    }

    /// Returns the authority-bound package location.
    pub fn location(&self) -> &ResolvedPluginLocation {
        &self.location
    }

    /// Returns the manifest resource used to resolve this package.
    pub fn manifest_path(&self) -> &PluginResourceLocator {
        &self.manifest_path
    }

    /// Returns package metadata whose resource fields retain their source authority.
    pub fn manifest(&self) -> &PluginManifest<PluginResourceLocator> {
        &self.manifest
    }
}

fn environment_resource(
    environment_id: &str,
    root: &PathUri,
    path: PathUri,
) -> Result<PluginResourceLocator, ResolvedPluginError> {
    if !path.starts_with(root) {
        return Err(ResolvedPluginError::ResourceOutsideRoot {
            root: root.clone(),
            path,
        });
    }
    Ok(PluginResourceLocator::Environment {
        environment_id: environment_id.to_string(),
        path,
    })
}

/// Resolves source-owned package roots into inert plugin descriptors.
///
/// Implementations must perform all filesystem access through the authority
/// named by the selected root. `None` means the root contains no plugin
/// manifest and may be handled as another standalone capability.
pub trait PluginProvider: Send + Sync {
    /// Source-specific resolution failure.
    type Error: StdError + Send + Sync + 'static;

    /// Resolves one selected root without activating any of its components.
    fn resolve(
        &self,
        root: &SelectedCapabilityRoot,
    ) -> impl Future<Output = Result<Option<ResolvedPlugin>, Self::Error>> + Send;
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
