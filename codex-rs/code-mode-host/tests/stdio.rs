#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::sync::Mutex;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSession;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::NotificationFuture;
use codex_code_mode::ProcessOwnedCodeModeSessionProvider;
use codex_code_mode::RuntimeResponse;
use codex_code_mode::ToolDefinition;
use codex_code_mode::ToolInvocationFuture;
use codex_code_mode::WaitOutcome;
use codex_code_mode::WaitRequest;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct RecordingDelegate {
    invocations: Mutex<Vec<CodeModeNestedToolCall>>,
    notifications: Mutex<Vec<(String, CellId, String)>>,
    closed_cells: Mutex<Vec<CellId>>,
}

impl CodeModeSessionDelegate for RecordingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        self.invocations
            .lock()
            .expect("invocations lock")
            .push(invocation);
        Box::pin(async { Ok(json!({ "value": "output" })) })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        self.notifications
            .lock()
            .expect("notifications lock")
            .push((call_id, cell_id, text));
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.closed_cells
            .lock()
            .expect("closed cells lock")
            .push(cell_id.clone());
    }
}

fn cell_id(value: &str) -> CellId {
    CellId::new(value.to_string())
}

fn execute_request(source: &str) -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
        yield_time_ms: None,
        max_output_tokens: None,
    }
}

async fn execute(session: &Arc<dyn CodeModeSession>, request: ExecuteRequest) -> RuntimeResponse {
    session
        .execute(request)
        .await
        .expect("start execution")
        .initial_response()
        .await
        .expect("initial response")
}

#[tokio::test]
async fn remote_session_persists_values_forwards_delegates_and_controls_cells() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .expect("create remote session");

    assert_eq!(
        execute(&session, execute_request(r#"store("key", "persisted");"#),).await,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            error_text: None,
        }
    );

    let mut callback_request = execute_request(
        r#"
const result = await tools.echo({ value: String(load("key")) });
notify("notice");
text(result.value);
"#,
    );
    callback_request.tool_call_id = "call-2".to_string();
    callback_request.enabled_tools = vec![ToolDefinition {
        name: "echo".to_string(),
        tool_name: ToolName::plain("echo"),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    }];
    assert_eq!(
        execute(&session, callback_request).await,
        RuntimeResponse::Result {
            cell_id: cell_id("2"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "output".to_string(),
            }],
            error_text: None,
        }
    );
    assert_eq!(
        *delegate.invocations.lock().expect("invocations lock"),
        vec![CodeModeNestedToolCall {
            cell_id: cell_id("2"),
            runtime_tool_call_id: "tool-1".to_string(),
            tool_name: ToolName::plain("echo"),
            tool_kind: CodeModeToolKind::Function,
            input: Some(json!({ "value": "persisted" })),
        }]
    );
    assert_eq!(
        *delegate.notifications.lock().expect("notifications lock"),
        vec![("call-2".to_string(), cell_id("2"), "notice".to_string())]
    );

    let mut pending_request = execute_request("await new Promise(() => {});");
    pending_request.tool_call_id = "call-3".to_string();
    pending_request.yield_time_ms = Some(1);
    assert_eq!(
        execute(&session, pending_request).await,
        RuntimeResponse::Yielded {
            cell_id: cell_id("3"),
            content_items: Vec::new(),
        }
    );
    assert_eq!(
        session
            .wait(WaitRequest {
                cell_id: cell_id("3"),
                yield_time_ms: 1,
            })
            .await
            .expect("wait for cell"),
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: cell_id("3"),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        session
            .terminate(cell_id("3"))
            .await
            .expect("terminate cell"),
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("3"),
            content_items: Vec::new(),
        })
    );

    session.shutdown().await.expect("shutdown remote session");
    assert_eq!(
        *delegate.closed_cells.lock().expect("closed cells lock"),
        vec![cell_id("1"), cell_id("2"), cell_id("3")]
    );
}
