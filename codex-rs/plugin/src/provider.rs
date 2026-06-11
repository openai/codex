use crate::manifest::PluginManifest;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::error::Error;
use std::future::Future;

/// Authority-bound location of a resolved plugin package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedPluginLocation {
    Environment {
        /// Environment whose filesystem owns the package.
        environment_id: String,
        /// Absolute package root within that filesystem.
        root: AbsolutePathBuf,
    },
}

/// A plugin package descriptor resolved from one source without activating its components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPlugin {
    selected_root_id: String,
    location: ResolvedPluginLocation,
    manifest_path: AbsolutePathBuf,
    manifest: PluginManifest,
}

impl ResolvedPlugin {
    /// Creates an environment-owned descriptor from a validated plugin manifest.
    pub fn from_environment(
        selected_root_id: String,
        environment_id: String,
        root: AbsolutePathBuf,
        manifest_path: AbsolutePathBuf,
        manifest: PluginManifest,
    ) -> Self {
        debug_assert!(manifest_path.as_path().starts_with(root.as_path()));
        Self {
            selected_root_id,
            location: ResolvedPluginLocation::Environment {
                environment_id,
                root,
            },
            manifest_path,
            manifest,
        }
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
    pub fn manifest_path(&self) -> &AbsolutePathBuf {
        &self.manifest_path
    }

    /// Returns the parsed package metadata and component locators.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

/// Resolves source-owned package roots into inert plugin descriptors.
///
/// Implementations must perform all filesystem access through the authority
/// named by the selected root. `None` means the root contains no plugin
/// manifest and may be handled as another standalone capability.
pub trait PluginProvider: Send + Sync {
    /// Source-specific resolution failure.
    type Error: Error + Send + Sync + 'static;

    /// Resolves one selected root without activating any of its components.
    fn resolve(
        &self,
        root: &SelectedCapabilityRoot,
    ) -> impl Future<Output = Result<Option<ResolvedPlugin>, Self::Error>> + Send;
}
