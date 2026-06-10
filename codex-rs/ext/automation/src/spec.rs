//! Responses API tool definition for scheduled automations.

use std::collections::BTreeMap;

use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use serde_json::json;

pub const AUTOMATION_UPDATE_TOOL_NAME: &str = "automation_update";

pub fn create_automation_update_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "mode".to_string(),
            JsonSchema::string_enum(
                vec![
                    json!("list"),
                    json!("read"),
                    json!("create"),
                    json!("update"),
                    json!("delete"),
                ],
                Some("Required. Operation to perform on automations for this thread.".to_string()),
            ),
        ),
        (
            "automation_id".to_string(),
            JsonSchema::string(Some(
                "Automation id. Required for read, update, and delete.".to_string(),
            )),
        ),
        (
            "kind".to_string(),
            JsonSchema::string_enum(
                vec![json!("cron"), json!("heartbeat")],
                Some("Automation target kind for create, or for retargeting an update.".to_string()),
            ),
        ),
        (
            "name".to_string(),
            JsonSchema::string(Some("Human-readable automation name.".to_string())),
        ),
        (
            "prompt".to_string(),
            JsonSchema::string(Some("Prompt to submit when the automation fires.".to_string())),
        ),
        (
            "rrule".to_string(),
            JsonSchema::string(Some(
                "RFC 5545-style recurrence rule supported by Codex automations.".to_string(),
            )),
        ),
        (
            "model".to_string(),
            JsonSchema::string(Some("Optional model override for cron-created threads.".to_string())),
        ),
        (
            "reasoning_effort".to_string(),
            JsonSchema::string(Some(
                "Optional reasoning effort override for cron-created threads.".to_string(),
            )),
        ),
        (
            "status".to_string(),
            JsonSchema::string_enum(
                vec![json!("ACTIVE"), json!("PAUSED")],
                Some("Automation status.".to_string()),
            ),
        ),
        (
            "cwds".to_string(),
            JsonSchema::array(
                JsonSchema::string(/*description*/ None),
                Some(
                    "Cron working directories. Each path must be absolute and inside this thread's workspace roots."
                        .to_string(),
                ),
            ),
        ),
        (
            "thread_id".to_string(),
            JsonSchema::string(Some(
                "Heartbeat target thread id. Must be this current thread.".to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: AUTOMATION_UPDATE_TOOL_NAME.to_string(),
        description: r#"Create, list, read, update, or delete scheduled automations and heartbeats for the current thread.
Use cron automations for recurring work that should start a fresh thread in one or more working directories.
Use heartbeat automations for recurring follow-ups to this existing thread.
This tool manages definitions only; immediate runNow dispatch is handled by the app-server API in a later automation dispatch path."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            /*required*/ Some(vec!["mode".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}
