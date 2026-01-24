//! Tool calling tests.
//!
//! Tests tool/function calling capabilities including single-turn and complete flow.

use anyhow::Result;
use hyper_sdk::Model;
use std::sync::Arc;

use crate::common::extract_text;
use crate::common::extract_tool_calls;
use crate::common::has_tool_call;
use crate::common::tool_output_request;
use crate::common::tool_request;
use crate::common::weather_tool;

/// Test basic tool calling.
///
/// Verifies that the model can generate function calls.
pub async fn run(model: &Arc<dyn Model>) -> Result<()> {
    let request = tool_request(
        "What's the weather in Tokyo? Use the get_weather tool.",
        vec![weather_tool()],
    );
    let response = model.generate(request).await?;

    assert!(
        has_tool_call(&response, "get_weather"),
        "Expected get_weather function call in response"
    );
    Ok(())
}

/// Test complete tool calling flow.
///
/// Verifies the full workflow: question -> tool call -> tool output -> final response.
pub async fn run_complete_flow(model: &Arc<dyn Model>) -> Result<()> {
    let question = "What's the weather in Tokyo?";
    let tools = vec![weather_tool()];

    // Step 1: Initial request - should get a tool call
    let request1 = tool_request(question, tools.clone());
    let result1 = model.generate(request1).await?;

    let tool_calls = extract_tool_calls(&result1);
    assert!(
        !tool_calls.is_empty(),
        "Expected at least one function call"
    );

    let call = &tool_calls[0];
    assert_eq!(call.name, "get_weather", "Expected get_weather call");

    // Step 2: Provide tool output and get final response
    let tool_output = r#"{"temperature": "22C", "condition": "sunny", "humidity": "45%"}"#;
    let request2 = tool_output_request(question, call, tool_output, tools);
    let result2 = model.generate(request2).await?;

    let text = extract_text(&result2);
    assert!(
        text.to_lowercase().contains("22") || text.to_lowercase().contains("sunny"),
        "Expected response to include weather info from tool output, got: {}",
        text
    );
    Ok(())
}
