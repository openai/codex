//! Test helpers and macros for codex-api integration tests.
//!
//! This module provides utilities for running integration tests against
//! real LLM providers. Tests are gated by environment configuration -
//! if credentials are not provided, tests skip gracefully.

pub mod config;
pub mod fixtures;

pub use config::TestConfig;
pub use config::load_test_config;
pub use fixtures::*;

use codex_api::AdapterConfig;
use codex_api::ProviderAdapter;
use std::sync::Arc;

/// Get an adapter by name and ensure it's registered.
pub fn get_adapter(name: &str) -> Option<Arc<dyn ProviderAdapter>> {
    codex_api::get_adapter(name)
}

/// Build AdapterConfig from TestConfig.
pub fn adapter_config(cfg: &TestConfig) -> AdapterConfig {
    AdapterConfig {
        api_key: Some(cfg.api_key.clone()),
        base_url: cfg.base_url.clone(),
        model: cfg.model.clone(),
        extra: None,
        request_hook: None,
        ultrathink_config: None,
    }
}

/// Macro to require a provider configuration, skipping the test if not available.
///
/// Usage:
/// ```ignore
/// #[tokio::test]
/// async fn test_genai_text_generation() -> anyhow::Result<()> {
///     let cfg = require_provider!("genai");
///     // Test code...
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_provider {
    ($provider:expr) => {
        match $crate::common::load_test_config($provider) {
            Some(cfg) if cfg.enabled => cfg,
            _ => {
                eprintln!(
                    "Skipping test: provider '{}' not configured in .env",
                    $provider
                );
                return Ok(());
            }
        }
    };
}
