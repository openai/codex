//! Shared utilities for codex-rs.
//!
//! This crate provides common utilities that can be used by all crates
//! in the workspace without circular dependencies.

pub mod logging;

pub use logging::ConfigurableTimer;
pub use logging::LoggingConfig;
pub use logging::TimezoneConfig;
pub use logging::build_env_filter;
