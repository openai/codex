use std::sync::Mutex;
use std::sync::PoisonError;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::NotificationFuture;
use codex_code_mode::ToolInvocationFuture;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(crate) struct RecordingDelegate {
    pub(crate) invocations: Mutex<Vec<CodeModeNestedToolCall>>,
    pub(crate) notifications: Mutex<Vec<(String, CellId, String)>>,
    pub(crate) closed_cells: Mutex<Vec<CellId>>,
}

impl CodeModeSessionDelegate for RecordingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        _cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        self.invocations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(invocation);
        Box::pin(async { Ok(json!({ "value": "output" })) })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        _cancellation: CancellationToken,
    ) -> NotificationFuture<'a> {
        self.notifications
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((call_id, cell_id, text));
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.closed_cells
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(cell_id.clone());
    }
}

pub(crate) fn cell_id(value: &str) -> CellId {
    CellId::new(value.to_string())
}
