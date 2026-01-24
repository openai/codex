//! Tool calling tests.
//!
//! Tests tool/function calling capabilities including single-turn and complete flow.

use anyhow::Result;
use codex_api::AdapterConfig;
use codex_api::ProviderAdapter;

use crate::common::extract_function_calls;
use crate::common::extract_text;
use crate::common::has_function_call;
use crate::common::tool_output_prompt;
use crate::common::tool_prompt;
use crate::common::weather_tool;

/// Test basic tool calling.
///
/// Verifies that the adapter can generate function calls.
pub async fn run(adapter: &dyn ProviderAdapter, config: &AdapterConfig) -> Result<()> {
    let prompt = tool_prompt(
        "What's the weather in Tokyo? Use the get_weather tool.",
        vec![weather_tool()],
    );
    let result = adapter.generate(&prompt, config).await?;

    assert!(
        has_function_call(&result, "get_weather"),
        "Expected get_weather function call in response"
    );
    Ok(())
}

/// Test complete tool calling flow.
///
/// Verifies the full workflow: question -> tool call -> tool output -> final response.
pub async fn run_complete_flow(
    adapter: &dyn ProviderAdapter,
    config: &AdapterConfig,
) -> Result<()> {
    let question = "What's the weather in Tokyo?";
    let tools = vec![weather_tool()];

    // Step 1: Initial request - should get a tool call
    let prompt1 = tool_prompt(question, tools.clone());
    let result1 = adapter.generate(&prompt1, config).await?;

    let function_calls = extract_function_calls(&result1);
    assert!(
        !function_calls.is_empty(),
        "Expected at least one function call"
    );

    let call = &function_calls[0];
    assert_eq!(call.name, "get_weather", "Expected get_weather call");

    // Step 2: Provide tool output and get final response
    let tool_output = r#"{"temperature": "22°C", "condition": "sunny", "humidity": "45%"}"#;
    let prompt2 = tool_output_prompt(question, call, tool_output, tools);
    let result2 = adapter.generate(&prompt2, config).await?;

    let text = extract_text(&result2);
    assert!(
        text.to_lowercase().contains("22") || text.to_lowercase().contains("sunny"),
        "Expected response to include weather info from tool output, got: {}",
        text
    );
    Ok(())
}
