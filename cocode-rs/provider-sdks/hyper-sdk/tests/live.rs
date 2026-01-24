//! Live integration tests for hyper-sdk providers.
//!
//! These tests run against real LLM provider APIs and require credentials
//! configured in `.env.test` or via environment variables.
//!
//! # Test Organization
//!
//! Tests are organized in a parameterized suite structure where the same test
//! logic runs against multiple providers. Each test function follows the pattern:
//! `test_{feature}_{provider}`.
//!
//! # Running Tests
//!
//! ```bash
//! # Run all integration tests (all configured providers)
//! cargo test -p hyper-sdk --test live -- --test-threads=1
//!
//! # Run all tests for a specific provider
//! cargo test -p hyper-sdk --test live openai -- --test-threads=1
//! cargo test -p hyper-sdk --test live anthropic -- --test-threads=1
//!
//! # Run specific test category
//! cargo test -p hyper-sdk --test live test_basic -- --test-threads=1
//! cargo test -p hyper-sdk --test live test_tools -- --test-threads=1
//!
//! # Run specific provider + feature
//! cargo test -p hyper-sdk --test live test_basic_openai -- --test-threads=1
//! cargo test -p hyper-sdk --test live test_streaming_anthropic -- --test-threads=1
//! ```
//!
//! # Configuration
//!
//! Set environment variables for each provider:
//! - `HYPER_SDK_TEST_{PROVIDER}_API_KEY` - Required
//! - `HYPER_SDK_TEST_{PROVIDER}_MODEL` - Required
//! - `HYPER_SDK_TEST_{PROVIDER}_BASE_URL` - Optional
//!
//! Or use a `.env.test` file in the crate root.

mod common;

// Test suite modules
#[path = "live/suite/mod.rs"]
mod suite;

use anyhow::Result;

// ============================================================================
// Helper Macro for Test Generation
// ============================================================================

/// Macro to generate a test function for a specific provider.
///
/// Usage: `provider_test!(provider_name, test_fn);`
macro_rules! provider_test {
    ($provider:expr, $test_fn:path) => {{
        let (_provider, model) = require_provider!($provider);
        $test_fn(&model).await
    }};
}

// ============================================================================
// Basic Text Generation Tests (All Providers)
// ============================================================================

#[tokio::test]
async fn test_basic_openai() -> Result<()> {
    provider_test!("openai", suite::basic::run)
}

#[tokio::test]
async fn test_basic_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::basic::run)
}

#[tokio::test]
async fn test_basic_gemini() -> Result<()> {
    provider_test!("gemini", suite::basic::run)
}

#[tokio::test]
async fn test_basic_volcengine() -> Result<()> {
    provider_test!("volcengine", suite::basic::run)
}

#[tokio::test]
async fn test_basic_zai() -> Result<()> {
    provider_test!("zai", suite::basic::run)
}

// ============================================================================
// Token Usage Tests (Providers with usage reporting)
// ============================================================================

#[tokio::test]
async fn test_token_usage_openai() -> Result<()> {
    provider_test!("openai", suite::basic::run_token_usage)
}

#[tokio::test]
async fn test_token_usage_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::basic::run_token_usage)
}

#[tokio::test]
async fn test_token_usage_gemini() -> Result<()> {
    provider_test!("gemini", suite::basic::run_token_usage)
}

// ============================================================================
// Multi-Turn Conversation Tests (Providers with context preservation)
// ============================================================================

#[tokio::test]
async fn test_multi_turn_openai() -> Result<()> {
    provider_test!("openai", suite::basic::run_multi_turn)
}

#[tokio::test]
async fn test_multi_turn_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::basic::run_multi_turn)
}

#[tokio::test]
async fn test_multi_turn_gemini() -> Result<()> {
    provider_test!("gemini", suite::basic::run_multi_turn)
}

// ============================================================================
// Tool Calling Tests (All Providers)
// ============================================================================

#[tokio::test]
async fn test_tools_openai() -> Result<()> {
    provider_test!("openai", suite::tools::run)
}

#[tokio::test]
async fn test_tools_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::tools::run)
}

#[tokio::test]
async fn test_tools_gemini() -> Result<()> {
    provider_test!("gemini", suite::tools::run)
}

#[tokio::test]
async fn test_tools_volcengine() -> Result<()> {
    provider_test!("volcengine", suite::tools::run)
}

