//! Streaming tests.
//!
//! Tests streaming generation capabilities.

use anyhow::Result;
use hyper_sdk::Model;
use hyper_sdk::StreamEvent;
use std::sync::Arc;

use crate::common::text_request;

/// Test basic streaming generation.
///
/// Verifies that the model can stream text responses.
pub async fn run(model: &Arc<dyn Model>) -> Result<()> {
    let request = text_request("Say 'hello world' exactly.");
    let mut stream = model.stream(request).await?;

    let mut collected_text = String::new();
    let mut event_count = 0;
    let mut has_response_done = false;

    while let Some(event) = stream.next_event().await {
        event_count += 1;
        match event? {
            StreamEvent::TextDelta { delta, .. } => {
                collected_text.push_str(&delta);
            }
            StreamEvent::ResponseDone { .. } => {
                has_response_done = true;
            }
            _ => {}
        }
    }

    assert!(event_count > 0, "Expected at least one stream event");
    assert!(has_response_done, "Expected ResponseDone event");
    assert!(
        collected_text.to_lowercase().contains("hello"),
        "Expected 'hello' in streamed text, got: {}",
        collected_text
    );

    Ok(())
}

/// Test streaming with tool calls.
///
/// Verifies that tool calls are properly streamed.
pub async fn run_with_tools(model: &Arc<dyn Model>) -> Result<()> {
    use crate::common::tool_request;
    use crate::common::weather_tool;

    let request = tool_request(
        "What's the weather in Tokyo? Use the get_weather tool.",
        vec![weather_tool()],
    );
    let mut stream = model.stream(request).await?;

    let mut has_tool_call_start = false;
    let mut tool_name = String::new();

    while let Some(event) = stream.next_event().await {
        match event? {
            StreamEvent::ToolCallStart { name, .. } => {
                has_tool_call_start = true;
                tool_name = name;
            }
            _ => {}
        }
    }

    assert!(
        has_tool_call_start,
        "Expected ToolCallStart event for weather tool"
    );
    assert_eq!(
        tool_name, "get_weather",
        "Expected get_weather tool call, got: {}",
        tool_name
    );

    Ok(())
}
