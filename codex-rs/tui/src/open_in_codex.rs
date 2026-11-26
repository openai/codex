//! Model-facing requests to show files in the calling conversation's sidebar.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::app_server_session::fs::AppServerFileSystem;
use crate::backend_id::BackendId;
use crate::markdown_render::SourceLocation;
use codex_app_server_client::AppServerPath;
use codex_app_server_client::AppServerRequestHandle;
use codex_protocol::ThreadId;
use codex_utils_path_uri::PathUri;
use rmcp::ErrorData as McpError;
use rmcp::model::CallToolResult;
use rmcp::model::Tool;
use serde::Deserialize;
use serde_json::json;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

pub(crate) const NAME: &str = "open_in_codex";
pub(crate) const MAX_PENDING_DOCUMENTS: usize = 32;

#[derive(Debug)]
pub(crate) struct DocumentRequest {
    pub(crate) original_path: AppServerPath,
    pub(crate) path: PathUri,
    pub(crate) location: Option<SourceLocation>,
    pub(crate) _pending: OwnedSemaphorePermit,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    target: Target,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Target {
    File {
        path: String,
        #[serde(default, deserialize_with = "deserialize_line")]
        line: Option<NonZeroUsize>,
    },
}

fn deserialize_line<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<NonZeroUsize>, D::Error> {
    NonZeroUsize::deserialize(deserializer).map(Some)
}

pub(crate) fn tool() -> Tool {
    let schema = rmcp::object!({
        "type": "object",
        "additionalProperties": false,
        "required": ["target"],
        "properties": {
            "target": {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "path"],
                "properties": {
                    "type": {"type": "string", "const": "file"},
                    "path": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 4096,
                        "description": "File path, absolute or relative to the calling thread's working directory."
                    },
                    "line": {"type": "integer", "minimum": 1, "description": "One-based source line."}
                }
            }
        }
    });
    Tool::new(
        NAME,
        "Show a workspace file in the calling thread's Codex sidebar. Use when showing a document, source location, or image would help the user. The request may queue until the thread is visible. This only opens the UI. Use file tools to inspect the content.",
        Arc::new(schema),
    )
}

pub(crate) async fn execute(
    request_handle: AppServerRequestHandle,
    events: &AppEventSender,
    pending_slots: Arc<Semaphore>,
    backend: BackendId,
    thread_id: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, McpError> {
    let thread_id = ThreadId::from_string(thread_id)
        .map_err(|_| McpError::invalid_params("Invalid calling thread ID", None))?;
    let Arguments {
        target: Target::File { path, line },
    } = serde_json::from_value(arguments).map_err(|_| {
        McpError::invalid_params(
            "Expected a file target with a path and optional positive line",
            None,
        )
    })?;
    if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
        return Err(McpError::invalid_params("Invalid file path", None));
    }
    let pending = pending_slots.try_acquire_owned().map_err(|_| {
        McpError::internal_error("The model preview queue is full. Try again later.", None)
    })?;
    let original_path = AppServerPath::from_app_server(path);
    let path = AppServerFileSystem { request_handle }
        .resolve_thread_file(thread_id, &original_path)
        .await
        .map_err(|_| McpError::invalid_params("File preview is unavailable for this path", None))?;
    let location = line.map(|line| SourceLocation::single(line.get(), /*column*/ None));
    events
        .app_event_tx
        .send(AppEvent::OpenModelSidebarDocument {
            backend,
            thread_id,
            document: DocumentRequest {
                original_path,
                path,
                location,
                _pending: pending,
            },
        })
        .map_err(|_| McpError::internal_error("The TUI is no longer accepting requests", None))?;
    Ok(CallToolResult::structured(json!({
        "status": "queued",
        "threadId": thread_id,
    })))
}
