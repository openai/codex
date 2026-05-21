//! Cloud-hosted configuration data for Codex.
//!
//! This crate owns transport, caching, and refresh behavior for cloud-delivered
//! config data. Parsing and composition remain in `codex-config`.

mod cache;
mod loader;

pub use loader::cloud_config_bundle_loader;
pub use loader::cloud_config_bundle_loader_for_storage;
