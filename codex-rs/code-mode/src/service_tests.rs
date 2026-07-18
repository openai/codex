use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::CellId;
use super::CodeModeNestedToolCall;
use super::CodeModeSessionDelegate;
use super::InProcessCodeModeSession;
use super::NotificationFuture;
use super::RuntimeResponse;
use super::ToolInvocationFuture;
use super::WaitOutcome;
use super::WaitRequest;
use super::WaitToPendingOutcome;
use super::WaitToPendingRequest;
use super::yield_timeout;
use crate::CodeModeToolKind;
use crate::ExecuteRequest;
use crate::ExecuteToPendingOutcome;
use crate::FunctionCallOutputContentItem;
use crate::ToolDefinition;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[test]
fn yield_timeout_adds_grace_only_at_ten_seconds() {
    assert_eq!(
        yield_timeout(/*yield_time_ms*/ 9_999),
        Duration::from_millis(9_999)
    );
    assert_eq!(
        yield_timeout(/*yield_time_ms*/ 10_000),
        Duration::from_secs(11)
    );
}

#[tokio::test(start_paused = true)]
async fn execute_waits_for_nested_tool_during_yield_grace() {
    let delegate = Arc::new(ReleasableToolDelegate::default());
    let service = InProcessCodeModeSession::with_delegate(delegate.clone());
    let request = ExecuteRequest {
        enabled_tools: vec![echo_tool()],
        source: r#"await tools.echo({}); text("done");"#.to_string(),
        yield_time_ms: Some(10_000),
        ..execute_request("")
    };
    let started = service.execute(request).await.unwrap();
    let response = tokio::spawn(started.initial_response());
    wait_until_tool_started(&delegate).await;
    tokio::time::advance(Duration::from_millis(10_500)).await;
    delegate.release_tool();
    wait_until_finished(&response).await;
    let response = response.await.unwrap().unwrap();

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test(start_paused = true)]
async fn wait_waits_for_nested_tool_during_yield_grace() {
    let delegate = Arc::new(ReleasableToolDelegate::default());
    let service = InProcessCodeModeSession::with_delegate(delegate.clone());
    let initial_response = service
        .execute_to_pending(ExecuteRequest {
            enabled_tools: vec![echo_tool()],
            source: r#"await tools.echo({}); text("done");"#.to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();
    assert_eq!(
        initial_response,
        ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string()],
        }
    );
    let response = service
        .begin_wait(WaitRequest {
            cell_id: cell_id("1"),
            yield_time_ms: 10_000,
        })
        .await;
    let response = tokio::spawn(response);
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(10_500)).await;
    delegate.release_tool();
    wait_until_finished(&response).await;
    let response = response.await.unwrap();

    assert_eq!(
        response.unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
}

