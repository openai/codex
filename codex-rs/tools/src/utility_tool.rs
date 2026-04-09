use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolOrigin;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_list_dir_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "dir_path".to_string(),
            JsonSchema::string(Some("Absolute path to the directory to list.".to_string())),
        ),
        (
            "offset".to_string(),
            JsonSchema::number(Some(
                "The entry number to start listing from. Must be 1 or greater.".to_string(),
            )),
        ),
        (
            "limit".to_string(),
            JsonSchema::number(Some("The maximum number of entries to return.".to_string())),
        ),
        (
            "depth".to_string(),
            JsonSchema::number(Some(
                "The maximum directory depth to traverse. Must be 1 or greater.".to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_dir".to_string(),
        description:
            "Lists entries in a local directory with 1-indexed entry numbers and simple type labels."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, Some(vec!["dir_path".to_string()]), Some(false.into())),
        output_schema: None,
        origin: ToolOrigin::Native,
    })
}

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
                "Maximum time in milliseconds to wait at the barrier".to_string(),
            )),
        ),
    ]);

    let properties = BTreeMap::from([
        (
            "sleep_before_ms".to_string(),
            JsonSchema::number(Some(
                "Optional delay in milliseconds before any other action".to_string(),
            )),
        ),
        (
            "sleep_after_ms".to_string(),
            JsonSchema::number(Some(
                "Optional delay in milliseconds after completing the barrier".to_string(),
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
        origin: ToolOrigin::Native,
    })
}

#[cfg(test)]
#[path = "utility_tool_tests.rs"]
mod tests;
