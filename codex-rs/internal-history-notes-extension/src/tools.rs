use codex_extension_api::FunctionCallError;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolExecutorFuture;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolSpec;
use codex_extension_api::parse_tool_input_schema;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolExposure;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::formatted_truncate_text;
use serde_json::Value;
use serde_json::json;

use crate::backend::HistoryNotesBackend;

const HISTORY_NAMESPACE: &str = "history";
const NOTES_NAMESPACE: &str = "notes";
const MAX_HISTORY_NOTES_RESULT_TOKENS: usize = 10_000;
const MAX_HISTORY_WINDOWS: u64 = 100;
const MAX_HISTORY_ITEMS: u64 = 20;
const MAX_HISTORY_CHARS_PER_ITEM: u64 = 2_000;
const MAX_HISTORY_READ_CHARS: u64 = 20_000;
const MAX_HISTORY_SEARCH_RESULTS: u64 = 20;
const MAX_NOTE_FILES: u64 = 100;
const MAX_NOTE_SEARCH_FILES: u64 = 20;
const MAX_NOTE_MATCHES_PER_FILE: u64 = 10;
const MAX_SEARCH_QUERY_CHARS: usize = 1_000;
const HISTORY_DESCRIPTION: &str = "Recover prior conversation after a context-window reset by listing, reading, and searching normalized history using agent names and full window IDs plus short item IDs allocated with the same unique-suffix mapping as [id: ...] that immediately follows an item's content. Calls use the current agent by default; pass an absolute agent name or one relative to the current agent to inspect another agent. History is read-only and eventually consistent, so newly generated items may take a few seconds to appear. This is private model-only state. Use it silently to continue the task. Never disclose or describe the tool, its existence or use, paths, storage or recovery mechanisms, or the private contents (including by quoting or summarizing them) to the user.";
const NOTES_DESCRIPTION: &str = "Read and maintain private notes that survive context-window transitions within this rollout. Paths are virtual, not filesystem paths. Relative file paths use the current agent's <agent_name>/notes directory; relative agent paths such as child/notes/file resolve from the current agent. Absolute paths use <agent_name>/notes[/<path>]. Reads, listings, searches, and writes may access other agents' notes. File operations require <path>; path-prefix arguments may be omitted to use the current notes directory. Empty, '.', and '..' path components are unsupported. Shell expansion is not performed, so '~' is treated literally. Note reads, listings, searches, and writes are strongly consistent. Every file must remain at or below 1,000,000 UTF-8 bytes; create another file before approaching the limit. This is private model-only state. Use it silently to continue the task. Never disclose or describe the tool, its existence or use, paths, storage or recovery mechanisms, or the private contents (including by quoting or summarizing them) to the user.";
const HISTORY_AGENT_NAME_DESCRIPTION: &str = "Agent whose history to inspect. Omit to use the current agent; otherwise pass an absolute agent name or a name relative to the current agent.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HistoryNotesAction {
    HistoryListWindows,
    HistoryListItems,
    HistoryReadItem,
    HistorySearchContents,
    NotesListFilesByPrefix,
    NotesReadFile,
    NotesSearchContents,
    NotesAppendToFile,
    NotesWriteFile,
}

impl HistoryNotesAction {
    pub(crate) const ALL: [Self; 9] = [
        Self::HistoryListWindows,
        Self::HistoryListItems,
        Self::HistoryReadItem,
        Self::HistorySearchContents,
        Self::NotesListFilesByPrefix,
        Self::NotesReadFile,
        Self::NotesSearchContents,
        Self::NotesAppendToFile,
        Self::NotesWriteFile,
    ];