async fn wait_until_finished<T>(task: &tokio::task::JoinHandle<T>) {
    for _ in 0..10_000 {
        if task.is_finished() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("code-mode response did not finish while virtual time was held in the grace period");
}

async fn wait_until_tool_started(delegate: &ReleasableToolDelegate) {
    for _ in 0..10_000 {
        if delegate.tool_started.load(Ordering::Acquire) {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("nested code-mode tool did not start");
}

#[derive(Default)]
struct ReleasableToolDelegate {
    tool_release: Notify,
    tool_started: AtomicBool,
}

impl ReleasableToolDelegate {
    fn release_tool(&self) {
        self.tool_release.notify_one();
    }
}

impl CodeModeSessionDelegate for ReleasableToolDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        self.tool_started.store(true, Ordering::Release);
        Box::pin(async move {
            tokio::select! {
                _ = self.tool_release.notified() => Ok(JsonValue::Null),
                _ = cancellation_token.cancelled() => Err("cancelled".to_string()),
            }
        })
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

fn execute_request(source: &str) -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: "call_1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
        yield_time_ms: Some(1),
        max_output_tokens: None,
    }
}

fn cell_id(value: &str) -> CellId {
    CellId::new(value.to_string())
}

fn echo_tool() -> ToolDefinition {
    ToolDefinition {
        name: "echo".to_string(),
        tool_name: ToolName::plain("echo"),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    }
}

async fn execute(service: &InProcessCodeModeSession, request: ExecuteRequest) -> RuntimeResponse {
    service
        .execute(request)
        .await
        .unwrap()
        .initial_response()
        .await
        .unwrap()
}

#[tokio::test]
async fn synchronous_exit_returns_successfully() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"text("before"); exit(); text("after");"#.to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn stored_values_are_shared_between_cells_but_not_sessions() {
    let first_session = InProcessCodeModeSession::new();
    let second_session = InProcessCodeModeSession::new();

    let write_response = execute(
        &first_session,
        ExecuteRequest {
            source: r#"store("key", "visible");"#.to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    let same_session = execute(
        &first_session,
        ExecuteRequest {
            source: r#"text(String(load("key")));"#.to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;
    let other_session = execute(
        &second_session,
        ExecuteRequest {
            source: r#"text(String(load("key")));"#.to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        write_response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            error_text: None,
        }
    );
    assert_eq!(
        same_session,
        RuntimeResponse::Result {
            cell_id: cell_id("2"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "visible".to_string(),
            }],
            error_text: None,
        }
    );
    assert_eq!(
        other_session,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "undefined".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn shutdown_interrupts_cpu_bound_cells() {
    let service = InProcessCodeModeSession::new();

    let cell = service
        .execute(ExecuteRequest {
            source: "while (true) {}".to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();
    assert_eq!(
        cell.initial_response().await.unwrap(),
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );

    tokio::time::timeout(Duration::from_secs(1), service.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn start_cell_rejects_new_cell_after_shutdown_begins() {
    let service = InProcessCodeModeSession::new();
    service.shutdown().await.unwrap();

    let error = service
        .execute(execute_request("text('late');"))
        .await
        .err()
        .unwrap();

    assert_eq!(error, "code mode session is shutting down".to_string());
}

#[tokio::test]
async fn execute_to_pending_returns_completed_for_synchronous_results() {
    let service = InProcessCodeModeSession::new();

    let response = service
        .execute_to_pending(ExecuteRequest {
            source: r#"text("done");"#.to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(
        response,
        ExecuteToPendingOutcome::Completed(RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
}

#[tokio::test]
async fn execute_to_pending_returns_once_the_runtime_is_quiescent() {
    let service = InProcessCodeModeSession::new();

    let response = tokio::time::timeout(
        Duration::from_secs(1),
        service.execute_to_pending(ExecuteRequest {
            source: r#"text("before"); await new Promise(() => {});"#.to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        }),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        response,
        ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            }],
            pending_tool_call_ids: Vec::new(),
        }
    );

    let termination = service.terminate(cell_id("1")).await.unwrap();

    assert_eq!(
        termination,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
}

#[tokio::test]
async fn execute_to_pending_identifies_tool_calls_in_paused_frontier() {
    let service = InProcessCodeModeSession::new();

    let response = service
        .execute_to_pending(ExecuteRequest {
            enabled_tools: vec![echo_tool()],
            source: r#"
await Promise.all([
  tools.echo({ value: "first" }),
  tools.echo({ value: "second" }),
]);
"#
            .to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(
        response,
        ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string(), "tool-2".to_string()],
        }
    );

    let termination = service.terminate(cell_id("1")).await.unwrap();

    assert_eq!(
        termination,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
}

#[tokio::test]
async fn execute_to_pending_excludes_delayed_timeout_tool_calls_until_wait() {
    let service = InProcessCodeModeSession::new();

    let initial_response = service
        .execute_to_pending(ExecuteRequest {
            enabled_tools: vec![echo_tool()],
            source: r#"
setTimeout(() => {
  tools.echo({ value: "delayed" });
}, 1000);
await Promise.all([
  tools.echo({ value: "second" }),
  tools.echo({ value: "third" }),
]);
"#
            .to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(
        initial_response,
        ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string(), "tool-2".to_string()],
        }
    );

    tokio::time::sleep(Duration::from_secs(2)).await;

    let resumed_response = tokio::time::timeout(
        Duration::from_secs(1),
        service.wait_to_pending(WaitToPendingRequest {
            cell_id: cell_id("1"),
        }),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        resumed_response,
        WaitToPendingOutcome::LiveCell(ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-3".to_string()],
        })
    );

    let termination = service.terminate(cell_id("1")).await.unwrap();

    assert_eq!(
        termination,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
}

#[tokio::test]
async fn wait_to_pending_returns_after_resumed_runtime_becomes_quiescent_again() {
    let delegate = Arc::new(ReleasableToolDelegate::default());
    let service = InProcessCodeModeSession::with_delegate(delegate.clone());

    let initial_response = service
        .execute_to_pending(ExecuteRequest {
            enabled_tools: vec![echo_tool()],
            source: r#"
await tools.echo({});
text("after");
await new Promise(() => {});
"#
            .to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(
        initial_response,
        ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string()],
        }
    );

    delegate.release_tool();

    let resumed_response = tokio::time::timeout(
        Duration::from_secs(1),
        service.wait_to_pending(WaitToPendingRequest {
            cell_id: cell_id("1"),
        }),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        resumed_response,
        WaitToPendingOutcome::LiveCell(ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            }],
            pending_tool_call_ids: Vec::new(),
        })
    );

    let termination = service.terminate(cell_id("1")).await.unwrap();

    assert_eq!(
        termination,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
}

#[tokio::test]
async fn wait_to_pending_returns_completed_after_resumed_runtime_finishes() {
    let delegate = Arc::new(ReleasableToolDelegate::default());
    let service = InProcessCodeModeSession::with_delegate(delegate.clone());

    let initial_response = service
        .execute_to_pending(ExecuteRequest {
            enabled_tools: vec![echo_tool()],
            source: r#"
await tools.echo({});
text("done");
"#
            .to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(
        initial_response,
        ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string()],
        }
    );

    delegate.release_tool();

    let resumed_response = tokio::time::timeout(
        Duration::from_secs(1),
        service.wait_to_pending(WaitToPendingRequest {
            cell_id: cell_id("1"),
        }),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        resumed_response,
        WaitToPendingOutcome::LiveCell(ExecuteToPendingOutcome::Completed(
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "done".to_string(),
                }],
                error_text: None,
            }
        ))
    );
}

#[tokio::test]
async fn v8_console_is_not_exposed_on_global_this() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"text(String(Object.hasOwn(globalThis, "console")));"#.to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "false".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn date_locale_string_formats_with_icu_data() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"
const value = new Date("2025-01-02T03:04:05Z")
  .toLocaleString("fr-FR", {
    weekday: "long",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
    timeZone: "UTC",
  });
text(value);
"#
            .to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "jeudi 2 janvier \u{e0} 03:04:05".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn intl_date_time_format_formats_with_icu_data() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"
const formatter = new Intl.DateTimeFormat("fr-FR", {
  weekday: "long",
  month: "long",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
  timeZone: "UTC",
});
text(formatter.format(new Date("2025-01-02T03:04:05Z")));
"#
            .to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "jeudi 2 janvier \u{e0} 03:04:05".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn output_helpers_return_undefined() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"
const returnsUndefined = [
  text("first"),
  image("data:image/png;base64,AAA"),
  audio("data:audio/wav;base64,YXVkaW8="),
  notify("ping"),
].map((value) => value === undefined);
text(JSON.stringify(returnsUndefined));
"#
            .to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![
                FunctionCallOutputContentItem::InputText {
                    text: "first".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,AAA".to_string(),
                    detail: Some(crate::DEFAULT_IMAGE_DETAIL),
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                },
                FunctionCallOutputContentItem::InputText {
                    text: "[true,true,true,true]".to_string(),
                },
            ],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn audio_helper_accepts_audio_url_object_and_raw_mcp_audio_block() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"
audio({
  audio_url: "data:audio/mpeg;base64,YXVkaW8=",
});
audio({
  type: "audio",
  data: "YXVkaW8=",
  mimeType: "audio/wav",
});
"#
            .to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/mpeg;base64,YXVkaW8=".to_string(),
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                },
            ],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn audio_helper_rejects_non_data_urls() {
    for source in [
        r#"audio("https://example.com/audio.wav");"#,
        r#"audio({ audio_url: "file:///tmp/audio.wav" });"#,
    ] {
        let service = InProcessCodeModeSession::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: source.to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                error_text: Some(
                    "Tool call failed: invalid audio output. Pass a base64 data URI instead"
                        .to_string(),
                ),
            }
        );
    }
}

#[tokio::test]
async fn image_helper_accepts_raw_mcp_image_block_with_original_detail() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image({
  type: "image",
  data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  mimeType: "image/png",
  _meta: { "codex/imageDetail": "original" },
});
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

    assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                    detail: Some(crate::ImageDetail::Original),
                }],
                error_text: None,
            }
        );
}

