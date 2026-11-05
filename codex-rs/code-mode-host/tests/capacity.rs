//! Shared-host capacity, callback completion, and unrelated-session progress.

use std::sync::Arc;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::GrpcCodeModeSessionProvider;
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
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

#[path = "support/host.rs"]
mod host;

struct BlockingDelegate {
    started: Semaphore,
    release: Semaphore,
}

impl CodeModeSessionDelegate for BlockingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            self.started.add_permits(1);
            tokio::select! {
                permit = self.release.acquire() => {
                    permit.map_err(|error| error.to_string())?.forget();
                    Ok(json!("done"))
                }
                _ = cancellation.cancelled() => Err("cancelled".to_string()),
            }
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

#[allow(clippy::expect_used)]
async fn assert_shared_capacity(
    provider: &dyn CodeModeSessionProvider,
    cells: usize,
    calls_per_cell: usize,
) {
    let delegate = Arc::new(BlockingDelegate {
        started: Semaphore::new(/*permits*/ 0),
        release: Semaphore::new(/*permits*/ 0),
    });
    let mut sessions = Vec::new();
    for _ in 0..2 {
        sessions.push(
            provider
                .create_session()
                .await
                .expect("create shared session"),
        );
    }
    let mut pending = Vec::new();
    for index in 0..cells {
        let session = &sessions[index % sessions.len()];
        let request = ExecuteRequest {
            tool_call_id: format!("capacity-{index}"),
            enabled_tools: vec![ToolDefinition {
                name: "block".to_string(),
                tool_name: ToolName::plain("block"),
                description: String::new(),
                kind: CodeModeToolKind::Function,
                input_schema: None,
                input_schema_max_bytes: None,
                output_schema: None,
            }],
            source: format!(
                "const results = await Promise.all(Array.from({{length: {calls_per_cell}}}, () => tools.block({{}}))); text(results.length);"
            ),
            yield_time_ms: Some(1),
            max_output_tokens: None,
        };
        let started = session
            .execute(request, delegate.clone(), /*preempt*/ None)
            .await
            .unwrap_or_else(|error| panic!("admit cell {index}: {error}"));
        let cell_id = started.cell_id.clone();
        let response = started.initial_response().await.expect("initial response");
        assert!(
            matches!(response, RuntimeResponse::Yielded { .. }),
            "{response:?}"
        );
        pending.push((Arc::clone(session), cell_id));
    }
    let callbacks = cells * calls_per_cell;
    delegate
        .started
        .acquire_many(callbacks as u32)
        .await
        .expect("all callbacks started")
        .forget();

    let probe = sessions[0]
        .execute(
            ExecuteRequest {
                tool_call_id: "unrelated-probe".to_string(),
                enabled_tools: Vec::new(),
                source: "text('ready');".to_string(),
                yield_time_ms: Some(60_000),
                max_output_tokens: None,
            },
            delegate.clone(),
            /*preempt*/ None,
        )
        .await
        .expect("unrelated work admitted")
        .initial_response()
        .await
        .expect("unrelated work completed");
    assert!(
        matches!(
            probe,
            RuntimeResponse::Result {
                error_text: None,
                ..
            }
        ),
        "{probe:?}"
    );

    delegate.release.add_permits(callbacks);
    for (session, cell_id) in pending {
        let result = session
            .wait(
                WaitRequest {
                    cell_id,
                    yield_time_ms: 60_000,
                    max_output_tokens: None,
                },
                /*preempt*/ None,
            )
            .await
            .expect("collect callback results");
        let WaitOutcome::LiveCell(RuntimeResponse::Result {
            content_items,
            error_text,
            ..
        }) = result
        else {
            panic!("expected completed cell: {result:?}");
        };
        assert_eq!(
            (content_items, error_text),
            (
                vec![FunctionCallOutputContentItem::InputText {
                    text: calls_per_cell.to_string()
                }],
                None
            )
        );
    }
    for session in sessions {
        session.shutdown().await.expect("close shared session");
    }
}

#[tokio::test]
async fn stdio_shared_sessions_complete_thousands_of_callbacks() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    assert_shared_capacity(&provider, /*cells*/ 256, /*calls_per_cell*/ 16).await;
}

#[tokio::test]
async fn grpc_shared_sessions_complete_thousands_of_callbacks() {
    let host = host::HostHarness::start("grpc://127.0.0.1:0")
        .await
        .expect("host");
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    assert_shared_capacity(&provider, /*cells*/ 256, /*calls_per_cell*/ 16).await;
}

#[tokio::test]
#[ignore = "requires resources for 2,048 live V8 isolates and OS threads"]
async fn grpc_full_host_capacity() {
    let host = host::HostHarness::start("grpc://127.0.0.1:0")
        .await
        .expect("host");
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    assert_shared_capacity(&provider, /*cells*/ 2_047, /*calls_per_cell*/ 2).await;
}
