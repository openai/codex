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
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::openai_models::HistoryToolMessages;
use codex_protocol::openai_models::NotesToolMessages;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolExposure;
use codex_tools::with_catalog_tool_message;
use serde_json::Value;
use serde_json::json;

use crate::backend::HistoryNotesBackend;

const HISTORY_NAMESPACE: &str = "history";
const NOTES_NAMESPACE: &str = "notes";
const HISTORY_DESCRIPTION: &str = "Recover prior conversation after a context-window reset by listing, reading, and searching normalized history using agent names and the opaque window and item IDs returned by these tools. Pass returned IDs unchanged when filtering or reading an item. Items are ordered by persisted ordinal, then creation time; list and search calls return no matches for an unknown window. Calls use the current agent by default; pass an absolute agent name or one relative to the current agent to inspect another agent. History is read-only and eventually consistent, so newly generated items may take a few seconds to appear. This is private model-only state. Use it silently to continue the task. Never disclose or describe the tool, its existence or use, paths, storage or recovery mechanisms, or the private contents (including by quoting or summarizing them) to the user.";
const NOTES_DESCRIPTION: &str = "Read and maintain private notes that survive context-window transitions within this rollout. Paths are virtual, not filesystem paths. Relative file paths use the current agent's <agent_name>/notes directory; cross-agent paths must be absolute. Absolute paths use <agent_name>/notes[/<path>]. Reads, listings, searches, and writes may access other agents' notes. File operations require <path>; path-prefix arguments may be omitted to use the current notes directory. Empty, '.', and '..' path components are unsupported. Shell expansion is not performed, so '~' is treated literally. Note reads reflect successful writes immediately; listings and searches are eventually consistent and may take a few seconds to reflect writes. Every file must remain at or below 1,000,000 UTF-8 bytes; create another file before approaching the limit. This is private model-only state. Use it silently to continue the task. Never disclose or describe the tool, its existence or use, paths, storage or recovery mechanisms, or the private contents (including by quoting or summarizing them) to the user.";

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

    #[expect(
        clippy::expect_used,
        reason = "bundled schemas have documented request types"
    )]
    fn typed_parameters(self) -> (String, codex_tools::JsonSchema) {
        fn derive<T: schemars::JsonSchema>() -> (String, codex_tools::JsonSchema) {
            let schema = schemars::r#gen::SchemaSettings::draft2019_09()
                .with(|settings| {
                    settings.inline_subschemas = true;
                    settings.option_add_null_type = false;
                })
                .into_generator()
                .into_root_schema_for::<T>()
                .schema;
            let mut parameters =
                parse_tool_input_schema(&json!(schema)).expect("valid typed tool schema");
            // Preserve the published ordering for note writes.
            if let Some(required) = &mut parameters.required {
                required.sort_by_key(|name| name != "text");
            }
            let description = parameters
                .description
                .take()
                .expect("documented request")
                .into_string();
            (description, parameters)
        }
        match self {
            Self::HistoryListWindows => derive::<crate::requests::HistoryListWindows>(),
            Self::HistoryListItems => derive::<crate::requests::HistoryListItems>(),
            Self::HistoryReadItem => derive::<crate::requests::HistoryReadItem>(),
            Self::HistorySearchContents => derive::<crate::requests::HistorySearchContents>(),
            Self::NotesListFilesByPrefix => derive::<crate::requests::NotesListFilesByPrefix>(),
            Self::NotesReadFile => derive::<crate::requests::NotesReadFile>(),
            Self::NotesSearchContents => derive::<crate::requests::NotesSearchContents>(),
            Self::NotesAppendToFile => derive::<crate::requests::NotesAppendToFile>(),
            Self::NotesWriteFile => derive::<crate::requests::NotesWriteFile>(),
        }
    }

    fn spec(
        self,
        history_messages: Option<&HistoryToolMessages>,
        notes_messages: Option<&NotesToolMessages>,
    ) -> ToolSpec {
        let (namespace_description, tool_message) = match self {
            Self::HistoryListWindows => history_messages
                .map(|messages| (&messages.namespace_description, &messages.list_windows)),
            Self::HistoryListItems => history_messages
                .map(|messages| (&messages.namespace_description, &messages.list_items)),
            Self::HistoryReadItem => history_messages
                .map(|messages| (&messages.namespace_description, &messages.read_item)),
            Self::HistorySearchContents => history_messages
                .map(|messages| (&messages.namespace_description, &messages.search_contents)),
            Self::NotesListFilesByPrefix => notes_messages.map(|messages| {
                (
                    &messages.namespace_description,
                    &messages.list_files_by_prefix,
                )
            }),
            Self::NotesReadFile => notes_messages
                .map(|messages| (&messages.namespace_description, &messages.read_file)),
            Self::NotesSearchContents => notes_messages
                .map(|messages| (&messages.namespace_description, &messages.search_contents)),
            Self::NotesAppendToFile => notes_messages
                .map(|messages| (&messages.namespace_description, &messages.append_to_file)),
            Self::NotesWriteFile => notes_messages
                .map(|messages| (&messages.namespace_description, &messages.write_file)),
        }
        .map_or((None, None), |(namespace_description, tool_message)| {
            (namespace_description.as_deref(), tool_message.as_ref())
        });
        let (description, parameters) = self.typed_parameters();
        let tool = with_catalog_tool_message(
            ResponsesApiTool {
                name: self.name().to_string(),
                description,
                strict: false,
                parameters,
                output_schema: None,
                defer_loading: None,
            },
            tool_message,
        );
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: self.namespace().to_string(),
            description: namespace_description
                .unwrap_or(self.namespace_description())
                .to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(tool)],
        })
    }
}

