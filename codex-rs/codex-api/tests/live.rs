//! Live integration tests for codex-api adapters.
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
//! cargo test -p codex-api --test live -- --test-threads=1
//!
//! # Run all tests for a specific provider
//! cargo test -p codex-api --test live genai -- --test-threads=1
//! cargo test -p codex-api --test live anthropic -- --test-threads=1
//!
//! # Run specific test category
//! cargo test -p codex-api --test live test_basic -- --test-threads=1
//! cargo test -p codex-api --test live test_tools -- --test-threads=1
//! cargo test -p codex-api --test live test_multi_turn -- --test-threads=1
//!
//! # Run specific provider + feature
//! cargo test -p codex-api --test live test_basic_genai -- --test-threads=1
//! cargo test -p codex-api --test live test_multi_turn_tools_anthropic -- --test-threads=1
//! ```
//!
//! # Configuration
//!
//! Set environment variables for each provider:
//! - `CODEX_API_TEST_{PROVIDER}_API_KEY` - Required
//! - `CODEX_API_TEST_{PROVIDER}_MODEL` - Required
//! - `CODEX_API_TEST_{PROVIDER}_BASE_URL` - Optional
//!
//! Or use a `.env.test` file in the crate root.

mod common;

// Test suite modules
#[path = "live/suite/mod.rs"]
mod suite;

// OpenAI uses native endpoint, keep separate
#[path = "live/openai.rs"]
mod openai;

use anyhow::Result;
use common::adapter_config;

// ============================================================================
// Helper Macro for Test Generation
// ============================================================================

/// Macro to generate a test function for a specific provider.
///
/// Usage: `provider_test!(provider_name, test_fn);`
macro_rules! provider_test {
    ($provider:expr, $test_fn:path) => {{
        let cfg = require_provider!($provider);
        let adapter =
            common::get_adapter($provider).expect(concat!($provider, " adapter not found"));
        let config = adapter_config(&cfg);
        $test_fn(&*adapter, &config).await
    }};
}

// ============================================================================
// Basic Text Generation Tests (All Providers)
// ============================================================================

#[tokio::test]
async fn test_basic_genai() -> Result<()> {
    provider_test!("genai", suite::basic::run)
}

#[tokio::test]
async fn test_basic_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::basic::run)
}

#[tokio::test]
async fn test_basic_volc_ark() -> Result<()> {
    provider_test!("volc_ark", suite::basic::run)
}

#[tokio::test]
async fn test_basic_zai() -> Result<()> {
    provider_test!("zai", suite::basic::run)
}

// ============================================================================
// Token Usage Tests (Providers with usage reporting)
// ============================================================================

#[tokio::test]
async fn test_token_usage_genai() -> Result<()> {
    provider_test!("genai", suite::basic::run_token_usage)
}

#[tokio::test]
async fn test_token_usage_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::basic::run_token_usage)
}

// ============================================================================
// Multi-Turn Conversation Tests (Providers with context preservation)
// ============================================================================

#[tokio::test]
async fn test_multi_turn_genai() -> Result<()> {
    provider_test!("genai", suite::basic::run_multi_turn)
}

#[tokio::test]
async fn test_multi_turn_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::basic::run_multi_turn)
}

// ============================================================================
// Tool Calling Tests (All Providers)
// ============================================================================

#[tokio::test]
async fn test_tools_genai() -> Result<()> {
    provider_test!("genai", suite::tools::run)
}

#[tokio::test]
async fn test_tools_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::tools::run)
}

#[tokio::test]
async fn test_tools_volc_ark() -> Result<()> {
    provider_test!("volc_ark", suite::tools::run)
}

#[tokio::test]
async fn test_tools_zai() -> Result<()> {
    provider_test!("zai", suite::tools::run)
}

// ============================================================================
// Tool Complete Flow Tests (All Providers)
// ============================================================================

#[tokio::test]
async fn test_tool_flow_genai() -> Result<()> {
    provider_test!("genai", suite::tools::run_complete_flow)
}

#[tokio::test]
async fn test_tool_flow_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::tools::run_complete_flow)
}

#[tokio::test]
async fn test_tool_flow_volc_ark() -> Result<()> {
    provider_test!("volc_ark", suite::tools::run_complete_flow)
}

#[tokio::test]
async fn test_tool_flow_zai() -> Result<()> {
    provider_test!("zai", suite::tools::run_complete_flow)
}

// ============================================================================
// Vision/Image Tests (Vision-capable providers only)
// ============================================================================

#[tokio::test]
async fn test_vision_genai() -> Result<()> {
    provider_test!("genai", suite::vision::run)
}

#[tokio::test]
async fn test_vision_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::vision::run)
}

// ============================================================================
// Reasoning/Thinking Mode Tests (Reasoning-capable providers only)
// ============================================================================

#[tokio::test]
async fn test_reasoning_genai() -> Result<()> {
    provider_test!("genai", suite::reasoning::run)
}

#[tokio::test]
async fn test_reasoning_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::reasoning::run)
}

// ============================================================================
// Multi-Turn Tool Tests (Travel Scenario)
// ============================================================================

#[tokio::test]
async fn test_multi_turn_tools_genai() -> Result<()> {
    provider_test!("genai", suite::multi_turn_tools::run)
}

#[tokio::test]
async fn test_multi_turn_tools_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::multi_turn_tools::run)
}

// ============================================================================
// Event Integrity Tests
// ============================================================================

#[tokio::test]
async fn test_event_integrity_genai() -> Result<()> {
    provider_test!("genai", suite::multi_turn_tools::run_event_integrity)
}

#[tokio::test]
async fn test_event_integrity_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::multi_turn_tools::run_event_integrity)
}

// ============================================================================
// Message Assembly Tests
// ============================================================================

#[tokio::test]
async fn test_message_assembly_genai() -> Result<()> {
    provider_test!("genai", suite::multi_turn_tools::run_message_assembly)
}

#[tokio::test]
async fn test_message_assembly_anthropic() -> Result<()> {
    provider_test!("anthropic", suite::multi_turn_tools::run_message_assembly)
}

// ============================================================================
// Configuration Tests
// ============================================================================

#[test]
fn test_list_configured_providers() {
    let providers = common::config::list_configured_providers();
    eprintln!("Configured providers: {:?}", providers);
    // This test just verifies the function doesn't panic
}

#[test]
fn test_all_builtin_adapters_available() {
    // Verify all built-in adapters are registered
    assert!(common::get_adapter("genai").is_some());
    assert!(common::get_adapter("anthropic").is_some());
    assert!(common::get_adapter("volc_ark").is_some());
    assert!(common::get_adapter("zai").is_some());
}
