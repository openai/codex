//! Types used to define the fields of [`crate::config::Config`].

// Note this file should generally be restricted to simple struct/enum
// definitions that do not contain business logic.

use serde::Deserialize;
use serde::Serialize;

/// Retrieval system configuration for code search.
///
/// This is a minimal configuration that controls whether retrieval is enabled.
/// Full configuration is handled by the retrieval crate via `RetrievalConfig::load()`.
///
/// # Usage
///
/// Core delegates config loading to retrieval module:
/// ```ignore
/// use codex_retrieval::{RetrievalConfig, RetrievalService};
///
/// // Load from default locations (.codex/retrieval.toml)
/// let config = RetrievalConfig::load(&workdir)?;
///
/// // Or from specific file
/// let config = RetrievalConfig::from_file(&config_path)?;
///
/// // Get service
/// let service = RetrievalService::for_workdir(&workdir).await?;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RetrievalConfigToml {
    /// Whether retrieval is enabled
    pub enabled: bool,

    /// Path to retrieval config file (optional).
    /// If not set, retrieval will search default locations:
    /// 1. {workdir}/.codex/retrieval.toml
    /// 2. ~/.codex/retrieval.toml
    pub config_path: Option<std::path::PathBuf>,
}

impl Default for RetrievalConfigToml {
    fn default() -> Self {
        Self {
            enabled: false,
            config_path: None,
        }
    }
}

// Note: Hooks configuration has been moved to separate hooks.json file.
// See codex_hooks::config::HooksJsonConfig for the new format.
// Load hooks using codex_hooks::loader::load_hooks_config(cwd)
