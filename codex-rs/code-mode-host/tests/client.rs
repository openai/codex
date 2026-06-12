use std::sync::Arc;

use codex_code_mode::NoopCodeModeSessionDelegate;
use codex_code_mode_client::CodeModeHostCommand;
use codex_code_mode_client::IpcCodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::RuntimeResponse;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn client_executes_code_through_host() {
    let provider = IpcCodeModeSessionProvider::new(CodeModeHostCommand {
        program: codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")
            .expect("resolve codex-code-mode-host binary"),
        args: Vec::new(),
    });
    let session = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .expect("create code mode session");
    let started = session
        .execute(ExecuteRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: "text('hello')".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
        .await
        .expect("execute code through host");
    let cell_id = started.cell_id.clone();

    assert_eq!(
        started
            .initial_response()
            .await
            .expect("receive initial response"),
        RuntimeResponse::Result {
            cell_id,
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "hello".to_string(),
            }],
            error_text: None,
        }
    );
    session.shutdown().await.expect("shut down session");
}