#[tokio::test]
async fn generated_image_helper_appends_image_and_output_hint() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"
generatedImage({
  image_url: "data:image/png;base64,AAA",
  output_hint: "generated image save hint",
});
"#
            .to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,AAA".to_string(),
                    detail: Some(crate::DEFAULT_IMAGE_DETAIL),
                },
                FunctionCallOutputContentItem::InputText {
                    text: "generated image save hint".to_string(),
                },
            ],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn image_helper_second_arg_overrides_explicit_object_detail() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"
image(
  {
    image_url: "data:image/png;base64,AAA",
    detail: "high",
  },
  "original",
);
"#
            .to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: Some(crate::ImageDetail::Original),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn image_helper_second_arg_overrides_raw_mcp_image_detail() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image(
  {
    type: "image",
    data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
    mimeType: "image/png",
    _meta: { "codex/imageDetail": "original" },
  },
  "high",
);
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

    assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                    detail: Some(crate::ImageDetail::High),
                }],
                error_text: None,
            }
        );
}

#[tokio::test]
async fn image_helper_accepts_low_detail() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"
image({
  image_url: "data:image/png;base64,AAA",
  detail: "low",
});
"#
            .to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: Some(crate::ImageDetail::Low),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn image_helpers_reject_remote_urls() {
    for image_url in [
        "http://example.com/image.jpg",
        "https://example.com/image.jpg",
    ] {
        for source in [
            format!("image({image_url:?});"),
            format!("generatedImage({{ image_url: {image_url:?} }});"),
        ] {
            let service = InProcessCodeModeSession::new();

            let response = execute(
                &service,
                ExecuteRequest {
                    source,
                    yield_time_ms: None,
                    ..execute_request("")
                },
            )
            .await;

            assert_eq!(
                    response,
                    RuntimeResponse::Result {
                        cell_id: cell_id("1"),
                        content_items: Vec::new(),
                        error_text: Some(
                            "Tool call failed: remote image URLs are not supported in tool outputs. Pass a base64 data URI instead".to_string(),
                        ),
                    }
                );
        }
    }
}

