//! Cross-provider conversation tests.
//!
//! Tests that verify messages from one provider can be correctly
//! sanitized and used with another provider.

use anyhow::Result;
use hyper_sdk::Model;
use hyper_sdk::messages::ContentBlock;
use hyper_sdk::messages::Message;
use hyper_sdk::request::GenerateRequest;
use std::sync::Arc;

/// Run cross-provider conversation test.
///
/// This test simulates a conversation where messages from one provider
/// are sent to another provider. It verifies:
/// 1. Messages are properly sanitized (thinking signatures stripped)
/// 2. The target provider can process the sanitized history
/// 3. Response is valid
pub async fn run(source_model: &Arc<dyn Model>, target_model: &Arc<dyn Model>) -> Result<()> {
    // Step 1: Get a response from the source provider
    let source_request = GenerateRequest::from_text("What is 2+2? Reply with just the number.")
        .max_tokens(100)
        .temperature(0.0);

    let source_response = source_model.generate(source_request).await?;

    // Convert response to assistant message with source tracking
    let mut assistant_msg = Message::assistant(&source_response.text());
    assistant_msg.metadata.source_provider = Some(source_model.provider().to_string());
    assistant_msg.metadata.source_model = Some(source_model.model_id().to_string());

    // Step 2: Create follow-up request with history for target provider
    let mut follow_up_request = GenerateRequest::new(vec![
        Message::user("What is 2+2? Reply with just the number."),
        assistant_msg,
        Message::user("What is double that number? Reply with just the number."),
    ])
    .max_tokens(100)
    .temperature(0.0);

    // Sanitize for target provider
    follow_up_request.sanitize_for_target(target_model.provider(), target_model.model_id());

    // Step 3: Send to target provider
    let target_response = target_model.generate(follow_up_request).await?;

    // Verify we got a valid response
    let response_text = target_response.text();
    let response_text = response_text.trim();
    assert!(
        !response_text.is_empty(),
        "Target provider should return a response"
    );

    // The response should be "8" (or variations like "8." or "The answer is 8")
    // We just verify it contains "8" somewhere
    assert!(
        response_text.contains('8'),
        "Response should contain '8', got: {}",
        response_text
    );

    Ok(())
}

/// Run cross-provider conversation test with thinking content.
///
/// This test verifies that thinking content and signatures are properly
/// handled when switching providers.
pub async fn run_with_thinking(
    source_model: &Arc<dyn Model>,
    target_model: &Arc<dyn Model>,
) -> Result<()> {
    // Create a message with thinking content (simulating source provider response)
    let thinking_msg = Message::new(
        hyper_sdk::messages::Role::Assistant,
        vec![
            ContentBlock::Thinking {
                content: "Let me think about this step by step...".to_string(),
                signature: Some("simulated_signature_from_source".to_string()),
            },
            ContentBlock::text("The answer is 42."),
        ],
    )
    .with_source(source_model.provider(), source_model.model_id());

    // Create request with the thinking message
    let mut request = GenerateRequest::new(vec![
        Message::user("What is the meaning of life?"),
        thinking_msg,
        Message::user("Why is that the answer? Be brief."),
    ])
    .max_tokens(200)
    .temperature(0.5);

    // Sanitize for target provider - this should strip the signature
    request.sanitize_for_target(target_model.provider(), target_model.model_id());

    // Verify signature was stripped before sending
    if let ContentBlock::Thinking { signature, .. } = &request.messages[1].content[0] {
        // If switching providers, signature should be stripped
        if source_model.provider() != target_model.provider() {
            assert!(
                signature.is_none(),
                "Signature should be stripped when switching providers"
            );
        }
    }

    // Send to target provider
    let response = target_model.generate(request).await?;

    // Verify we got a valid response
    assert!(!response.text().is_empty(), "Should get a response");

    Ok(())
}

/// Run streaming cross-provider test.
///
/// Tests that streaming works correctly with cross-provider message history.
pub async fn run_streaming(
    source_model: &Arc<dyn Model>,
    target_model: &Arc<dyn Model>,
) -> Result<()> {
    // Create message from source provider
    let source_msg = Message::assistant("The capital of France is Paris.")
        .with_source(source_model.provider(), source_model.model_id());

    // Create follow-up request
    let mut request = GenerateRequest::new(vec![
        Message::user("What is the capital of France?"),
        source_msg,
        Message::user("What is a famous landmark there?"),
    ])
    .max_tokens(150)
    .temperature(0.5);

    // Sanitize for target
    request.sanitize_for_target(target_model.provider(), target_model.model_id());

    // Stream the response and collect using the processor
    let stream = target_model.stream(request).await?;
    let response = stream.into_processor().collect().await?;

    // Verify response
    let text = response.text().to_lowercase();
    assert!(!text.is_empty(), "Should get a streaming response");
    // Response should mention Eiffel Tower or another Paris landmark
    let mentions_landmark = text.contains("eiffel")
        || text.contains("louvre")
        || text.contains("notre")
        || text.contains("arc")
        || text.contains("tower")
        || text.contains("museum");
    assert!(
        mentions_landmark || text.len() > 20,
        "Response should mention a Paris landmark or be substantive: {}",
        text
    );

    Ok(())
}
