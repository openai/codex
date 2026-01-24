//! Multi-turn tool calling tests with mock tool execution.
//!
//! Tests realistic multi-turn conversations with tool calls to verify
//! event handling and message assembly correctness.
//!
//! ## Scenario: Travel Decision Assistant
//!
//! User asks about weather for travel planning. The assistant uses
//! weather and rain forecast tools to provide advice.
//!
//! Flow:
//! 1. User: "Should I bring an umbrella to Beijing tomorrow?"
//! 2. LLM -> tool_call: get_weather("Beijing")
//! 3. Tool -> {"temperature": 25, "condition": "cloudy"}
//! 4. LLM -> tool_call: get_rain_forecast("Beijing", "tomorrow")
//! 5. Tool -> {"probability": 80, "expected_mm": 15}
//! 6. LLM -> Final response with recommendation

use anyhow::Result;
use codex_api::AdapterConfig;
use codex_api::ProviderAdapter;
use codex_api::ResponseEvent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use serde_json::Value as JsonValue;
use serde_json::json;

use crate::common::extract_function_calls;
use crate::common::extract_text;
use crate::common::multi_turn_prompt;
use crate::common::tool_prompt;

/// Travel-related tool definitions.
fn travel_tools() -> Vec<JsonValue> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get current weather for a city",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": {
                            "type": "string",
                            "description": "City name"
                        }
                    },
                    "required": ["city"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_rain_forecast",
                "description": "Get rain forecast for a city on a specific date",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": {
                            "type": "string",
                            "description": "City name"
                        },
                        "date": {
                            "type": "string",
                            "description": "Date like 'today', 'tomorrow', or YYYY-MM-DD"
                        }
                    },
                    "required": ["city", "date"]
                }
            }
        }),
    ]
}

/// Mock tool executor that returns predefined responses.
fn execute_mock_tool(name: &str, _args: &str) -> JsonValue {
    match name {
        "get_weather" => json!({
            "temperature": 25,
            "condition": "cloudy",
            "humidity": 70,
            "wind_speed": "10 km/h"
        }),
        "get_rain_forecast" => json!({
            "probability": 80,
            "expected_mm": 15,
            "description": "Heavy rain expected in the afternoon"
        }),
        _ => json!({"error": "Unknown tool"}),
    }
}

/// Create assistant function call ResponseItem.
fn assistant_function_call(name: &str, arguments: &str, call_id: &str) -> ResponseItem {
    ResponseItem::FunctionCall {
        id: None,
        name: name.to_string(),
        arguments: arguments.to_string(),
        call_id: call_id.to_string(),
    }
}

/// Create tool result ResponseItem.
fn tool_result(call_id: &str, output: JsonValue) -> ResponseItem {
    ResponseItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload {
            content: serde_json::to_string(&output).unwrap(),
            content_items: None,
            success: Some(true),
        },
    }
}

