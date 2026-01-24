//! Reasoning/thinking mode tests.
//!
//! Tests extended thinking capabilities for providers that support it.

use anyhow::Result;
use codex_api::AdapterConfig;
use codex_api::ProviderAdapter;

use crate::common::extract_text;
use crate::common::text_prompt;

/// Test reasoning mode.
///
/// Verifies that the adapter can perform step-by-step reasoning.
pub async fn run(adapter: &dyn ProviderAdapter, config: &AdapterConfig) -> Result<()> {
    let mut config = config.clone();

    // Enable thinking mode via extra config
    config.extra = Some(serde_json::json!({
        "thinking": {
            "type": "enabled",
            "budget_tokens": 1024
        }
    }));

    let prompt = text_prompt("What is 17 * 23? Think step by step.");
    let result = adapter.generate(&prompt, &config).await?;

    let text = extract_text(&result);
    assert!(
        text.contains("391"),
        "Expected '391' in response, got: {}",
        text
    );
    // Note: Reasoning items may or may not be returned depending on model
    Ok(())
}
