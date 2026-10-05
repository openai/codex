//! Verifies partial settlement output survives waits on both host transports.

use std::sync::Arc;
use std::time::Duration;

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
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[path = "support/host.rs"]
mod host;

#[derive(Default)]
struct SlowDelegate {
    release: Notify,
}

impl CodeModeSessionDelegate for SlowDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            tokio::select! {
                _ = self.release.notified() => Ok(json!("slow")),
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
async fn assert_partial_results(provider: &dyn CodeModeSessionProvider) {
    for consumer in [
        "for await (const result of as_settled(promises)) text(result);",
        "await stream_settled(promises, text);",
    ] {
        let delegate = Arc::new(SlowDelegate::default());
        let session = provider.create_session().await.expect("create session");
        let started = session
            .execute(ExecuteRequest {
                tool_call_id: "settled".to_string(),
                enabled_tools: vec![ToolDefinition {
                    name: "slow".to_string(),
                    tool_name: ToolName::plain("slow"),
                    description: String::new(),
                    kind: CodeModeToolKind::Function,
                    input_schema: None,
                    input_schema_max_bytes: None,
                    output_schema: None,
                }],
                source: format!(
                    r#"const promises = [tools.slow({{}}), Promise.resolve("fast"), Promise.reject("failed")];
{consumer}"#
                ),
                yield_time_ms: Some(/*value*/ 10),
                max_output_tokens: None,
            }, delegate.clone(), /*preempt*/ None)
            .await
            .expect("start cell");
        let cell_id = started.cell_id.clone();
        let mut response = started.initial_response().await.expect("initial response");
        let mut partial = Vec::new();
        loop {
            let RuntimeResponse::Yielded { content_items, .. } = response else {
                panic!("cell completed before the slow tool was released: {response:?}");
            };
            partial.extend(content_items.into_iter().map(|item| {
                let FunctionCallOutputContentItem::InputText { text } = item else {
                    panic!("unexpected output: {item:?}");
                };
                serde_json::from_str::<serde_json::Value>(&text).expect("JSON output")
            }));
            if partial.len() >= 2 {
                break;
            }
            let WaitOutcome::LiveCell(next) = session
                .wait(
                    WaitRequest {
                        cell_id: cell_id.clone(),
                        yield_time_ms: 10,
                    },
                    /*preempt*/ None,
                )
                .await
                .expect("wait for partial results")
            else {
                panic!("cell disappeared while the slow tool was pending");
            };
            response = next;
        }
        assert_eq!(
            partial,
            [
                json!({"index": 1, "status": "fulfilled", "value": "fast"}),
                json!({"index": 2, "status": "rejected", "reason": "failed"}),
            ]
        );

        delegate.release.notify_one();
        let completed = session
            .wait(
                WaitRequest {
                    cell_id: cell_id.clone(),
                    yield_time_ms: 5_000,
                },
                /*preempt*/ None,
            )
            .await
            .expect("wait for completion");
        assert_eq!(
            completed,
            WaitOutcome::LiveCell(RuntimeResponse::Result {
                code_mode_host_duration: completed.code_mode_host_duration(),
                cell_id,
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: json!({"index": 0, "status": "fulfilled", "value": "slow"}).to_string(),
                }],
                error_text: None,
            })
        );
        session.shutdown().await.expect("shutdown session");
    }
}

#[tokio::test]
async fn settled_results_stream_over_stdio() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    tokio::time::timeout(
        Duration::from_secs(/*secs*/ 20),
        assert_partial_results(&provider),
    )
    .await
    .expect("settlement streaming timed out");
}

#[tokio::test]
async fn settled_results_stream_over_grpc() {
    let host = host::HostHarness::start("grpc://127.0.0.1:0")
        .await
        .expect("start gRPC host");
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    tokio::time::timeout(
        Duration::from_secs(/*secs*/ 20),
        assert_partial_results(&provider),
    )
    .await
    .expect("settlement streaming timed out");
}