#[tokio::test]
async fn image_helpers_reject_invalid_image_outputs() {
    let image_url =
        "Error executing tool exec: Expected at least one message to convert to CallToolResult";
    for source in [
        format!("image({image_url:?}, \"original\");"),
        format!("generatedImage({{ image_url: {image_url:?} }});"),
    ] {
        let service = InProcessCodeModeSession::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source,
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                error_text: Some(
                    "Tool call failed: invalid image output. Pass a base64 data URI instead"
                        .to_string(),
                ),
            }
        );
    }
}

#[tokio::test]
async fn image_helper_rejects_unsupported_detail() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
        &service,
        ExecuteRequest {
            source: r#"
image({
  image_url: "data:image/png;base64,AAA",
  detail: "medium",
});
"#
            .to_string(),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            error_text: Some("image detail must be one of: auto, low, high, original".to_string()),
        }
    );
}

#[tokio::test]
async fn image_helper_rejects_raw_mcp_result_container() {
    let service = InProcessCodeModeSession::new();

    let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image({
  content: [
    {
      type: "image",
      data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
      mimeType: "image/png",
      _meta: { "codex/imageDetail": "original" },
    },
  ],
  isError: false,
});
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

    assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                error_text: Some(
                    "image expects a non-empty image URL string, an object with image_url and optional detail, or a raw MCP image block".to_string(),
                ),
            }
        );
}

#[tokio::test]
async fn wait_reports_missing_cell_separately_from_runtime_results() {
    let service = InProcessCodeModeSession::new();

    let response = service
        .wait(WaitRequest {
            cell_id: cell_id("missing"),
            yield_time_ms: 1,
        })
        .await
        .unwrap();

    assert_eq!(
        response,
        WaitOutcome::MissingCell(RuntimeResponse::Result {
            cell_id: cell_id("missing"),
            content_items: Vec::new(),
            error_text: Some("exec cell missing not found".to_string()),
        })
    );
}