#[tokio::test]
async fn test_tools_zai() -> Result<()> {
    provider_test!("zai", suite::tools::run)
}

// ============================================================================
// Tool Complete Flow Tests (All Providers)
// ============================================================================

#[tokio::test]
async fn test_tool_flow_openai() -> Result<()> {
    provider_test!("openai", suite::tools::run_complete_flow)
}

#[tokio::test]
async fn test_tool_flow_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::tools::run_complete_flow)
}

#[tokio::test]
async fn test_tool_flow_gemini() -> Result<()> {
    provider_test!("gemini", suite::tools::run_complete_flow)
}

// ============================================================================
// Vision/Image Tests (Vision-capable providers only)
// ============================================================================

#[tokio::test]
async fn test_vision_openai() -> Result<()> {
    provider_test!("openai", suite::vision::run)
}

#[tokio::test]
async fn test_vision_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::vision::run)
}

#[tokio::test]
async fn test_vision_gemini() -> Result<()> {
    provider_test!("gemini", suite::vision::run)
}

// ============================================================================
// Streaming Tests (All Providers)
// ============================================================================

#[tokio::test]
async fn test_streaming_openai() -> Result<()> {
    provider_test!("openai", suite::streaming::run)
}

#[tokio::test]
async fn test_streaming_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::streaming::run)
}

#[tokio::test]
async fn test_streaming_gemini() -> Result<()> {
    provider_test!("gemini", suite::streaming::run)
}

// ============================================================================
// Streaming with Tools Tests
// ============================================================================

#[tokio::test]
async fn test_streaming_tools_openai() -> Result<()> {
    provider_test!("openai", suite::streaming::run_with_tools)
}

#[tokio::test]
async fn test_streaming_tools_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::streaming::run_with_tools)
}

// ============================================================================
// Cross-Provider Conversation Tests
// ============================================================================

/// Macro to generate cross-provider test functions.
macro_rules! cross_provider_test {
    ($source:expr, $target:expr, $test_fn:path) => {{
        let source_cfg = match common::load_test_config($source) {
            Some(cfg) if cfg.enabled => cfg,
            _ => {
                eprintln!(
                    "Skipping cross-provider test: source provider '{}' not configured",
                    $source
                );
                return Ok(());
            }
        };
        let target_cfg = match common::load_test_config($target) {
            Some(cfg) if cfg.enabled => cfg,
            _ => {
                eprintln!(
                    "Skipping cross-provider test: target provider '{}' not configured",
                    $target
                );
                return Ok(());
            }
        };

        let (_, source_model) = match common::create_provider_and_model(&source_cfg) {
            Some(pair) => pair,
            None => {
                eprintln!("Skipping: failed to create source provider '{}'", $source);
                return Ok(());
            }
        };
        let (_, target_model) = match common::create_provider_and_model(&target_cfg) {
            Some(pair) => pair,
            None => {
                eprintln!("Skipping: failed to create target provider '{}'", $target);
                return Ok(());
            }
        };

        $test_fn(&source_model, &target_model).await
    }};
}

#[tokio::test]
async fn test_cross_provider_openai_to_anthropic() -> Result<()> {
    cross_provider_test!("openai", "anthropic", suite::cross_provider::run)
}

#[tokio::test]
async fn test_cross_provider_anthropic_to_openai() -> Result<()> {
    cross_provider_test!("anthropic", "openai", suite::cross_provider::run)
}

#[tokio::test]
async fn test_cross_provider_openai_to_gemini() -> Result<()> {
    cross_provider_test!("openai", "gemini", suite::cross_provider::run)
}

#[tokio::test]
async fn test_cross_provider_gemini_to_anthropic() -> Result<()> {
    cross_provider_test!("gemini", "anthropic", suite::cross_provider::run)
}

#[tokio::test]
async fn test_cross_provider_with_thinking_anthropic_to_openai() -> Result<()> {
    cross_provider_test!(
        "anthropic",
        "openai",
        suite::cross_provider::run_with_thinking
    )
}

#[tokio::test]
async fn test_cross_provider_streaming_openai_to_anthropic() -> Result<()> {
    cross_provider_test!("openai", "anthropic", suite::cross_provider::run_streaming)
}

// ============================================================================
// Configuration Tests
// ============================================================================

#[test]
fn test_list_configured_providers() {
    let providers = common::config::list_configured_providers();
    eprintln!("Configured providers: {:?}", providers);
}
