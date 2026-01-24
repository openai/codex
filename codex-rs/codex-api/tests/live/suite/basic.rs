//! Basic text generation tests.
//!
//! Tests simple text completion and token usage reporting.

use anyhow::Result;
use codex_api::AdapterConfig;
use codex_api::ProviderAdapter;

use crate::common::extract_text;
use crate::common::text_prompt;

/// Test basic text generation.
///
/// Verifies that the adapter can generate simple text responses.
pub async fn run(adapter: &dyn ProviderAdapter, config: &AdapterConfig) -> Result<()> {
    let prompt = text_prompt("Say 'hello' in exactly one word, nothing else.");
    let result = adapter.generate(&prompt, config).await?;

    let text = extract_text(&result);
    assert!(
        text.to_lowercase().contains("hello"),
        "Expected 'hello' in response, got: {}",
        text
    );
    Ok(())
}

/// Test token usage reporting.
///
/// Verifies that the adapter reports token usage statistics.
pub async fn run_token_usage(adapter: &dyn ProviderAdapter, config: &AdapterConfig) -> Result<()> {
    let prompt = text_prompt("Say 'hello'.");
    let result = adapter.generate(&prompt, config).await?;

    assert!(result.usage.is_some(), "Expected token usage in response");

    let usage = result.usage.unwrap();
    assert!(usage.input_tokens > 0, "Expected non-zero input tokens");
    assert!(usage.output_tokens > 0, "Expected non-zero output tokens");
    Ok(())
}

/// Test multi-turn conversation.
///
/// Verifies that the adapter preserves context across conversation turns.
pub async fn run_multi_turn(adapter: &dyn ProviderAdapter, config: &AdapterConfig) -> Result<()> {
    use crate::common::assistant_message;
    use crate::common::multi_turn_prompt;
    use crate::common::user_message;

    let history = vec![
        user_message("My name is TestUser. Please remember it."),
        assistant_message("Hello TestUser! I'll remember your name."),
    ];

    let prompt = multi_turn_prompt(history, "What is my name?", vec![]);
    let result = adapter.generate(&prompt, config).await?;

    let text = extract_text(&result);
    assert!(
        text.to_lowercase().contains("testuser"),
        "Expected 'testuser' in response (context should be preserved), got: {}",
        text
    );
    Ok(())
}
