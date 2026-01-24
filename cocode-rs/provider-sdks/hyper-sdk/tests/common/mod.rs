//! Test helpers and macros for hyper-sdk integration tests.
//!
//! This module provides utilities for running integration tests against
//! real LLM providers. Tests are gated by environment configuration -
//! if credentials are not provided, tests skip gracefully.

pub mod config;
pub mod fixtures;

pub use config::TestConfig;
pub use config::load_test_config;
pub use fixtures::*;

use hyper_sdk::Model;
use hyper_sdk::Provider;
use std::sync::Arc;

/// Create a provider and model from test configuration.
pub fn create_provider_and_model(cfg: &TestConfig) -> Option<(Arc<dyn Provider>, Arc<dyn Model>)> {
    let provider: Arc<dyn Provider> = match cfg.provider.as_str() {
        "openai" => {
            let mut builder = hyper_sdk::OpenAIProvider::builder().api_key(&cfg.api_key);
            if let Some(ref url) = cfg.base_url {
                builder = builder.base_url(url);
            }
            match builder.build() {
                Ok(p) => Arc::new(p),
                Err(e) => {
                    eprintln!("Failed to create OpenAI provider: {e}");
                    return None;
                }
            }
        }
        "anthropic" => {
            let mut builder = hyper_sdk::AnthropicProvider::builder().api_key(&cfg.api_key);
            if let Some(ref url) = cfg.base_url {
                builder = builder.base_url(url);
            }
            match builder.build() {
                Ok(p) => Arc::new(p),
                Err(e) => {
                    eprintln!("Failed to create Anthropic provider: {e}");
                    return None;
                }
            }
        }
        "gemini" => {
            let mut builder = hyper_sdk::GeminiProvider::builder().api_key(&cfg.api_key);
            if let Some(ref url) = cfg.base_url {
                builder = builder.base_url(url);
            }
            match builder.build() {
                Ok(p) => Arc::new(p),
                Err(e) => {
                    eprintln!("Failed to create Gemini provider: {e}");
                    return None;
                }
            }
        }
        "volcengine" => {
            let mut builder = hyper_sdk::VolcengineProvider::builder().api_key(&cfg.api_key);
            if let Some(ref url) = cfg.base_url {
                builder = builder.base_url(url);
            }
            match builder.build() {
                Ok(p) => Arc::new(p),
                Err(e) => {
                    eprintln!("Failed to create Volcengine provider: {e}");
                    return None;
                }
            }
        }
        "zai" => {
            let mut builder = hyper_sdk::ZaiProvider::builder().api_key(&cfg.api_key);
            if let Some(ref url) = cfg.base_url {
                builder = builder.base_url(url);
            }
            match builder.build() {
                Ok(p) => Arc::new(p),
                Err(e) => {
                    eprintln!("Failed to create ZAI provider: {e}");
                    return None;
                }
            }
        }
        _ => {
            eprintln!("Unknown provider: {}", cfg.provider);
            return None;
        }
    };

    match provider.model(&cfg.model) {
        Ok(model) => Some((provider, model)),
        Err(e) => {
            eprintln!("Failed to create model {}: {e}", cfg.model);
            None
        }
    }
}

/// Macro to require a provider configuration, skipping the test if not available.
///
/// Usage:
/// ```ignore
/// #[tokio::test]
/// async fn test_openai_text_generation() -> anyhow::Result<()> {
///     let (provider, model) = require_provider!("openai");
///     // Test code...
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_provider {
    ($provider:expr) => {
        match $crate::common::load_test_config($provider) {
            Some(cfg) if cfg.enabled => match $crate::common::create_provider_and_model(&cfg) {
                Some((provider, model)) => (provider, model),
                None => {
                    eprintln!("Skipping test: failed to create provider '{}'", $provider);
                    return Ok(());
                }
            },
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
