use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use serde_json::json;
use std::collections::BTreeMap;

pub const REFLECTIONS_NEW_CONTEXT_WINDOW_TOOL_NAME: &str = "reflections_new_context_window";
pub const REFLECTIONS_GET_CONTEXT_REMAINING_TOOL_NAME: &str = "reflections_get_context_remaining";
pub const REFLECTIONS_LIST_TOOL_NAME: &str = "reflections_list";
pub const REFLECTIONS_READ_TOOL_NAME: &str = "reflections_read";
pub const REFLECTIONS_SEARCH_TOOL_NAME: &str = "reflections_search";
pub const REFLECTIONS_WRITE_NOTE_TOOL_NAME: &str = "reflections_write_note";
pub const REFLECTIONS_LIST_SHARED_NOTES_TOOL_NAME: &str = "reflections_list_shared_notes";
pub const REFLECTIONS_READ_SHARED_NOTE_TOOL_NAME: &str = "reflections_read_shared_note";
pub const REFLECTIONS_SEARCH_SHARED_NOTES_TOOL_NAME: &str = "reflections_search_shared_notes";
pub const REFLECTIONS_WRITE_SHARED_NOTE_TOOL_NAME: &str = "reflections_write_shared_note";

pub fn create_reflections_new_context_window_tool(
    usage_hint: Option<&str>,
    storage_tools_enabled: bool,
) -> ToolSpec {
    let recovery_notes_location = if storage_tools_enabled {
        "with `reflections_write_note`"
    } else {
        "under the Reflections notes directory"
    };
    let mut description = format!(
        "Starts a fresh context window for the same task. Use this after you have saved concise recovery notes {recovery_notes_location} when the current context is large or the next steps should continue from durable logs. This is a control-flow tool: after the tool result is recorded, the current response stops and the next model request resumes from the Reflections handoff in a fresh context window."
    );
    if let Some(usage_hint) = usage_hint {
        description.push_str("\n\n");
        description.push_str(usage_hint);
    }

    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_NEW_CONTEXT_WINDOW_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: empty_parameters(),
        output_schema: None,
    })
}

pub fn create_reflections_get_context_remaining_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_GET_CONTEXT_REMAINING_TOOL_NAME.to_string(),
        description: "Returns the estimated context window size, used tokens, and remaining tokens for the current thread."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: empty_parameters(),
        output_schema: None,
    })
}

pub fn create_reflections_list_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_LIST_TOOL_NAME.to_string(),
        description: "Lists Reflections logs or notes using backend-neutral IDs. Use this to discover explicit log window IDs such as `cw00003` or durable note IDs before reading them.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: object(
            BTreeMap::from([
                (
                    "collection".to_string(),
                    JsonSchema::string_enum(
                        vec![json!("logs"), json!("notes")],
                        Some("Which Reflections collection to list.".to_string()),
                    ),
                ),
                (
                    "start".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive start position. Defaults to 1.".to_string(),
                    )),
                ),
                (
                    "stop".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive stop position. Defaults to 50 and returns at most 200 items.".to_string(),
                    )),
                ),
            ]),
            vec!["collection"],
        ),
        output_schema: None,
    })
}

pub fn create_reflections_read_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_READ_TOOL_NAME.to_string(),
        description: "Reads a Reflections log window or note by explicit ID. Log IDs are `cwNNNNN`; note IDs are simple slugs. The `latest` alias is not supported.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: object(
            BTreeMap::from([
                (
                    "kind".to_string(),
                    JsonSchema::string_enum(
                        vec![json!("log"), json!("note")],
                        Some("Whether to read a log window or note.".to_string()),
                    ),
                ),
                (
                    "id".to_string(),
                    JsonSchema::string(Some(
                        "Explicit log ID such as `cw00003`, or note slug such as `handoff`."
                            .to_string(),
                    )),
                ),
                (
                    "start".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive start entry or line. Defaults to 1.".to_string(),
                    )),
                ),
                (
                    "stop".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive stop entry or line. Defaults to 50 for logs and 1000 for notes."
                            .to_string(),
                    )),
                ),
            ]),
            vec!["kind", "id"],
        ),
        output_schema: None,
    })
}

pub fn create_reflections_search_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_SEARCH_TOOL_NAME.to_string(),
        description: "Searches Reflections notes and/or explicit log windows. Search results include a one-call `read` locator for follow-up with `reflections_read`.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: object(
            BTreeMap::from([
                (
                    "scope".to_string(),
                    JsonSchema::string_enum(
                        vec![json!("all"), json!("logs"), json!("notes")],
                        Some("Where to search.".to_string()),
                    ),
                ),
                (
                    "query".to_string(),
                    JsonSchema::string(Some(
                        "Case-insensitive literal text to search for.".to_string(),
                    )),
                ),
                (
                    "start".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive start result position. Defaults to 1.".to_string(),
                    )),
                ),
                (
                    "stop".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive stop result position. Defaults to 20 and returns at most 100 results."
                            .to_string(),
                    )),
                ),
                (
                    "log_id".to_string(),
                    JsonSchema::string(Some(
                        "Optional explicit log ID such as `cw00003`; only valid when searching logs."
                            .to_string(),
                    )),
                ),
            ]),
            vec!["scope", "query"],
        ),
        output_schema: None,
    })
}