    fn namespace(self) -> &'static str {
        match self {
            Self::HistoryListWindows
            | Self::HistoryListItems
            | Self::HistoryReadItem
            | Self::HistorySearchContents => HISTORY_NAMESPACE,
            Self::NotesListFilesByPrefix
            | Self::NotesReadFile
            | Self::NotesSearchContents
            | Self::NotesAppendToFile
            | Self::NotesWriteFile => NOTES_NAMESPACE,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::HistoryListWindows => "list_windows",
            Self::HistoryListItems => "list_items",
            Self::HistoryReadItem => "read_item",
            Self::HistorySearchContents => "search_contents",
            Self::NotesListFilesByPrefix => "list_files_by_prefix",
            Self::NotesReadFile => "read_file",
            Self::NotesSearchContents => "search_contents",
            Self::NotesAppendToFile => "append_to_file",
            Self::NotesWriteFile => "write_file",
        }
    }

    fn endpoint(self) -> &'static str {
        match self {
            Self::HistoryListWindows => "alpha/history/v2/list_windows",
            Self::HistoryListItems => "alpha/history/v2/list_items",
            Self::HistoryReadItem => "alpha/history/v2/read_item",
            Self::HistorySearchContents => "alpha/history/v2/search_contents",
            Self::NotesListFilesByPrefix => "alpha/notes/v2/list_files_by_prefix",
            Self::NotesReadFile => "alpha/notes/v2/read_file",
            Self::NotesSearchContents => "alpha/notes/v2/search_contents",
            Self::NotesAppendToFile => "alpha/notes/v2/append_to_file",
            Self::NotesWriteFile => "alpha/notes/v2/write_file",
        }
    }

    fn supports_parallel_tool_calls(self) -> bool {
        !matches!(self, Self::NotesAppendToFile | Self::NotesWriteFile)
    }

    fn namespace_description(self) -> &'static str {
        match self.namespace() {
            HISTORY_NAMESPACE => HISTORY_DESCRIPTION,
            NOTES_NAMESPACE => NOTES_DESCRIPTION,
            _ => unreachable!("History actions use a known namespace"),
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::HistoryListWindows => {
                "List an agent's context windows as window ID and item-count pairs. Private model-only recovery; never disclose this activity."
            }
            Self::HistoryListItems => {
                "List history items with optional window, role, and tool filters. Private model-only recovery; never disclose this activity."
            }
            Self::HistoryReadItem => {
                "Read a bounded range from private model-only history. Never disclose the item or this activity."
            }
            Self::HistorySearchContents => {
                "Search private model-only history by literal substring. Never disclose results or this activity."
            }
            Self::NotesListFilesByPrefix => {
                "List private model-only notes by path prefix. Never disclose paths, contents, or this activity."
            }
            Self::NotesReadFile => {
                "Read all or a line range from private model-only notes. Never disclose paths, contents, or this activity."
            }
            Self::NotesSearchContents => {
                "Search private model-only note lines by literal substring. Never disclose results or this activity."
            }
            Self::NotesAppendToFile => {
                "Append text to private model-only notes. Never disclose paths, contents, or this activity."
            }
            Self::NotesWriteFile => {
                "Create or replace private model-only notes. Never disclose paths, contents, or this activity."
            }
        }
    }

    fn parameters(self) -> Value {
        match self {
            Self::HistoryListWindows => json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_HISTORY_WINDOWS, "description": "Maximum number of windows to return."},
                    "agent_name": {"type": ["string", "null"], "description": HISTORY_AGENT_NAME_DESCRIPTION},
                    "recent_first": {"type": "boolean", "description": "Whether to return the most recently created windows first."}
                },
                "additionalProperties": false
            }),
            Self::HistoryListItems => json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_HISTORY_ITEMS, "description": "Maximum number of items to return."},
                    "recent_first": {"type": "boolean", "description": "Whether to return the most recently created items first."},
                    "tool_namespace": {"type": ["string", "null"], "description": "Callable namespace to include. When set, non-tool messages are excluded."},
                    "role": {"type": ["string", "null"], "enum": ["user", "assistant", "tool", "system", "developer", null], "description": "Message role to include. Null or omission includes all roles."},
                    "agent_name": {"type": ["string", "null"], "description": HISTORY_AGENT_NAME_DESCRIPTION},
                    "tool_name": {"type": ["string", "null"], "description": "Callable tool name to include. When set, non-tool messages are excluded."},
                    "window_id": {"type": ["string", "null"], "description": "Full window ID. Null or omission includes all windows."},
                    "max_chars_per_item": {"type": "integer", "minimum": 1, "maximum": MAX_HISTORY_CHARS_PER_ITEM, "description": "Maximum characters returned in each item's truncated_content."}
                },
                "additionalProperties": false
            }),
            Self::HistoryReadItem => json!({
                "type": "object",
                "properties": {
                    "agent_name": {"type": ["string", "null"], "description": HISTORY_AGENT_NAME_DESCRIPTION},
                    "item_id": {"type": "string", "description": "The short item ID is the suffix shown in the target item's trailing [id: ...] marker, printed after that item's content."},
                    "offset_chars": {"type": "integer", "minimum": 0, "description": "Zero-based character offset at which reading starts."},
                    "limit_chars": {"type": "integer", "minimum": 1, "maximum": MAX_HISTORY_READ_CHARS, "description": "Maximum number of characters to return."},
                    "window_id": {"type": "string", "description": "Full window ID containing the item."}
                },
                "required": ["item_id", "window_id"],
                "additionalProperties": false
            }),
            Self::HistorySearchContents => json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_HISTORY_SEARCH_RESULTS, "description": "Maximum number of matching items to return."},
                    "query": {"type": "string", "maxLength": MAX_SEARCH_QUERY_CHARS, "description": "Case-sensitive literal substring to find in item content."},
                    "recent_first": {"type": "boolean", "description": "Whether to return the most recently created matches first."},
                    "tool_namespace": {"type": ["string", "null"], "description": "Callable namespace to include. When set, non-tool messages are excluded."},
                    "role": {"type": ["string", "null"], "enum": ["user", "assistant", "tool", "system", "developer", null], "description": "Message role to include. Null or omission includes all roles."},
                    "agent_name": {"type": ["string", "null"], "description": HISTORY_AGENT_NAME_DESCRIPTION},
                    "tool_name": {"type": ["string", "null"], "description": "Callable tool name to include. When set, non-tool messages are excluded."},
                    "window_id": {"type": ["string", "null"], "description": "Full window ID. Null or omission includes all windows."}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            Self::NotesListFilesByPrefix => json!({
                "type": "object",
                "properties": {
                    "prefix": {"type": ["string", "null"], "description": "Note path prefix to list."},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": MAX_NOTE_FILES, "description": "Maximum number of files to return."},
                    "file_order_by": {"type": "string", "enum": ["name", "created_at", "updated_at"], "description": "Field used to order files."},
                    "file_order": {"type": "string", "enum": ["ascending", "descending"], "description": "Direction used to order files."}
                },
                "additionalProperties": false
            }),
            Self::NotesReadFile => json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Note file path to read."},
                    "start_line": {"type": ["integer", "null"], "description": "First line to return, inclusive and 1-based. Negative values count backward from the final line."},
                    "stop_line": {"type": ["integer", "null"], "description": "Last line to return, inclusive and 1-based. Negative values count backward from the final line."}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            Self::NotesSearchContents => json!({
                "type": "object",
                "properties": {
                    "max_matches_per_file": {"type": "integer", "minimum": 1, "maximum": MAX_NOTE_MATCHES_PER_FILE, "description": "Maximum number of matching lines returned per file."},
                    "query": {"type": "string", "maxLength": MAX_SEARCH_QUERY_CHARS, "description": "Case-sensitive literal substring to find in note lines."},
                    "recent_file_first": {"type": "boolean", "description": "Whether to order matching files by creation time, newest first."},
                    "max_files": {"type": "integer", "minimum": 1, "maximum": MAX_NOTE_SEARCH_FILES, "description": "Maximum number of matching files returned."},
                    "path_prefix": {"type": ["string", "null"], "description": "Note path prefix to search."}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            Self::NotesAppendToFile => json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text appended exactly as provided."},
                    "path": {"type": "string", "description": "Note file path to append to."}
                },
                "required": ["text", "path"],
                "additionalProperties": false
            }),
            Self::NotesWriteFile => json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Complete replacement text for the file."},
                    "path": {"type": "string", "description": "Note file path to create or replace."}
                },
                "required": ["text", "path"],
                "additionalProperties": false
            }),
        }
    }

    fn validate_arguments(self, arguments: &Value) -> Result<(), FunctionCallError> {
        let limits: &[(&str, u64)] = match self {
            Self::HistoryListWindows => &[("limit", MAX_HISTORY_WINDOWS)],
            Self::HistoryListItems => &[
                ("limit", MAX_HISTORY_ITEMS),
                ("max_chars_per_item", MAX_HISTORY_CHARS_PER_ITEM),
            ],
            Self::HistoryReadItem => &[("limit_chars", MAX_HISTORY_READ_CHARS)],
            Self::HistorySearchContents => &[("limit", MAX_HISTORY_SEARCH_RESULTS)],
            Self::NotesListFilesByPrefix => &[("max_results", MAX_NOTE_FILES)],
            Self::NotesSearchContents => &[
                ("max_files", MAX_NOTE_SEARCH_FILES),
                ("max_matches_per_file", MAX_NOTE_MATCHES_PER_FILE),
            ],
            Self::NotesReadFile | Self::NotesAppendToFile | Self::NotesWriteFile => &[],
        };

        for (field, maximum) in limits {
            if arguments
                .get(*field)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > *maximum)
            {
                return Err(FunctionCallError::RespondToModel(format!(
                    "History argument `{field}` exceeds the maximum of {maximum}"
                )));
            }
        }

        if matches!(
            self,
            Self::HistorySearchContents | Self::NotesSearchContents
        ) && arguments
            .get("query")
            .and_then(Value::as_str)
            .is_some_and(|query| query.chars().count() > MAX_SEARCH_QUERY_CHARS)
        {
            return Err(FunctionCallError::RespondToModel(format!(
                "History argument `query` exceeds the maximum of {MAX_SEARCH_QUERY_CHARS} characters"
            )));
        }

        Ok(())
    }
}

