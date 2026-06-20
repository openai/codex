use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use super::CellId;
use super::CodeModeNestedToolCall;
use super::CodeModeService;
use super::CodeModeSessionDelegate;
use super::NotificationFuture;
use super::ObserveOutcome;
use super::ObserveRequest;
use super::ObserveToPendingOutcome;
use super::ObserveToPendingRequest;
use super::PendingOutcome;
use super::ProtocolDelegate;
use super::RuntimeResponse;
use super::TerminateOutcome;
use super::ToolInvocationFuture;
use super::missing_cell_response;
use super::observe_outcome;
use super::pending_outcome;
use super::protocol_cell_id;
use super::runtime;
use super::runtime_cell_id;
use super::runtime_request;
use crate::CodeModeToolKind;
use crate::CreateCellRequest;
use crate::FunctionCallOutputContentItem;
use crate::ToolDefinition;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use serde_json::json;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct ReleasableToolDelegate {
    tool_release: Notify,
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
}

#[derive(Debug, PartialEq)]
enum RecordedDelegateCall {
    Tool {
        invocation: CodeModeNestedToolCall,
        cancellation_requested: bool,
    },
    Notification {
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_requested: bool,
    },
}

struct RecordingDelegate {
    calls: Mutex<Vec<RecordedDelegateCall>>,
    tool_results: Mutex<VecDeque<Result<JsonValue, String>>>,
    notification_results: Mutex<VecDeque<Result<(), String>>>,
}

impl RecordingDelegate {
    fn new(
        tool_results: Vec<Result<JsonValue, String>>,
        notification_results: Vec<Result<(), String>>,
    ) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            tool_results: Mutex::new(tool_results.into()),
            notification_results: Mutex::new(notification_results.into()),
        }
    }

    fn take_calls(&self) -> Vec<RecordedDelegateCall> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
}

impl CodeModeSessionDelegate for RecordingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(RecordedDelegateCall::Tool {
                invocation,
                cancellation_requested: cancellation_token.is_cancelled(),
            });
            self.tool_results
                .lock()
                .unwrap()
                .pop_front()
                .expect("test must provide one result per tool call")
        })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap()
                .push(RecordedDelegateCall::Notification {
                    call_id,
                    cell_id,
                    text,
                    cancellation_requested: cancellation_token.is_cancelled(),
                });
            self.notification_results
                .lock()
                .unwrap()
                .pop_front()
                .expect("test must provide one result per notification")
        })
    }
}

