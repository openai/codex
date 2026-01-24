//! Test fixtures and data generators for integration tests.
//!
//! This module provides reusable test data including prompts, tools,
//! and image content for testing various provider features.

#![allow(dead_code)]

use hyper_sdk::ContentBlock;
use hyper_sdk::GenerateRequest;
use hyper_sdk::GenerateResponse;
use hyper_sdk::ImageSource;
use hyper_sdk::Message;
use hyper_sdk::ToolCall;
use hyper_sdk::ToolDefinition;
use hyper_sdk::ToolResultContent;
use serde_json::Value;
use serde_json::json;

/// Create a simple text request for testing.
pub fn text_request(content: &str) -> GenerateRequest {
    GenerateRequest::new(vec![
        Message::system("You are a helpful assistant. Be concise."),
        Message::user(content),
    ])
}

/// Create a request with tool definitions for testing tool calling.
pub fn tool_request(content: &str, tools: Vec<ToolDefinition>) -> GenerateRequest {
    GenerateRequest::new(vec![
        Message::system("You are a helpful assistant. Use the provided tools when appropriate."),
        Message::user(content),
    ])
    .tools(tools)
}

/// Create a request with an image for testing multi-modal capabilities.
pub fn image_request(content: &str, image_data_url: &str) -> GenerateRequest {
    // Parse data URL: data:image/png;base64,<base64_data>
    let (media_type, base64_data) = parse_data_url(image_data_url);

    GenerateRequest::new(vec![
        Message::system("You are a helpful assistant that can analyze images."),
        Message::user_with_image(
            content,
            ImageSource::Base64 {
                data: base64_data,
                media_type,
            },
        ),
    ])
}

/// Parse a data URL into media type and base64 data.
fn parse_data_url(data_url: &str) -> (String, String) {
    // Format: data:<media_type>;base64,<data>
    if let Some(rest) = data_url.strip_prefix("data:") {
        if let Some((meta, data)) = rest.split_once(",") {
            if let Some((media_type, _)) = meta.split_once(";") {
                return (media_type.to_string(), data.to_string());
            }
        }
    }
    ("image/png".to_string(), data_url.to_string())
}

/// Create a simple weather tool definition for testing.
pub fn weather_tool() -> ToolDefinition {
    ToolDefinition::full(
        "get_weather",
        "Get the current weather for a city",
        json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "The city name"
                }
            },
            "required": ["city"]
        }),
    )
}

/// Create a calculator tool definition for testing.
pub fn calculator_tool() -> ToolDefinition {
    ToolDefinition::full(
        "calculate",
        "Perform a mathematical calculation",
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate"
                }
            },
            "required": ["expression"]
        }),
    )
}

/// Get rain forecast tool definition for travel scenario.
pub fn rain_forecast_tool() -> ToolDefinition {
    ToolDefinition::full(
        "get_rain_forecast",
        "Get rain forecast for a city on a specific date",
        json!({
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
        }),
    )
}

/// Get all travel-related tools.
pub fn travel_tools() -> Vec<ToolDefinition> {
    vec![weather_tool(), rain_forecast_tool()]
}

/// A small 10x10 red square PNG image encoded as base64 data URL.
pub const TEST_RED_SQUARE_BASE64: &str = "data:image/png;base64,\
iVBORw0KGgoAAAANSUhEUgAAAAoAAAAKCAYAAACNMs+9AAAAFUlEQVR4AWNgGAWjYBSMglEwCkgHAA+IAAT6kbF5AAAAAElFTkSuQmCC";

/// A small 10x10 blue square PNG image encoded as base64 data URL.
pub const TEST_BLUE_SQUARE_BASE64: &str = "data:image/png;base64,\
iVBORw0KGgoAAAANSUhEUgAAAAoAAAAKCAYAAACNMs+9AAAAFUlEQVR4AWP4//8/w0AmGAWjgHIAABZQAQVmGY6GAAAAAElFTkSuQmCC";

/// Extract text content from a GenerateResponse.
pub fn extract_text(response: &GenerateResponse) -> String {
    response.text()
}

/// Check if response contains a tool call with the given name.
pub fn has_tool_call(response: &GenerateResponse, name: &str) -> bool {
    response.tool_calls().iter().any(|tc| tc.name == name)
}

/// Check if response contains reasoning/thinking content.
pub fn has_thinking(response: &GenerateResponse) -> bool {
    response.has_thinking()
}

/// Extract all tool calls from a GenerateResponse.
pub fn extract_tool_calls(response: &GenerateResponse) -> Vec<ToolCall> {
    response.tool_calls()
}

/// Create a multi-turn conversation request.
pub fn multi_turn_request(
    history: Vec<Message>,
    user_message: &str,
    tools: Vec<ToolDefinition>,
) -> GenerateRequest {
    let mut messages = vec![Message::system("You are a helpful assistant.")];
    messages.extend(history);
    messages.push(Message::user(user_message));

    if tools.is_empty() {
        GenerateRequest::new(messages)
    } else {
        GenerateRequest::new(messages).tools(tools)
    }
}

/// Create a request with tool call and its output for the second turn.
pub fn tool_output_request(
    original_question: &str,
    tool_call: &ToolCall,
    tool_output: &str,
    tools: Vec<ToolDefinition>,
) -> GenerateRequest {
    GenerateRequest::new(vec![
        Message::system(
            "You are a helpful assistant. Use tool results to answer the user's question.",
        ),
        Message::user(original_question),
        // Assistant's tool call
        Message::new(
            hyper_sdk::Role::Assistant,
            vec![ContentBlock::tool_use(
                &tool_call.id,
                &tool_call.name,
                tool_call.arguments.clone(),
            )],
        ),
        // Tool result
        Message::tool_result(&tool_call.id, ToolResultContent::text(tool_output)),
    ])
    .tools(tools)
}

/// Mock tool executor for travel scenario.
pub fn execute_travel_tool(name: &str, _args: &str) -> Value {
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

/// Create assistant message with tool call.
pub fn assistant_tool_call(name: &str, arguments: Value, call_id: &str) -> Message {
    Message::new(
        hyper_sdk::Role::Assistant,
        vec![ContentBlock::tool_use(call_id, name, arguments)],
    )
}

/// Create tool result message.
pub fn tool_result_message(call_id: &str, output: Value) -> Message {
    Message::tool_result(
        call_id,
        ToolResultContent::text(serde_json::to_string(&output).unwrap()),
    )
}
