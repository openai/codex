//! Sandbox enforcement for the cocode agent.
//!
//! This crate provides:
//! - Sandbox configuration (mode, allowed/denied paths, network access)
//! - Permission checking for file and network operations
//! - Platform-specific sandbox stubs (Unix, Windows)

pub mod checker;
pub mod config;
pub mod error;
pub mod platform;

pub use checker::PermissionChecker;
pub use config::{SandboxConfig, SandboxMode};
pub use error::SandboxError;
pub use platform::SandboxPlatform;