pub fn create_reflections_write_note_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_WRITE_NOTE_TOOL_NAME.to_string(),
        description: "Creates, appends to, or replaces a durable Reflections note. Note IDs are simple slugs, not file paths. A single note can contain at most 65,536 characters after the write.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: object(
            BTreeMap::from([
                (
                    "note_id".to_string(),
                    JsonSchema::string(Some(
                        "Note slug matching ^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$ with no `..`."
                            .to_string(),
                    )),
                ),
                (
                    "operation".to_string(),
                    JsonSchema::string_enum(
                        vec![json!("create_only"), json!("append"), json!("replace")],
                        Some("How to write the note.".to_string()),
                    ),
                ),
                (
                    "content".to_string(),
                    JsonSchema::string(Some(
                        "UTF-8 note content. The individual write and final note are each limited to 65,536 characters."
                            .to_string(),
                    )),
                ),
            ]),
            vec!["note_id", "operation", "content"],
        ),
        output_schema: None,
    })
}

pub fn create_reflections_list_shared_notes_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_LIST_SHARED_NOTES_TOOL_NAME.to_string(),
        description: "Lists shared Reflections notes visible to agents in the same agent tree using backend-neutral note IDs. Use this to discover coordination notes before reading them.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: object(
            BTreeMap::from([
                (
                    "start".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive start position. Defaults to 1.".to_string(),
                    )),
                ),
                (
                    "stop".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive stop position. Defaults to 50 and returns at most 200 items.".to_string(),
                    )),
                ),
            ]),
            Vec::new(),
        ),
        output_schema: None,
    })
}

pub fn create_reflections_read_shared_note_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_READ_SHARED_NOTE_TOOL_NAME.to_string(),
        description: "Reads a shared Reflections note by explicit note ID. Shared notes are for coordination across agents in the same agent tree. The `latest` alias is not supported.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: object(
            BTreeMap::from([
                (
                    "note_id".to_string(),
                    JsonSchema::string(Some(
                        "Shared note slug matching ^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$ with no `..`."
                            .to_string(),
                    )),
                ),
                (
                    "start".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive start line. Defaults to 1.".to_string(),
                    )),
                ),
                (
                    "stop".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive stop line. Defaults to 1000 and returns at most 1000 lines."
                            .to_string(),
                    )),
                ),
            ]),
            vec!["note_id"],
        ),
        output_schema: None,
    })
}

pub fn create_reflections_search_shared_notes_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_SEARCH_SHARED_NOTES_TOOL_NAME.to_string(),
        description: "Searches shared Reflections notes visible to agents in the same agent tree. Search results include a one-call locator for `reflections_read_shared_note`.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: object(
            BTreeMap::from([
                (
                    "query".to_string(),
                    JsonSchema::string(Some(
                        "Case-insensitive literal text to search for.".to_string(),
                    )),
                ),
                (
                    "start".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive start result position. Defaults to 1.".to_string(),
                    )),
                ),
                (
                    "stop".to_string(),
                    JsonSchema::integer(Some(
                        "1-based inclusive stop result position. Defaults to 20 and returns at most 100 results."
                            .to_string(),
                    )),
                ),
            ]),
            vec!["query"],
        ),
        output_schema: None,
    })
}

pub fn create_reflections_write_shared_note_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_WRITE_SHARED_NOTE_TOOL_NAME.to_string(),
        description: "Creates, appends to, or replaces a shared Reflections note visible to agents in the same agent tree. Shared notes are for coordination state that other agents should see. Note IDs are simple slugs, not file paths. A single note can contain at most 65,536 characters after the write.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: object(
            BTreeMap::from([
                (
                    "note_id".to_string(),
                    JsonSchema::string(Some(
                        "Shared note slug matching ^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$ with no `..`."
                            .to_string(),
                    )),
                ),
                (
                    "operation".to_string(),
                    JsonSchema::string_enum(
                        vec![json!("create_only"), json!("append"), json!("replace")],
                        Some("How to write the shared note.".to_string()),
                    ),
                ),
                (
                    "content".to_string(),
                    JsonSchema::string(Some(
                        "UTF-8 note content. The individual write and final shared note are each limited to 65,536 characters."
                            .to_string(),
                    )),
                ),
            ]),
            vec!["note_id", "operation", "content"],
        ),
        output_schema: None,
    })
}

fn empty_parameters() -> JsonSchema {
    object(BTreeMap::new(), Vec::new())
}

fn object(properties: BTreeMap<String, JsonSchema>, required: Vec<&str>) -> JsonSchema {
    JsonSchema::object(
        properties,
        Some(required.into_iter().map(str::to_string).collect()),
        Some(false.into()),
    )
}
