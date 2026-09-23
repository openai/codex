//! Executor capability inventory; callers apply identity and enablement.

use codex_file_system::FileSystemSandboxContext;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;

use crate::CapabilityTextFile;
use crate::DiscoveredPluginFiles;

pub const CAPABILITIES_DISCOVER_V2_METHOD: &str = "capabilities/discoverV2";

/// Discovers installed capabilities on the executor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverV2CapabilitiesRequest {
    pub cwd: PathUri,
    #[serde(default)]
    pub sandbox: Option<FileSystemSandboxContext>,
}

/// Discovered metadata before caller policy is applied.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverV2CapabilitiesResponse {
    pub plugins: Vec<ExecutorPlugin>,
    /// Skills outside installed packages.
    pub skills: Vec<ExecutorSkill>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutorPlugin {
    /// Environment-local ID: `<name>@<marketplace>`.
    pub id: String,
    pub remote_plugin_id: Option<String>,
    pub version: String,
    pub root: PathUri,
    pub files: DiscoveredPluginFiles,
    pub skills: Vec<ExecutorSkill>,
}

/// Skill metadata, excluding instructions and resources.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutorSkill {
    pub name: String,
    /// Nearest enclosing legacy plugin's name, following V1 precedence.
    #[serde(default)]
    pub namespace: Option<String>,
    pub description: String,
    #[serde(default)]
    pub short_description: Option<String>,
    /// Path to SKILL.md, not its containing directory.
    pub path: PathUri,
    pub metadata: Option<CapabilityTextFile>,
}

impl fmt::Debug for ExecutorPlugin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecutorPlugin")
            .field("id", &self.id)
            .field("remote_plugin_id", &self.remote_plugin_id)
            .field("version", &self.version)
            .field("root", &self.root)
            .field("manifest_path", &self.files.manifest.path)
            .field(
                "mcp_config_path",
                &self.files.mcp_config.as_ref().map(|file| &file.path),
            )
            .field(
                "apps_config_path",
                &self.files.apps_config.as_ref().map(|file| &file.path),
            )
            .field("skills", &self.skills)
            .finish()
    }
}

impl fmt::Debug for ExecutorSkill {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Omit potentially sensitive descriptions and file contents.
        formatter
            .debug_struct("ExecutorSkill")
            .field("name", &self.name)
            .field("namespace", &self.namespace)
            .field("path", &self.path)
            .field(
                "metadata_path",
                &self.metadata.as_ref().map(|file| &file.path),
            )
            .finish()
    }
}

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
