//! Responses API tool specs for thread-local persistent timer management.
//!
//! These specs expose the `TimerCreate`, `TimerDelete`, and `TimerList`
//! built-in tools so models can create, inspect, and delete timers on the
//! current thread.

use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_timer_create_tool() -> ToolSpec {
    let trigger_properties = BTreeMap::from([
        (
            "kind".to_string(),
            JsonSchema::String {
                description: Some("Trigger kind. Use `delay` or `schedule`.".to_string()),
            },
        ),
        (
            "seconds".to_string(),
            JsonSchema::Number {
                description: Some("Delay trigger seconds from creation time.".to_string()),
            },
        ),
        (
            "repeat".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Delay trigger recurrence flag. With seconds 0, repeat means run whenever the thread is idle."
                        .to_string(),
                ),
            },
        ),
        (
            "dtstart".to_string(),
            JsonSchema::String {
                description: Some(
                    "Schedule trigger floating local datetime in YYYY-MM-DDTHH:MM:SS format."
                        .to_string(),
                ),
            },
        ),
        (
            "rrule".to_string(),
            JsonSchema::String {
                description: Some("Schedule trigger RRULE string.".to_string()),
            },
        ),
    ]);
    let properties = BTreeMap::from([
        (
            "trigger".to_string(),
            JsonSchema::Object {
                properties: trigger_properties,
                required: Some(vec!["kind".to_string()]),
                additional_properties: Some(false.into()),
            },
        ),
        (
            "prompt".to_string(),
            JsonSchema::String {
                description: Some("Prompt to execute when the timer fires.".to_string()),
            },
        ),
        (
            "delivery".to_string(),
            JsonSchema::String {
                description: Some(
                    "Delivery mode for the timer. Use `after-turn` or `steer-current-turn`."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "TimerCreate".to_string(),
        description: "Create a thread timer using a structured trigger, prompt, and delivery mode."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec![
                "trigger".to_string(),
                "prompt".to_string(),
                "delivery".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

pub fn create_timer_delete_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "id".to_string(),
        JsonSchema::String {
            description: Some("Identifier of the timer to delete.".to_string()),
        },
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: "TimerDelete".to_string(),
        description: "Delete a thread timer by id.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

pub fn create_timer_list_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "TimerList".to_string(),
        description: "List thread timers for the current thread.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "timer_tool_tests.rs"]
mod tests;
