use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub fn create_test_sync_tool() -> ToolSpec {
    let barrier_properties = BTreeMap::from([
        (
            "id".to_string(),
            JsonSchema::string(Some(
                "Identifier shared by concurrent calls that should rendezvous".to_string(),
            )),
        ),
        (
            "participants".to_string(),
            JsonSchema::number(Some(
                "Number of tool calls that must arrive before the barrier opens".to_string(),
            )),
        ),
        (
            "timeout_ms".to_string(),
            JsonSchema::number(Some(
                "Maximum barrier wait in milliseconds. Defaults to 1000.".to_string(),
            )),
        ),
    ]);

    let properties = BTreeMap::from([
        (
            "sleep_before_ms".to_string(),
            JsonSchema::number(Some(
                "Delay before any other action. Defaults to no delay.".to_string(),
            )),
        ),
        (
            "sleep_after_ms".to_string(),
            JsonSchema::number(Some(
                "Delay after completing the barrier. Defaults to no delay.".to_string(),
            )),
        ),
        (
            "barrier".to_string(),
            JsonSchema::object(
                barrier_properties,
                Some(vec!["id".to_string(), "participants".to_string()]),
                Some(false.into()),
            ),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "test_sync_tool".to_string(),
        description: "Internal synchronization helper used by Codex integration tests.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, /*required*/ None, Some(false.into())),
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "test_sync_spec_tests.rs"]
mod tests;
