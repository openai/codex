//! Cloud-hosted configuration data for Codex.
//!
//! This crate owns transport, caching, and refresh behavior for cloud-delivered
//! config data. Parsing and composition remain in `codex-config`.

mod backend;
mod bundle_loader;
mod cache;
mod chatgpt;
mod config_server;
mod config_source;
mod metrics;
mod service;
mod validation;

pub use bundle_loader::cloud_config_bundle_loader;
pub use bundle_loader::cloud_config_bundle_loader_for_storage;
pub use bundle_loader::cloud_config_bundle_loader_for_storage_without_cache;
pub use bundle_loader::managed_config_auth_manager;
pub use bundle_loader::managed_config_bundle_loader;
pub use bundle_loader::managed_config_bundle_loader_for_storage;
pub use config_server::config_server_auth_manager;
pub use config_server::config_server_bundle_loader;
