//! Responses API tool specs for thread-local persistent alarm management.
//!
//! These specs expose the `AlarmCreate`, `AlarmDelete`, and `AlarmList`
//! built-in tools so models can create, inspect, and delete alarms on the
//! current thread.

use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_alarm_create_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "cron_expression".to_string(),
            JsonSchema::String {
                description: Some(
                    "Scheduler expression for the alarm. Supported values are scheduler-specific."
                        .to_string(),
                ),
            },
        ),
        (
            "prompt".to_string(),
            JsonSchema::String {
                description: Some("Prompt to execute when the alarm fires.".to_string()),
            },
        ),
        (
            "run_once".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Optional. When true, delete the alarm after its next execution is claimed."
                        .to_string(),
                ),
            },
        ),
        (
            "delivery".to_string(),
            JsonSchema::String {
                description: Some(
                    "Delivery mode for the alarm. Use `after-turn` or `steer-current-turn`."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "AlarmCreate".to_string(),
        description:
            "Create a thread alarm using a structured scheduler expression, prompt, and delivery mode."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec![
                "cron_expression".to_string(),
                "prompt".to_string(),
                "delivery".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

pub fn create_alarm_delete_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "id".to_string(),
        JsonSchema::String {
            description: Some("Identifier of the alarm to delete.".to_string()),
        },
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: "AlarmDelete".to_string(),
        description: "Delete a thread alarm by id.".to_string(),
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

pub fn create_alarm_list_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "AlarmList".to_string(),
        description: "List thread alarms for the current thread.".to_string(),
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
#[path = "alarm_tool_tests.rs"]
mod tests;
