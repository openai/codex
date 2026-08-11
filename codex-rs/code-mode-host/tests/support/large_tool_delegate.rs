use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::NotificationFuture;
use codex_code_mode::ToolInvocationFuture;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

pub(crate) struct LargeToolResultDelegate {
    pub(crate) started: Semaphore,
    pub(crate) release: Semaphore,
}

impl CodeModeSessionDelegate for LargeToolResultDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        _cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            if invocation.tool_name == ToolName::plain("fast") {
                return Ok(json!({ "value": "isolated" }));
            }
            assert_eq!(invocation.tool_name, ToolName::plain("large"));
            self.started.add_permits(/*n*/ 1);
            self.release
                .acquire()
                .await
                .map_err(|_| "large tool release closed".to_string())?
                .forget();
            Ok(json!({ "value": "x".repeat(8 * 1024 * 1024) }))
        })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}
