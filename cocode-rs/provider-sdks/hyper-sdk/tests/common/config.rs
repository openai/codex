//! Test configuration loading from environment variables.
//!
//! This module handles loading test credentials from `.env` files,
//! with support for per-provider configuration and graceful skipping
//! when credentials are not available.
//!
//! # Environment Variable Naming
//!
//! ```text
//! HYPER_SDK_TEST_{PROVIDER}_{FIELD}
//! ```
//!
//! Examples:
//! - `HYPER_SDK_TEST_OPENAI_API_KEY`
//! - `HYPER_SDK_TEST_ANTHROPIC_MODEL`
//! - `HYPER_SDK_TEST_GEMINI_BASE_URL`
//!
//! # .env File Loading Priority
//!
//! 1. Path from `HYPER_SDK_TEST_ENV_FILE` environment variable
//! 2. `.env.test` in crate root
//! 3. `.env` in crate root

use std::path::PathBuf;
use std::sync::OnceLock;

/// Environment variable for custom .env file path.
const ENV_FILE_VAR: &str = "HYPER_SDK_TEST_ENV_FILE";

/// Default .env file location (relative to crate root).
const DEFAULT_ENV_FILE: &str = ".env.test";

/// Fallback .env file location.
const FALLBACK_ENV_FILE: &str = ".env";

/// Environment variable prefix for test configuration.
const ENV_PREFIX: &str = "HYPER_SDK_TEST";

/// Ensure .env file is loaded exactly once per test run.
static ENV_LOADED: OnceLock<bool> = OnceLock::new();

/// Test configuration for a specific LLM provider.
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Provider name (e.g., "openai", "anthropic").
    pub provider: String,
    /// API key for authentication.
    pub api_key: String,
    /// Model name to use.
    pub model: String,
    /// Optional custom endpoint URL.
    pub base_url: Option<String>,
    /// Whether this provider is enabled (has required credentials).
    pub enabled: bool,
}

/// Load .env file once per test run.
fn ensure_env_loaded() {
    ENV_LOADED.get_or_init(|| {
        let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // Priority: ENV_FILE_VAR > .env.test > .env
        let env_file = std::env::var(ENV_FILE_VAR)
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let test_env = crate_root.join(DEFAULT_ENV_FILE);
                if test_env.exists() {
                    test_env
                } else {
                    crate_root.join(FALLBACK_ENV_FILE)
                }
            });

        if env_file.exists() {
            if dotenvy::from_path(&env_file).is_ok() {
                eprintln!("Loaded test config from: {}", env_file.display());
            }
        } else {
            eprintln!(
                "No .env file found at {}, tests will be skipped",
                env_file.display()
            );
        }

        true
    });
}

/// Get environment variable for a specific provider and field.
fn get_provider_env(provider: &str, field: &str) -> Option<String> {
    let key = format!("{}_{}_{}", ENV_PREFIX, provider.to_uppercase(), field);
    std::env::var(&key).ok().filter(|v| !v.is_empty())
}

/// Load test configuration for a specific provider.
///
/// Returns `None` if the provider is not configured (no API key).
/// Returns `Some(config)` with `enabled = false` if partial config exists.
pub fn load_test_config(provider: &str) -> Option<TestConfig> {
    ensure_env_loaded();

    let api_key = get_provider_env(provider, "API_KEY");
    let model = get_provider_env(provider, "MODEL");
    let base_url = get_provider_env(provider, "BASE_URL");

    // API key is required for a provider to be enabled
    let enabled = api_key.is_some() && model.is_some();

    // Return None if no configuration at all
    if api_key.is_none() && model.is_none() && base_url.is_none() {
        return None;
    }

    Some(TestConfig {
        provider: provider.to_string(),
        api_key: api_key.unwrap_or_default(),
        model: model.unwrap_or_default(),
        base_url,
        enabled,
    })
}

/// List all providers that are configured (have API keys).
pub fn list_configured_providers() -> Vec<String> {
    ensure_env_loaded();

    let providers = ["openai", "anthropic", "gemini", "volcengine", "zai"];

    providers
        .iter()
        .filter_map(|p| {
            load_test_config(p).and_then(|c| if c.enabled { Some(c.provider) } else { None })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_loading_does_not_panic() {
        ensure_env_loaded();
    }

    #[test]
    fn test_load_test_config_returns_none_for_unconfigured() {
        ensure_env_loaded();
        let cfg = load_test_config("nonexistent_provider_xyz");
        assert!(cfg.is_none());
    }
}
