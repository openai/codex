use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::RuntimeResponse;
use pretty_assertions::assert_eq;

use super::ProcessOwnedCodeModeSession;
use super::ProcessOwnedCodeModeSessionProvider;
use super::resolve_host_program;
use crate::NoopCodeModeSessionDelegate;

#[test]
fn provider_reuses_its_live_process_host() {
    let provider = ProcessOwnedCodeModeSessionProvider::default();

    let first = provider.process_host().expect("owned process host");
    let second = provider.process_host().expect("owned process host");

    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn host_program_override_takes_precedence() {
    assert_eq!(
        resolve_host_program(
            Some("custom-code-mode-host".into()),
            Ok(PathBuf::from("/opt/codex/bin/codex")),
        ),
        PathBuf::from("custom-code-mode-host")
    );
}

#[test]
fn host_program_is_next_to_the_main_executable_even_when_missing() {
    let executable_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };

    assert_eq!(
        resolve_host_program(
            /*override_path*/ None,
            Ok(PathBuf::from("/opt/codex/bin/codex")),
        ),
        PathBuf::from("/opt/codex/bin").join(executable_name)
    );
}

#[test]
fn host_program_falls_back_to_its_name_when_main_executable_is_unknown() {
    let executable_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };

    assert_eq!(
        resolve_host_program(
            /*override_path*/ None,
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "missing executable"
            )),
        ),
        PathBuf::from(executable_name)
    );
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
    let session = ProcessOwnedCodeModeSession::new();

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
