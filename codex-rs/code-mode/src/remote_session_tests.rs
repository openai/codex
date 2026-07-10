use std::sync::Arc;

use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::RuntimeResponse;
use pretty_assertions::assert_eq;

use super::OwnedProcessHost;
use super::ProcessOwnedCodeModeSession;
use super::ProcessOwnedCodeModeSessionProvider;
use crate::NoopCodeModeSessionDelegate;

#[test]
fn provider_reuses_its_live_process_host() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        "codex-code-mode-host-for-test".into(),
    );

    let first = provider.process_host().expect("owned process host");
    let second = provider.process_host().expect("owned process host");

    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn provider_without_host_program_uses_in_process_mode() {
    let provider = ProcessOwnedCodeModeSessionProvider::in_process();

    assert!(provider.process_host().is_none());
}

#[tokio::test]
async fn provider_falls_back_to_in_process_session_when_host_is_missing() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        "codex-code-mode-host-does-not-exist".into(),
    );

    let session = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .expect("missing host should fall back to an in-process session");
    let response = session
        .execute(ExecuteRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: "text('fallback')".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
        .await
        .expect("execute fallback session")
        .initial_response()
        .await
        .expect("read fallback response");

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: codex_code_mode_protocol::CellId::new("1".to_string()),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "fallback".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn shutdown_before_open_does_not_spawn_the_host() {
    let session = ProcessOwnedCodeModeSession::with_process_host(
        Arc::new(NoopCodeModeSessionDelegate),
        Arc::new(OwnedProcessHost::new(
            "codex-code-mode-host-does-not-exist".into(),
        )),
    );

    session.shutdown().await.expect("shutdown session");
    let error = session
        .execute(codex_code_mode_protocol::ExecuteRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: "text('unreachable')".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
        .await
        .err()
        .expect("shutdown session should reject execution");

    assert_eq!(error, "code mode session is shutting down");
}