/// Test multi-turn tool calling with mock execution.
///
/// This test verifies:
/// 1. Tool calls are generated correctly
/// 2. Tool results are processed correctly
/// 3. Final response incorporates tool outputs
/// 4. Message assembly is correct
pub async fn run(adapter: &dyn ProviderAdapter, config: &AdapterConfig) -> Result<()> {
    let tools = travel_tools();
    let mut history: Vec<ResponseItem> = vec![];

    // Turn 1: User asks about bringing umbrella
    let user_question = "I'm planning to go to Beijing tomorrow. Should I bring an umbrella? Please check the weather.";
    let prompt1 = tool_prompt(user_question, tools.clone());
    let result1 = adapter.generate(&prompt1, config).await?;

    // Expect tool call(s)
    let function_calls = extract_function_calls(&result1);
    assert!(
        !function_calls.is_empty(),
        "Expected at least one tool call for weather query"
    );

    // Process each tool call
    for call in &function_calls {
        let output = execute_mock_tool(&call.name, &call.arguments);
        history.push(assistant_function_call(
            &call.name,
            &call.arguments,
            &call.call_id,
        ));
        history.push(tool_result(&call.call_id, output));
    }

    // Add the original user message at the beginning
    let user_msg = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: user_question.to_string(),
        }],
    };

    // Build prompt for turn 2 with tool results
    let mut full_history = vec![user_msg];
    full_history.extend(history);

    let prompt2 = multi_turn_prompt(full_history, "", tools.clone());
    let result2 = adapter.generate(&prompt2, config).await?;

    // Check if more tool calls are needed (some models may call both tools separately)
    let calls2 = extract_function_calls(&result2);
    let final_result = if !calls2.is_empty() {
        // Process additional tool calls
        let mut history2: Vec<ResponseItem> = vec![];
        for call in &calls2 {
            let output = execute_mock_tool(&call.name, &call.arguments);
            history2.push(assistant_function_call(
                &call.name,
                &call.arguments,
                &call.call_id,
            ));
            history2.push(tool_result(&call.call_id, output));
        }

        let prompt3 = multi_turn_prompt(history2, "", tools);
        adapter.generate(&prompt3, config).await?
    } else {
        result2
    };

    // Final assertion: response should mention weather-related advice
    let final_text = extract_text(&final_result);
    assert!(
        final_text.to_lowercase().contains("umbrella")
            || final_text.to_lowercase().contains("rain")
            || final_text.to_lowercase().contains("weather")
            || final_text.contains("80") // rain probability
            || final_text.contains("25"), // temperature
        "Expected weather-related advice in response, got: {}",
        final_text
    );

    Ok(())
}

/// Test event stream integrity for tool calls.
///
/// Verifies that the event stream has correct structure:
/// - Has message_start event
/// - Has message_end event
/// - Tool calls have valid IDs and names
pub async fn run_event_integrity(
    adapter: &dyn ProviderAdapter,
    config: &AdapterConfig,
) -> Result<()> {
    let tools = travel_tools();
    let prompt = tool_prompt("What's the weather in Tokyo?", tools);
    let result = adapter.generate(&prompt, config).await?;

    // Verify event structure
    let events = &result.events;

    // Should have Created event
    let has_created = events.iter().any(|e| matches!(e, ResponseEvent::Created));
    assert!(has_created, "Missing Created event");

    // Should have Completed event
    let has_completed = events
        .iter()
        .any(|e| matches!(e, ResponseEvent::Completed { .. }));
    assert!(has_completed, "Missing Completed event");

    // Verify tool call structure
    let tool_calls = extract_function_calls(&result);
    for call in tool_calls {
        assert!(!call.call_id.is_empty(), "Tool call missing ID");
        assert!(!call.name.is_empty(), "Tool call missing name");
    }

    Ok(())
}

/// Test message assembly correctness for tool responses.
///
/// Verifies that tool outputs are correctly incorporated into the conversation.
pub async fn run_message_assembly(
    adapter: &dyn ProviderAdapter,
    config: &AdapterConfig,
) -> Result<()> {
    use crate::common::tool_output_prompt;
    use crate::common::weather_tool;

    let tools = vec![weather_tool()];

    // Step 1: Get tool call
    let prompt1 = tool_prompt("What's the weather in Paris?", tools.clone());
    let result1 = adapter.generate(&prompt1, config).await?;

    let calls = extract_function_calls(&result1);
    assert!(!calls.is_empty(), "Expected tool call");
    let call = &calls[0];

    // Step 2: Send tool output back with specific data
    let tool_output = r#"{"temperature": 18, "condition": "sunny", "humidity": 55}"#;
    let prompt2 = tool_output_prompt("What's the weather in Paris?", call, tool_output, tools);
    let result2 = adapter.generate(&prompt2, config).await?;

    // Verify response references the specific tool output values
    let text = extract_text(&result2);
    assert!(
        text.contains("18") || text.to_lowercase().contains("sunny") || text.contains("55"),
        "Response should reference specific tool output values, got: {}",
        text
    );

    Ok(())
}