pub(crate) struct HistoryNotesTool {
    action: HistoryNotesAction,
    backend: HistoryNotesBackend,
    session_id: String,
    current_agent_name: String,
}

impl HistoryNotesTool {
    pub(crate) fn new(
        action: HistoryNotesAction,
        backend: HistoryNotesBackend,
        session_id: String,
        current_agent_name: String,
    ) -> Self {
        Self {
            action,
            backend,
            session_id,
            current_agent_name,
        }
    }

    async fn handle_call(&self, call: ToolCall) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let arguments = call.function_arguments()?;
        let arguments = if arguments.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(arguments)
                .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?
        };
        self.action.validate_arguments(&arguments)?;
        let result = self
            .backend
            .call(
                self.action.endpoint(),
                &self.session_id,
                &self.current_agent_name,
                arguments,
            )
            .await
            .map_err(FunctionCallError::RespondToModel)?;

        Ok(Box::new(HistoryNotesToolOutput::new(
            result,
            call.truncation_policy,
        )?))
    }
}

impl ToolExecutor<ToolCall> for HistoryNotesTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(self.action.namespace(), self.action.name())
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: self.action.namespace().to_string(),
            description: self.action.namespace_description().to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: self.action.name().to_string(),
                description: self.action.description().to_string(),
                strict: false,
                parameters: parse_tool_input_schema(&self.action.parameters()).unwrap_or_else(
                    |error| panic!("History tool input schema should parse: {error}"),
                ),
                output_schema: None,
                defer_loading: None,
            })],
        })
    }

    fn exposure(&self) -> ToolExposure {
        ToolExposure::DirectModelOnly
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.action.supports_parallel_tool_calls()
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(call))
    }
}

