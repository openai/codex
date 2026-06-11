use std::path::PathBuf;
use std::sync::Arc;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::ToolInvocationFuture;
use tokio_util::sync::CancellationToken;

use super::CodeModeHostCommand;
use super::IpcCodeModeSessionProvider;

struct TestDelegate;

impl CodeModeSessionDelegate for TestDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async { Err("unexpected tool invocation".to_string()) })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

#[tokio::test]
async fn create_session_reports_host_spawn_errors() {
    let provider = IpcCodeModeSessionProvider::new(CodeModeHostCommand {
        program: PathBuf::from("codex-code-mode-host-does-not-exist"),
        args: Vec::new(),
    });

    let error = provider
        .create_session(Arc::new(TestDelegate))
        .await
        .err()
        .expect("session creation should fail");

    assert!(error.contains("failed to spawn code-mode host"));
}
