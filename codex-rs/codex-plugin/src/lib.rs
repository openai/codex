//! Claude Code-compatible plugin system for Codex (V2 schema only).
//!
//! This crate provides plugin discovery, installation, loading, and injection.
//! It supports multi-scope installations where the same plugin can have different
//! versions at user, project, managed, and local scopes.

pub mod cli;
pub mod error;
pub mod frontmatter;
pub mod global;
pub mod injection;
pub mod installer;
pub mod loader;
pub mod manifest;
pub mod marketplace;
pub mod registry;
pub mod service;
pub mod settings;

pub use error::PluginError;
pub use error::Result;
pub use global::clear_plugin_service;
pub use global::get_or_init_plugin_service;
pub use global::get_plugin_service;
pub use global::is_plugin_service_initialized;
pub use injection::PluginInjector;
pub use installer::PluginInstaller;
pub use loader::PluginLoader;
pub use manifest::AuthorInfo;
pub use manifest::PluginManifest;
pub use marketplace::MarketplaceManager;
pub use registry::InstallEntryV2;
pub use registry::InstallScope;
pub use registry::InstalledPluginsV2;
pub use registry::PluginRegistryV2;
pub use service::PluginService;
pub use settings::PluginSettings;

/// Plugin ID format: `{plugin-name}@{marketplace-name}`
pub const PLUGIN_ID_REGEX: &str = r"^[a-z0-9][-a-z0-9._]*@[a-z0-9][-a-z0-9._]*$";

/// V2 registry filename
pub const REGISTRY_FILENAME: &str = "installed_plugins_v2.json";

/// Marketplaces config filename
pub const MARKETPLACES_FILENAME: &str = ".marketplaces.json";

/// Plugin manifest directory name (Codex-specific, not Claude compatible)
pub const PLUGIN_MANIFEST_DIR: &str = ".codex-plugin";

/// Plugin manifest filename
pub const PLUGIN_MANIFEST_FILE: &str = "plugin.json";

/// Marketplace manifest filename
pub const MARKETPLACE_MANIFEST_FILE: &str = "marketplace.json";
