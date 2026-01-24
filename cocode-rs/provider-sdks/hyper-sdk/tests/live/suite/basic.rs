//! Basic text generation tests.
//!
//! Tests simple text completion and token usage reporting.

use anyhow::Result;
use hyper_sdk::GenerateRequest;
use hyper_sdk::Message;
use hyper_sdk::Model;
use std::sync::Arc;

use crate::common::extract_text;
use crate::common::text_request;

/// Test basic text generation.
///
/// Verifies that the model can generate simple text responses.
pub async fn run(model: &Arc<dyn Model>) -> Result<()> {
    let request = text_request("Say 'hello' in exactly one word, nothing else.");
    let response = model.generate(request).await?;

    let text = extract_text(&response);
    assert!(
        text.to_lowercase().contains("hello"),
        "Expected 'hello' in response, got: {}",
        text
    );
    Ok(())
}

/// Test token usage reporting.
///
/// Verifies that the model reports token usage statistics.
pub async fn run_token_usage(model: &Arc<dyn Model>) -> Result<()> {
    let request = text_request("Say 'hello'.");
    let response = model.generate(request).await?;

    assert!(response.usage.is_some(), "Expected token usage in response");

    let usage = response.usage.unwrap();
    assert!(usage.prompt_tokens > 0, "Expected non-zero prompt tokens");
    assert!(
        usage.completion_tokens > 0,
        "Expected non-zero completion tokens"
    );
    Ok(())
}

/// Test multi-turn conversation.
///
/// Verifies that the model preserves context across conversation turns.
pub async fn run_multi_turn(model: &Arc<dyn Model>) -> Result<()> {
    let request = GenerateRequest::new(vec![
        Message::system("You are a helpful assistant."),
        Message::user("My name is TestUser. Please remember it."),
        Message::assistant("Hello TestUser! I'll remember your name."),
        Message::user("What is my name?"),
    ]);

    let response = model.generate(request).await?;

    let text = extract_text(&response);
    assert!(
        text.to_lowercase().contains("testuser"),
        "Expected 'testuser' in response (context should be preserved), got: {}",
        text
    );
    Ok(())
}
