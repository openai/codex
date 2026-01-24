//! Plugin system error types.

use std::path::PathBuf;
use thiserror::Error;

/// Plugin system errors.
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Invalid plugin ID format: {0}")]
    InvalidPluginId(String),

    #[error("Invalid manifest at {path}: {reason}")]
    InvalidManifest { path: PathBuf, reason: String },

    #[error("Plugin already installed: {0}")]
    AlreadyInstalled(String),

    #[error("Installation failed for {plugin}: {reason}")]
    InstallFailed { plugin: String, reason: String },

    #[error("Marketplace not found: {0}")]
    MarketplaceNotFound(String),

    #[error("Marketplace error: {0}")]
    Marketplace(String),

    #[error("Source error: {0}")]
    Source(String),

    #[error("Component injection failed: {0}")]
    InjectionFailed(String),

    #[error("Invalid scope for operation: {0}")]
    InvalidScope(String),

    #[error("Project path required for scope: {0}")]
    ProjectPathRequired(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Git operation failed: {0}")]
    Git(String),

    #[error("NPM operation failed: {0}")]
    Npm(String),

    #[error("Pip operation failed: {0}")]
    Pip(String),

    #[error("Registry error: {0}")]
    Registry(String),

    #[error("Update failed: {0}")]
    Update(String),
}

/// Result type alias for plugin operations.
pub type Result<T> = std::result::Result<T, PluginError>;