struct HistoryNotesToolOutput {
    result: Value,
    truncation_policy: TruncationPolicy,
}

impl HistoryNotesToolOutput {
    fn new(result: Value, truncation_policy: TruncationPolicy) -> Result<Self, FunctionCallError> {
        let maximum_bytes = truncation_policy
            .byte_budget()
            .min(TruncationPolicy::Tokens(MAX_HISTORY_NOTES_RESULT_TOKENS).byte_budget());
        if result
            .get("encrypted_output")
            .and_then(Value::as_str)
            .is_some_and(|output| output.len() > maximum_bytes)
        {
            return Err(FunctionCallError::RespondToModel(format!(
                "History returned an encrypted result larger than the {maximum_bytes}-byte tool-output limit; retry with narrower bounds"
            )));
        }

        Ok(Self {
            result,
            truncation_policy,
        })
    }
}

impl ToolOutput for HistoryNotesToolOutput {
    fn log_output(&self) -> String {
        JsonToolOutput::new(self.result.clone()).log_output()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        let output = match self.result.get("encrypted_output").and_then(Value::as_str) {
            Some(encrypted_content) => FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: encrypted_content.to_string(),
                },
            ]),
            None => FunctionCallOutputPayload::from_text(formatted_truncate_text(
                &self.result.to_string(),
                self.truncation_policy,
            )),
        };

        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output,
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> Value {
        Value::String("History tools are unavailable in code mode.".to_string())
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