fn execute_request(source: &str) -> CreateCellRequest {
    CreateCellRequest {
        idempotency_key: format!("call_1:{source}"),
        tool_call_id: "call_1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
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

async fn execute(service: &CodeModeService, request: CreateCellRequest) -> RuntimeResponse {
    execute_with_yield_time(service, request, /*yield_time_ms*/ 10_000).await
}

async fn execute_with_yield_time(
    service: &CodeModeService,
    request: CreateCellRequest,
    yield_time_ms: u64,
) -> RuntimeResponse {
    let cell_id = service.create_cell(request).await.unwrap();
    service
        .observe(ObserveRequest {
            idempotency_key: format!("observe:{cell_id}"),
            cell_id,
            yield_time_ms,
        })
        .await
        .unwrap()
        .into()
}

async fn create_and_observe_to_pending(
    service: &CodeModeService,
    request: CreateCellRequest,
) -> Result<PendingOutcome, String> {
    let cell_id = service.create_pausable_cell(request).await?;
    match service
        .observe_to_pending(ObserveToPendingRequest { cell_id })
        .await?
    {
        ObserveToPendingOutcome::LiveCell(outcome) => Ok(outcome),
        ObserveToPendingOutcome::MissingCell(response) => Ok(PendingOutcome::Completed(response)),
    }
}

#[tokio::test]
async fn synchronous_exit_returns_successfully() {
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
            source: r#"text("before"); exit(); text("after");"#.to_string(),
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
    let first_session = CodeModeService::new();
    let second_session = CodeModeService::new();

    let write_response = execute(
        &first_session,
        CreateCellRequest {
            idempotency_key: "write-shared-value".to_string(),
            source: r#"store("key", "visible");"#.to_string(),
            ..execute_request("")
        },
    )
    .await;

    let same_session = execute(
        &first_session,
        CreateCellRequest {
            idempotency_key: "read-shared-value".to_string(),
            source: r#"text(String(load("key")));"#.to_string(),
            ..execute_request("")
        },
    )
    .await;
    let other_session = execute(
        &second_session,
        CreateCellRequest {
            idempotency_key: "read-other-session-value".to_string(),
            source: r#"text(String(load("key")));"#.to_string(),
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
    let service = CodeModeService::new();

    let response = execute_with_yield_time(
        &service,
        CreateCellRequest {
            source: "while (true) {}".to_string(),
            ..execute_request("")
        },
        /*yield_time_ms*/ 1,
    )
    .await;
    assert_eq!(
        response,
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
    let service = CodeModeService::new();
    service.shutdown().await.unwrap();

    let error = service
        .create_cell(execute_request("text('late');"))
        .await
        .err()
        .unwrap();

    assert_eq!(error, "code mode session is shutting down".to_string());
}

#[tokio::test]
async fn create_and_observe_to_pending_returns_completed_for_synchronous_results() {
    let service = CodeModeService::new();

    let response = create_and_observe_to_pending(
        &service,
        CreateCellRequest {
            source: r#"text("done");"#.to_string(),
            ..execute_request("")
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response,
        PendingOutcome::Completed(RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
}

#[tokio::test]
async fn create_and_observe_to_pending_returns_once_the_runtime_is_quiescent() {
    let service = CodeModeService::new();

    let response = tokio::time::timeout(
        Duration::from_secs(1),
        create_and_observe_to_pending(
            &service,
            CreateCellRequest {
                source: r#"text("before"); await new Promise(() => {});"#.to_string(),
                ..execute_request("")
            },
        ),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        response,
        PendingOutcome::Pending {
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
        TerminateOutcome::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
}

#[tokio::test]
async fn create_and_observe_to_pending_identifies_tool_calls_in_paused_frontier() {
    let service = CodeModeService::new();

    let response = create_and_observe_to_pending(
        &service,
        CreateCellRequest {
            enabled_tools: vec![echo_tool()],
            source: r#"
await Promise.all([
  tools.echo({ value: "first" }),
  tools.echo({ value: "second" }),
]);
"#
            .to_string(),
            ..execute_request("")
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response,
        PendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string(), "tool-2".to_string()],
        }
    );

    let termination = service.terminate(cell_id("1")).await.unwrap();

    assert_eq!(
        termination,
        TerminateOutcome::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
}

#[tokio::test]
async fn wait_to_pending_retains_outstanding_calls_when_a_delayed_call_is_added() {
    let service = CodeModeService::new();

    let initial_response = create_and_observe_to_pending(
        &service,
        CreateCellRequest {
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
            ..execute_request("")
        },
    )
    .await
    .unwrap();

    assert_eq!(
        initial_response,
        PendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string(), "tool-2".to_string()],
        }
    );

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Resuming can expose an empty quiescent frontier before the expired timer
    // callback is dequeued, so keep observing until its tool call is visible.
    let resumed_response = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let response = service
                .observe_to_pending(ObserveToPendingRequest {
                    cell_id: cell_id("1"),
                })
                .await?;
            if !matches!(
                &response,
                ObserveToPendingOutcome::LiveCell(PendingOutcome::Pending {
                    pending_tool_call_ids,
                    ..
                }) if pending_tool_call_ids.is_empty()
            ) {
                break Ok::<_, String>(response);
            }
        }
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        resumed_response,
        ObserveToPendingOutcome::LiveCell(PendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec![
                "tool-1".to_string(),
                "tool-2".to_string(),
                "tool-3".to_string(),
            ],
        })
    );

    let termination = service.terminate(cell_id("1")).await.unwrap();

    assert_eq!(
        termination,
        TerminateOutcome::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
}

#[tokio::test]
async fn observe_to_pending_returns_after_resumed_runtime_becomes_quiescent_again() {
    let delegate = Arc::new(ReleasableToolDelegate::default());
    let service = CodeModeService::with_delegate(delegate.clone());

    let initial_response = create_and_observe_to_pending(
        &service,
        CreateCellRequest {
            enabled_tools: vec![echo_tool()],
            source: r#"
await tools.echo({});
text("after");
await new Promise(() => {});
"#
            .to_string(),
            ..execute_request("")
        },
    )
    .await
    .unwrap();

    assert_eq!(
        initial_response,
        PendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string()],
        }
    );

    delegate.release_tool();

    let resumed_response = tokio::time::timeout(
        Duration::from_secs(1),
        service.observe_to_pending(ObserveToPendingRequest {
            cell_id: cell_id("1"),
        }),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        resumed_response,
        ObserveToPendingOutcome::LiveCell(PendingOutcome::Pending {
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
        TerminateOutcome::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
}

#[tokio::test]
async fn observe_to_pending_returns_completed_after_resumed_runtime_finishes() {
    let delegate = Arc::new(ReleasableToolDelegate::default());
    let service = CodeModeService::with_delegate(delegate.clone());

    let initial_response = create_and_observe_to_pending(
        &service,
        CreateCellRequest {
            enabled_tools: vec![echo_tool()],
            source: r#"
await tools.echo({});
text("done");
"#
            .to_string(),
            ..execute_request("")
        },
    )
    .await
    .unwrap();

    assert_eq!(
        initial_response,
        PendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string()],
        }
    );

    delegate.release_tool();

    let resumed_response = tokio::time::timeout(
        Duration::from_secs(1),
        service.observe_to_pending(ObserveToPendingRequest {
            cell_id: cell_id("1"),
        }),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        resumed_response,
        ObserveToPendingOutcome::LiveCell(PendingOutcome::Completed(RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        }))
    );
}

#[tokio::test]
async fn v8_console_is_not_exposed_on_global_this() {
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
            source: r#"text(String(Object.hasOwn(globalThis, "console")));"#.to_string(),
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
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
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
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
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
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
            source: r#"
const returnsUndefined = [
  text("first"),
  image("data:image/png;base64,AAA"),
  notify("ping"),
].map((value) => value === undefined);
text(JSON.stringify(returnsUndefined));
"#
            .to_string(),
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
                FunctionCallOutputContentItem::InputText {
                    text: "[true,true,true]".to_string(),
                },
            ],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn image_helper_accepts_raw_mcp_image_block_with_original_detail() {
    let service = CodeModeService::new();

    let response = execute(
            &service,
            CreateCellRequest {
                source: r#"
image({
  type: "image",
  data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  mimeType: "image/png",
  _meta: { "codex/imageDetail": "original" },
});
"#
                .to_string(),
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
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
            source: r#"
generatedImage({
  image_url: "data:image/png;base64,AAA",
  output_hint: "generated image save hint",
});
"#
            .to_string(),
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
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
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
    let service = CodeModeService::new();

    let response = execute(
            &service,
            CreateCellRequest {
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
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
            source: r#"
image({
  image_url: "data:image/png;base64,AAA",
  detail: "low",
});
"#
            .to_string(),
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
            let service = CodeModeService::new();

            let response = execute(
                &service,
                CreateCellRequest {
                    source,
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
async fn image_helper_rejects_unsupported_detail() {
    let service = CodeModeService::new();

    let response = execute(
        &service,
        CreateCellRequest {
            source: r#"
image({
  image_url: "data:image/png;base64,AAA",
  detail: "medium",
});
"#
            .to_string(),
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
    let service = CodeModeService::new();

    let response = execute(
            &service,
            CreateCellRequest {
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
async fn observe_reports_missing_cell_separately_from_runtime_results() {
    let service = CodeModeService::new();

    let response = service
        .observe(ObserveRequest {
            idempotency_key: "observe-missing".to_string(),
            cell_id: cell_id("missing"),
            yield_time_ms: 1,
        })
        .await
        .unwrap();

    assert_eq!(
        response,
        ObserveOutcome::Missing {
            cell_id: cell_id("missing"),
        }
    );
}

#[test]
fn protocol_requests_map_to_runtime_requests_field_for_field() {
    let request = CreateCellRequest {
        idempotency_key: "thread-3:response-call-7".to_string(),
        tool_call_id: "response-call-7".to_string(),
        enabled_tools: vec![
            ToolDefinition {
                name: "mcp__search__query".to_string(),
                tool_name: ToolName::namespaced("search", "query"),
                description: "Search indexed documents".to_string(),
                kind: CodeModeToolKind::Function,
                input_schema: Some(json!({"type": "object"})),
                output_schema: None,
            },
            ToolDefinition {
                name: "apply_patch".to_string(),
                tool_name: ToolName::plain("apply_patch"),
                description: "Apply a patch".to_string(),
                kind: CodeModeToolKind::Freeform,
                input_schema: None,
                output_schema: Some(json!({"type": "string"})),
            },
        ],
        source: "await tools.mcp__search__query({ q: 'rust' });".to_string(),
    };

    assert_eq!(
        runtime_request(request),
        runtime::CreateCellRequest {
            idempotency_key: "thread-3:response-call-7".to_string(),
            tool_call_id: "response-call-7".to_string(),
            enabled_tools: vec![
                runtime::ToolDefinition {
                    name: "mcp__search__query".to_string(),
                    tool_name: runtime::ToolName {
                        name: "query".to_string(),
                        namespace: Some("search".to_string()),
                    },
                    description: "Search indexed documents".to_string(),
                    kind: runtime::ToolKind::Function,
                },
                runtime::ToolDefinition {
                    name: "apply_patch".to_string(),
                    tool_name: runtime::ToolName {
                        name: "apply_patch".to_string(),
                        namespace: None,
                    },
                    description: "Apply a patch".to_string(),
                    kind: runtime::ToolKind::Freeform,
                },
            ],
            source: "await tools.mcp__search__query({ q: 'rust' });".to_string(),
        }
    );

    let protocol_id = cell_id("cell-a7");
    assert_eq!(
        protocol_cell_id(&runtime_cell_id(&protocol_id)),
        protocol_id
    );
}

#[tokio::test]
async fn protocol_delegate_maps_callbacks_cancellation_and_errors_field_for_field() {
    let delegate = Arc::new(RecordingDelegate::new(
        vec![
            Ok(json!({"matches": 3})),
            Err("freeform tool failed".to_string()),
        ],
        vec![Err("notification failed".to_string())],
    ));
    let adapter = ProtocolDelegate {
        delegate: delegate.clone(),
    };
    let cancelled = CancellationToken::new();
    cancelled.cancel();

    assert_eq!(
        runtime::SessionRuntimeDelegate::invoke_tool(
            &adapter,
            runtime::NestedToolCall {
                cell_id: runtime::CellId::new("cell-a7"),
                runtime_tool_call_id: "runtime-call-1".to_string(),
                tool_name: runtime::ToolName {
                    name: "query".to_string(),
                    namespace: Some("search".to_string()),
                },
                tool_kind: runtime::ToolKind::Function,
                input: Some(json!({"q": "rust"})),
            },
            CancellationToken::new(),
        )
        .await,
        Ok(json!({"matches": 3}))
    );
    assert_eq!(
        runtime::SessionRuntimeDelegate::invoke_tool(
            &adapter,
            runtime::NestedToolCall {
                cell_id: runtime::CellId::new("cell-b9"),
                runtime_tool_call_id: "runtime-call-2".to_string(),
                tool_name: runtime::ToolName {
                    name: "apply_patch".to_string(),
                    namespace: None,
                },
                tool_kind: runtime::ToolKind::Freeform,
                input: None,
            },
            cancelled.clone(),
        )
        .await,
        Err("freeform tool failed".to_string())
    );
    assert_eq!(
        runtime::SessionRuntimeDelegate::notify(
            &adapter,
            "notification-1".to_string(),
            runtime::CellId::new("cell-c3"),
            "progress".to_string(),
            cancelled,
        )
        .await,
        Err("notification failed".to_string())
    );

    assert_eq!(
        delegate.take_calls(),
        vec![
            RecordedDelegateCall::Tool {
                invocation: CodeModeNestedToolCall {
                    cell_id: cell_id("cell-a7"),
                    runtime_tool_call_id: "runtime-call-1".to_string(),
                    tool_name: ToolName::namespaced("search", "query"),
                    tool_kind: CodeModeToolKind::Function,
                    input: Some(json!({"q": "rust"})),
                },
                cancellation_requested: false,
            },
            RecordedDelegateCall::Tool {
                invocation: CodeModeNestedToolCall {
                    cell_id: cell_id("cell-b9"),
                    runtime_tool_call_id: "runtime-call-2".to_string(),
                    tool_name: ToolName::plain("apply_patch"),
                    tool_kind: CodeModeToolKind::Freeform,
                    input: None,
                },
                cancellation_requested: true,
            },
            RecordedDelegateCall::Notification {
                call_id: "notification-1".to_string(),
                cell_id: cell_id("cell-c3"),
                text: "progress".to_string(),
                cancellation_requested: true,
            },
        ]
    );
}

#[test]
fn runtime_events_map_to_protocol_outcomes_field_for_field() {
    let output_items = vec![
        runtime::OutputItem::Text {
            text: "before".to_string(),
        },
        runtime::OutputItem::Image {
            image_url: "data:image/png;base64,auto".to_string(),
            detail: Some(runtime::ImageDetail::Auto),
        },
        runtime::OutputItem::Image {
            image_url: "data:image/png;base64,low".to_string(),
            detail: Some(runtime::ImageDetail::Low),
        },
        runtime::OutputItem::Image {
            image_url: "data:image/png;base64,high".to_string(),
            detail: Some(runtime::ImageDetail::High),
        },
        runtime::OutputItem::Image {
            image_url: "data:image/png;base64,original".to_string(),
            detail: Some(runtime::ImageDetail::Original),
        },
        runtime::OutputItem::Image {
            image_url: "data:image/png;base64,default".to_string(),
            detail: None,
        },
    ];
    let expected_output_items = vec![
        FunctionCallOutputContentItem::InputText {
            text: "before".to_string(),
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,auto".to_string(),
            detail: Some(crate::ImageDetail::Auto),
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,low".to_string(),
            detail: Some(crate::ImageDetail::Low),
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,high".to_string(),
            detail: Some(crate::ImageDetail::High),
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,original".to_string(),
            detail: Some(crate::ImageDetail::Original),
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,default".to_string(),
            detail: None,
        },
    ];

    assert_eq!(
        observe_outcome(
            &cell_id("cell-a7"),
            runtime::CellEvent::Yielded {
                content_items: output_items,
            },
        ),
        ObserveOutcome::Yielded {
            cell_id: cell_id("cell-a7"),
            content_items: expected_output_items,
        }
    );
    assert_eq!(
        observe_outcome(
            &cell_id("cell-b9"),
            runtime::CellEvent::Completed {
                content_items: vec![runtime::OutputItem::Text {
                    text: "failed".to_string(),
                }],
                error_text: Some("tool failed".to_string()),
            },
        ),
        ObserveOutcome::Completed {
            cell_id: cell_id("cell-b9"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "failed".to_string(),
            }],
            error_text: Some("tool failed".to_string()),
        }
    );
    assert_eq!(
        observe_outcome(
            &cell_id("cell-c3"),
            runtime::CellEvent::Terminated {
                content_items: vec![runtime::OutputItem::Text {
                    text: "partial".to_string(),
                }],
            },
        ),
        ObserveOutcome::Terminated {
            cell_id: cell_id("cell-c3"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "partial".to_string(),
            }],
        }
    );
    assert_eq!(
        pending_outcome(
            &cell_id("cell-d4"),
            runtime::PausableCellEvent::Pending(runtime::PendingFrontier {
                generation: runtime::PendingGeneration::new(/*value*/ 1),
                content_items: vec![runtime::OutputItem::Text {
                    text: "waiting".to_string(),
                }],
                pending_tool_call_ids: vec!["runtime-call-1".to_string()],
            }),
        ),
        Ok(PendingOutcome::Pending {
            cell_id: cell_id("cell-d4"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "waiting".to_string(),
            }],
            pending_tool_call_ids: vec!["runtime-call-1".to_string()],
        })
    );
    assert_eq!(
        pending_outcome(
            &cell_id("cell-e5"),
            runtime::PausableCellEvent::Completed {
                content_items: Vec::new(),
                error_text: None,
            },
        ),
        Ok(PendingOutcome::Completed(RuntimeResponse::Result {
            cell_id: cell_id("cell-e5"),
            content_items: Vec::new(),
            error_text: None,
        }))
    );
    assert_eq!(
        missing_cell_response(cell_id("missing")),
        RuntimeResponse::Result {
            cell_id: cell_id("missing"),
            content_items: Vec::new(),
            error_text: Some("exec cell missing not found".to_string()),
        }
    );
}