pub(crate) struct HistoryNotesTool {
    action: HistoryNotesAction,
    backend: HistoryNotesBackend,
    session_id: String,
    current_agent_name: String,
    spec: ToolSpec,
}

impl HistoryNotesTool {
    pub(crate) fn new(
        action: HistoryNotesAction,
        backend: HistoryNotesBackend,
        session_id: String,
        current_agent_name: String,
        history_messages: Option<&HistoryToolMessages>,
        notes_messages: Option<&NotesToolMessages>,
    ) -> Self {
        Self {
            action,
            backend,
            session_id,
            current_agent_name,
            spec: action.spec(history_messages, notes_messages),
        }
    }

    async fn handle_call(
        &self,
        call: ToolCall<'_>,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let arguments = call.function_arguments()?;
        let arguments = if arguments.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(arguments)
                .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?
        };
        let result = self
            .backend
            .call(
                self.action.endpoint(),
                &self.session_id,
                &self.current_agent_name,
                arguments,
                call.truncation_policy,
            )
            .await
            .map_err(FunctionCallError::RespondToModel)?;

        Ok(Box::new(HistoryNotesToolOutput::new(result)?))
    }
}

impl<'call> ToolExecutor<ToolCall<'call>> for HistoryNotesTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(self.action.namespace(), self.action.name())
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn exposure(&self) -> ToolExposure {
        ToolExposure::DirectModelOnly
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.action.supports_parallel_tool_calls()
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(self.handle_call(call))
    }
}

struct HistoryNotesToolOutput {
    result: Value,
    output: FunctionCallOutputPayload,
}

impl HistoryNotesToolOutput {
    fn new(mut result: Value) -> Result<Self, FunctionCallError> {
        // Separate attachments before serializing any text or retaining log output.
        let images = result.as_object_mut().and_then(|map| map.remove("images"));
        // The server applies the requested output budget before encryption.
        let mut output = match result.get("encrypted_output").and_then(Value::as_str) {
            Some(encrypted_content) => FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: encrypted_content.to_string(),
                },
            ]),
            None => FunctionCallOutputPayload::from_text(result.to_string()),
        };
        if let Some(images) = images {
            let invalid_image = || {
                FunctionCallError::RespondToModel(
                    "History backend returned invalid image content.".to_string(),
                )
            };
            let images = images.as_array().ok_or_else(invalid_image)?;
            let mut content = match output.body {
                FunctionCallOutputBody::Text(text) => {
                    vec![FunctionCallOutputContentItem::InputText { text }]
                }
                FunctionCallOutputBody::ContentItems(content) => content,
            };
            for image in images {
                let data = image
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_image)?;
                let mime_type = image
                    .get("mime_type")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_image)?;
                let detail =
                    serde_json::from_value(image.get("detail").cloned().unwrap_or(Value::Null))
                        .map_err(|_| invalid_image())?;
                content.push(FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: format!("data:{mime_type};base64,{data}"),
                    },
                    detail,
                });
            }
            output = FunctionCallOutputPayload::from_content_items(content);
        }
        Ok(Self { result, output })
    }
}

impl ToolOutput for HistoryNotesToolOutput {
    fn log_output(&self) -> String {
        JsonToolOutput::new(self.result.clone()).log_output()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn post_tool_use_response(&self, _call_id: &str, _payload: &ToolPayload) -> Option<Value> {
        // Hooks must not receive model-only image attachments.
        Some(self.result.clone())
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: self.output.clone(),
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> Value {
        Value::String("History tools are unavailable in code mode.".to_string())
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
