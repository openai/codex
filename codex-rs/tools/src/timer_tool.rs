//! Responses API tool specs for thread-local persistent timer management.
//!
//! These specs expose the `create_timer`, `delete_timer`, and `list_timers`
//! built-in tools.

use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_timer_tool() -> ToolSpec {
    let trigger_properties = BTreeMap::from([
        (
            "kind".to_string(),
            JsonSchema::string(Some(
                "Trigger kind. Use `delay` or `schedule`.".to_string(),
            )),
        ),
        (
            "seconds".to_string(),
            JsonSchema::number(Some(
                "Delay trigger seconds from creation time.".to_string(),
            )),
        ),
        (
            "repeat".to_string(),
            JsonSchema::boolean(Some(
                "Delay trigger recurrence flag. With seconds 0, repeat means run whenever the thread is idle."
                    .to_string(),
            )),
        ),
        (
            "dtstart".to_string(),
            JsonSchema::string(Some(
                "Schedule trigger floating local datetime in YYYY-MM-DDTHH:MM:SS format."
                    .to_string(),
            )),
        ),
        (
            "rrule".to_string(),
            JsonSchema::string(Some("Schedule trigger RRULE string.".to_string())),
        ),
    ]);
    let properties = BTreeMap::from([
        (
            "trigger".to_string(),
            JsonSchema::object(
                trigger_properties,
                Some(vec!["kind".to_string()]),
                Some(false.into()),
            ),
        ),
        (
            "content".to_string(),
            JsonSchema::string(Some(
                "Message content to execute when the timer fires.".to_string(),
            )),
        ),
        (
            "meta".to_string(),
            JsonSchema::object(BTreeMap::new(), None, Some(true.into())),
        ),
        (
            "delivery".to_string(),
            JsonSchema::string(Some(
                "Delivery mode for the timer. Use `after-turn` or `steer-current-turn`."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "create_timer".to_string(),
        description:
            "Create a thread timer using a structured trigger, message content, and delivery mode."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec![
                "trigger".to_string(),
                "content".to_string(),
                "delivery".to_string(),
            ]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

pub fn create_delete_timer_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "id".to_string(),
        JsonSchema::string(Some("Identifier of the timer to delete.".to_string())),
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: "delete_timer".to_string(),
        description: "Delete a thread timer by id.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["id".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

pub fn create_list_timers_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "list_timers".to_string(),
        description: "List thread timers for the current thread.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "timer_tool_tests.rs"]
mod tests;
