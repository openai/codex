//! Real stdio callbacks retain each execution's context after it yields.

use std::sync::Arc;
use std::time::Duration;

use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::NotificationFuture;
use codex_code_mode::ProcessOwnedCodeModeSessionProvider;
use codex_code_mode::RuntimeResponse;
use codex_code_mode::ToolDefinition;
use codex_code_mode::ToolInvocationFuture;
use codex_code_mode::WaitOutcome;
use codex_code_mode::WaitRequest;
use codex_protocol::ToolName;
use codex_protocol::protocol::W3cTraceContext;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::Sampler;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::layer::SubscriberExt;

struct RecordingDelegate {
    contexts: mpsc::UnboundedSender<W3cTraceContext>,
    release: Notify,
    closed: Notify,
}

#[allow(clippy::expect_used)]
impl CodeModeSessionDelegate for RecordingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        _cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        // Read on entry as well as after an await, like real delegate implementations.
        self.contexts
            .send(codex_otel::current_span_w3c_trace_context().expect("callback context"))
            .expect("context receiver remains open");
        Box::pin(async move {
            self.release.notified().await;
            self.contexts
                .send(codex_otel::current_span_w3c_trace_context().expect("context after await"))
                .expect("context receiver remains open");
            Ok(serde_json::json!("ready"))
        })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: codex_code_mode::CellId,
        _text: String,
        _cancellation: CancellationToken,
    ) -> NotificationFuture<'a> {
        self.contexts
            .send(codex_otel::current_span_w3c_trace_context().expect("notification context"))
            .expect("context receiver remains open");
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, _cell_id: &codex_code_mode::CellId) {
        self.closed.notify_one();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_callbacks_preserve_context_across_overlapping_yielded_cells() {
    let exported = InMemorySpanExporter::default();
    let telemetry = SdkTracerProvider::builder()
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::AlwaysOff)))
        .with_simple_exporter(exported.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(telemetry.tracer("code-mode-test")));
    let _subscriber = tracing::subscriber::set_default(subscriber);
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").unwrap(),
    );
    let session = provider.create_session().await.unwrap();
    let mut pending = Vec::new();

    // Share one host and session across sampled, unsampled, and untraced callers.
    for (sequence, remote) in [
        Some("00-11111111111111111111111111111111-1111111111111111-01"),
        Some("00-22222222222222222222222222222222-2222222222222222-00"),
        None,
    ]
    .into_iter()
    .enumerate()
    {
        let parent = if let Some(traceparent) = remote {
            let parent = tracing::info_span!(parent: None, "exec", sequence);
            assert!(codex_otel::set_parent_from_w3c_trace_context(
                &parent,
                &W3cTraceContext {
                    traceparent: Some(traceparent.to_string()),
                    tracestate: Some(format!("test=cell{sequence}")),
                },
            ));
            parent
        } else {
            tracing::Span::none()
        };
        let expected = parent.context().span().span_context().clone();
        let (contexts, mut observed) = mpsc::unbounded_channel();
        let delegate = Arc::new(RecordingDelegate {
            contexts,
            release: Notify::new(),
            closed: Notify::new(),
        });
        let response = async {
            session
                .execute(
                    ExecuteRequest {
                        tool_call_id: format!("call-{sequence}"),
                        enabled_tools: vec![ToolDefinition {
                            name: "gate".to_string(),
                            tool_name: ToolName::plain("gate"),
                            description: String::new(),
                            kind: CodeModeToolKind::Function,
                            input_schema: None,
                            input_schema_max_bytes: None,
                            output_schema: None,
                        }],
                        source: "text(await tools.gate()); await notify('done');".to_string(),
                        yield_time_ms: Some(1),
                        max_output_tokens: None,
                    },
                    delegate.clone(),
                    /*preempt*/ None,
                )
                .await
                .unwrap()
                .initial_response()
                .await
                .unwrap()
        }
        .instrument(parent)
        .await;
        let RuntimeResponse::Yielded { cell_id, .. } = response else {
            panic!("blocked callback must yield");
        };
        let first = tokio::time::timeout(Duration::from_secs(10), observed.recv())
            .await
            .unwrap()
            .unwrap();
        if expected.is_sampled() {
            assert!(
                exported.get_finished_spans().unwrap().iter().any(|span| {
                    span.name == "exec" && span.span_context.span_id() == expected.span_id()
                }),
                "exec must finish exporting while its callback is still blocked"
            );
        }
        pending.push((cell_id, delegate, observed, first, expected));
    }

    // Complete in the opposite order to admission to catch shared-context mistakes.
    for (cell_id, delegate, mut observed, first, expected) in pending.into_iter().rev() {
        delegate.release.notify_one();
        let WaitOutcome::LiveCell(RuntimeResponse::Result {
            error_text: None, ..
        }) = session
            .wait(
                WaitRequest {
                    cell_id,
                    yield_time_ms: 5_000,
                    max_output_tokens: None,
                },
                /*preempt*/ None,
            )
            .await
            .unwrap()
        else {
            panic!("released callback must complete successfully");
        };
        tokio::time::timeout(Duration::from_secs(10), delegate.closed.notified())
            .await
            .unwrap();
        let after_await = observed.recv().await.unwrap();
        let notification = observed.recv().await.unwrap();
        assert_eq!(first, after_await);
        for context in [first, notification] {
            let actual = codex_otel::context_from_w3c_trace_context(&context).unwrap();
            let span = actual.span();
            let actual = span.span_context();
            if expected.is_valid() {
                assert_eq!(
                    (
                        actual.trace_id(),
                        actual.trace_flags(),
                        actual.trace_state()
                    ),
                    (
                        expected.trace_id(),
                        expected.trace_flags(),
                        expected.trace_state()
                    ),
                );
                assert_ne!(actual.span_id(), expected.span_id());
            } else {
                assert!(!actual.is_sampled());
                assert!(actual.trace_state().header().is_empty());
                assert_ne!(
                    actual.trace_id().to_string(),
                    "11111111111111111111111111111111"
                );
                assert_ne!(
                    actual.trace_id().to_string(),
                    "22222222222222222222222222222222"
                );
            }
        }
        assert!(observed.try_recv().is_err());
    }
    session.shutdown().await.unwrap();
    telemetry.force_flush().unwrap();
    let spans = exported.get_finished_spans().unwrap();
    let exec = spans.iter().find(|span| span.name == "exec").unwrap();
    let mut callbacks = spans
        .iter()
        .filter(|span| {
            matches!(
                span.name.as_ref(),
                "code_mode.stdio.callback" | "code_mode.stdio.notify"
            )
        })
        .map(|span| {
            (
                span.name.as_ref(),
                span.span_context.trace_id(),
                span.parent_span_id,
            )
        })
        .collect::<Vec<_>>();
    callbacks.sort_by_key(|(name, _, _)| *name);
    assert_eq!(
        callbacks,
        vec![
            (
                "code_mode.stdio.callback",
                exec.span_context.trace_id(),
                exec.span_context.span_id()
            ),
            (
                "code_mode.stdio.notify",
                exec.span_context.trace_id(),
                exec.span_context.span_id()
            ),
        ]
    );
}
