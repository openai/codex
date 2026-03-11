use super::ConnectionSessionState;
use super::MessageProcessor;
use super::MessageProcessorArgs;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::transport::AppServerTransport;
use anyhow::Result;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_arg0::Arg0DispatchPaths;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::W3cTraceContext;
use opentelemetry::global;
use opentelemetry::trace::SpanId;
use opentelemetry::trace::SpanKind;
use opentelemetry::trace::TraceId;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::SpanData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tracing_subscriber::layer::SubscriberExt;
use wiremock::MockServer;

const TEST_CONNECTION_ID: ConnectionId = ConnectionId(7);

struct TestTracing {
    exporter: InMemorySpanExporter,
    provider: SdkTracerProvider,
}

fn init_test_tracing() -> &'static TestTracing {
    static TEST_TRACING: OnceLock<TestTracing> = OnceLock::new();
    TEST_TRACING.get_or_init(|| {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let tracer = provider.tracer("codex-app-server-message-processor-tests");
        global::set_text_map_propagator(TraceContextPropagator::new());
        let subscriber =
            tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
        tracing::subscriber::set_global_default(subscriber)
            .expect("global tracing subscriber should only be installed once");
        TestTracing { exporter, provider }
    })
}

fn request_from_client_request(request: ClientRequest) -> JSONRPCRequest {
    serde_json::from_value(serde_json::to_value(request).expect("serialize client request"))
        .expect("client request should convert to JSON-RPC")
}

fn tracing_test_guard() -> &'static tokio::sync::Mutex<()> {
    static GUARD: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| tokio::sync::Mutex::new(()))
}

struct TracingHarness {
    _server: MockServer,
    _codex_home: TempDir,
    processor: MessageProcessor,
    outgoing_rx: mpsc::Receiver<crate::outgoing_message::OutgoingEnvelope>,
    session: ConnectionSessionState,
    tracing: &'static TestTracing,
}

impl TracingHarness {
    async fn new() -> Result<Self> {
        let server = create_mock_responses_server_repeating_assistant("Done").await;
        let codex_home = TempDir::new()?;
        let config = Arc::new(build_test_config(codex_home.path(), &server.uri()).await?);
        let (mut processor, outgoing_rx) = build_test_processor(config);
        let tracing = init_test_tracing();
        tracing.exporter.reset();
        let mut session = ConnectionSessionState::default();
        tracing::callsite::rebuild_interest_cache();

        let initialize_request = request_from_client_request(ClientRequest::Initialize {
            request_id: RequestId::Integer(1),
            params: InitializeParams {
                client_info: ClientInfo {
                    name: "codex-app-server-tests".to_string(),
                    title: None,
                    version: "0.1.0".to_string(),
                },
                capabilities: Some(InitializeCapabilities {
                    experimental_api: true,
                    ..Default::default()
                }),
            },
        });
        processor
            .process_request(
                TEST_CONNECTION_ID,
                initialize_request,
                AppServerTransport::Stdio,
                &mut session,
            )
            .await;
        assert!(session.initialized);

        Ok(Self {
            _server: server,
            _codex_home: codex_home,
            processor,
            outgoing_rx,
            session,
            tracing,
        })
    }

    fn reset_tracing(&self) {
        self.tracing.exporter.reset();
    }

    async fn start_thread(
        &mut self,
        request_id: i64,
        trace: Option<W3cTraceContext>,
    ) -> ThreadStartResponse {
        let mut thread_start_request = request_from_client_request(ClientRequest::ThreadStart {
            request_id: RequestId::Integer(request_id),
            params: ThreadStartParams {
                ephemeral: Some(true),
                ..ThreadStartParams::default()
            },
        });
        thread_start_request.trace = trace;

        self.processor
            .process_request(
                TEST_CONNECTION_ID,
                thread_start_request,
                AppServerTransport::Stdio,
                &mut self.session,
            )
            .await;
        let response = read_response(&mut self.outgoing_rx, request_id).await;
        read_thread_started_notification(&mut self.outgoing_rx).await;
        response
    }
}

async fn build_test_config(codex_home: &Path, server_uri: &str) -> Result<Config> {
    write_mock_responses_config_toml(
        codex_home,
        server_uri,
        &BTreeMap::new(),
        8_192,
        Some(false),
        "mock_provider",
        "compact",
    )?;

    Ok(ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .build()
        .await?)
}

fn build_test_processor(
    config: Arc<Config>,
) -> (
    MessageProcessor,
    mpsc::Receiver<crate::outgoing_message::OutgoingEnvelope>,
) {
    let (outgoing_tx, outgoing_rx) = mpsc::channel(16);
    let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
    let processor = MessageProcessor::new(MessageProcessorArgs {
        outgoing,
        arg0_paths: Arg0DispatchPaths::default(),
        config,
        cli_overrides: Vec::new(),
        loader_overrides: LoaderOverrides::default(),
        cloud_requirements: CloudRequirementsLoader::default(),
        feedback: CodexFeedback::new(),
        log_db: None,
        config_warnings: Vec::new(),
        session_source: SessionSource::VSCode,
        enable_codex_api_key_env: false,
    });
    (processor, outgoing_rx)
}

fn span_attr<'a>(span: &'a SpanData, key: &str) -> Option<&'a str> {
    span.attributes
        .iter()
        .find(|kv| kv.key.as_str() == key)
        .and_then(|kv| match &kv.value {
            opentelemetry::Value::String(value) => Some(value.as_str()),
            _ => None,
        })
}

fn find_rpc_span<'a>(spans: &'a [SpanData], kind: SpanKind, method: &str) -> &'a SpanData {
    spans
        .iter()
        .find(|span| {
            span.span_kind == kind
                && span_attr(span, "rpc.system") == Some("jsonrpc")
                && span_attr(span, "rpc.method") == Some(method)
        })
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} span for rpc.method={method}; exported spans:\n{}",
                format_spans(spans)
            )
        })
}

fn find_rpc_span_with_trace<'a>(
    spans: &'a [SpanData],
    kind: SpanKind,
    method: &str,
    trace_id: TraceId,
) -> &'a SpanData {
    spans
        .iter()
        .find(|span| {
            span.span_kind == kind
                && span_attr(span, "rpc.system") == Some("jsonrpc")
                && span_attr(span, "rpc.method") == Some(method)
                && span.span_context.trace_id() == trace_id
        })
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} span for rpc.method={method} trace={trace_id}; exported spans:\n{}",
                format_spans(spans)
            )
        })
}

fn find_span_by_name<'a>(spans: &'a [SpanData], name: &str) -> &'a SpanData {
    spans
        .iter()
        .find(|span| span.name.as_ref() == name)
        .unwrap_or_else(|| {
            panic!(
                "missing span named {name}; exported spans:\n{}",
                format_spans(spans)
            )
        })
}

fn find_span_by_name_with_trace<'a>(
    spans: &'a [SpanData],
    name: &str,
    trace_id: TraceId,
) -> &'a SpanData {
    spans
        .iter()
        .find(|span| span.name.as_ref() == name && span.span_context.trace_id() == trace_id)
        .unwrap_or_else(|| {
            panic!(
                "missing span named {name} for trace={trace_id}; exported spans:\n{}",
                format_spans(spans)
            )
        })
}

fn format_spans(spans: &[SpanData]) -> String {
    spans
        .iter()
        .map(|span| {
            let rpc_method = span_attr(span, "rpc.method").unwrap_or("-");
            format!(
                "name={} span_id={} kind={:?} parent={} trace={} rpc.method={}",
                span.name,
                span.span_context.span_id(),
                span.span_kind,
                span.parent_span_id,
                span.span_context.trace_id(),
                rpc_method
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_span_descends_from(spans: &[SpanData], child: &SpanData, ancestor: &SpanData) {
    let ancestor_span_id = ancestor.span_context.span_id();
    let mut parent_span_id = child.parent_span_id;
    while parent_span_id != SpanId::INVALID {
        if parent_span_id == ancestor_span_id {
            return;
        }
        let Some(parent_span) = spans
            .iter()
            .find(|span| span.span_context.span_id() == parent_span_id)
        else {
            break;
        };
        parent_span_id = parent_span.parent_span_id;
    }

    panic!(
        "span {} does not descend from {}; exported spans:\n{}",
        child.name,
        ancestor.name,
        format_spans(spans)
    );
}

async fn read_response<T: serde::de::DeserializeOwned>(
    outgoing_rx: &mut mpsc::Receiver<crate::outgoing_message::OutgoingEnvelope>,
    request_id: i64,
) -> T {
    loop {
        let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), outgoing_rx.recv())
            .await
            .expect("timed out waiting for response")
            .expect("outgoing channel closed");
        let crate::outgoing_message::OutgoingEnvelope::ToConnection {
            connection_id,
            message,
        } = envelope
        else {
            continue;
        };
        if connection_id != TEST_CONNECTION_ID {
            continue;
        }
        let crate::outgoing_message::OutgoingMessage::Response(response) = message else {
            continue;
        };
        if response.id != RequestId::Integer(request_id) {
            continue;
        }
        return serde_json::from_value(response.result)
            .expect("response payload should deserialize");
    }
}

async fn read_thread_started_notification(
    outgoing_rx: &mut mpsc::Receiver<crate::outgoing_message::OutgoingEnvelope>,
) {
    loop {
        let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), outgoing_rx.recv())
            .await
            .expect("timed out waiting for thread/started notification")
            .expect("outgoing channel closed");
        match envelope {
            crate::outgoing_message::OutgoingEnvelope::ToConnection {
                connection_id,
                message,
            } => {
                if connection_id != TEST_CONNECTION_ID {
                    continue;
                }
                let crate::outgoing_message::OutgoingMessage::AppServerNotification(notification) =
                    message
                else {
                    continue;
                };
                if matches!(
                    notification,
                    codex_app_server_protocol::ServerNotification::ThreadStarted(_)
                ) {
                    return;
                }
            }
            crate::outgoing_message::OutgoingEnvelope::Broadcast { message } => {
                let crate::outgoing_message::OutgoingMessage::AppServerNotification(notification) =
                    message
                else {
                    continue;
                };
                if matches!(
                    notification,
                    codex_app_server_protocol::ServerNotification::ThreadStarted(_)
                ) {
                    return;
                }
            }
        }
    }
}

async fn wait_for_exported_spans<F>(tracing: &TestTracing, predicate: F) -> Vec<SpanData>
where
    F: Fn(&[SpanData]) -> bool,
{
    let mut last_spans = Vec::new();
    for _ in 0..200 {
        tokio::task::yield_now().await;
        tracing
            .provider
            .force_flush()
            .expect("force flush should succeed");
        let spans = tracing.exporter.get_finished_spans().expect("span export");
        last_spans = spans.clone();
        if predicate(&spans) {
            return spans;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    panic!(
        "timed out waiting for expected exported spans:\n{}",
        format_spans(&last_spans)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn thread_start_jsonrpc_span_exports_server_span_and_parents_children() -> Result<()> {
    let _guard = tracing_test_guard().lock().await;
    let mut harness = TracingHarness::new().await?;

    let remote_trace_id = TraceId::from_hex("00000000000000000000000000000011").expect("trace id");
    let remote_parent_span_id = SpanId::from_hex("0000000000000022").expect("parent span id");
    let remote_trace = W3cTraceContext {
        traceparent: Some(format!("00-{remote_trace_id}-{remote_parent_span_id}-01")),
        tracestate: Some("vendor=value".to_string()),
    };

    let _: ThreadStartResponse = harness.start_thread(2, Some(remote_trace)).await;
    drop(harness.processor);
    let spans = wait_for_exported_spans(harness.tracing, |spans| {
        spans.iter().any(|span| {
            span.span_kind == SpanKind::Server
                && span_attr(span, "rpc.method") == Some("thread/start")
                && span.span_context.trace_id() == remote_trace_id
        }) && spans
            .iter()
            .any(|span| span.name.as_ref() == "thread_spawn")
    })
    .await;

    let server_request_span =
        find_rpc_span_with_trace(&spans, SpanKind::Server, "thread/start", remote_trace_id);
    let thread_spawn_span = find_span_by_name(&spans, "thread_spawn");
    let session_init_span = find_span_by_name(&spans, "session_init");
    assert_eq!(server_request_span.name.as_ref(), "thread/start");
    assert_eq!(server_request_span.span_context.trace_id(), remote_trace_id);
    assert_ne!(server_request_span.span_context.span_id(), SpanId::INVALID);
    assert_span_descends_from(&spans, thread_spawn_span, server_request_span);
    assert_span_descends_from(&spans, session_init_span, server_request_span);

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn turn_start_jsonrpc_span_parents_core_turn_spans() -> Result<()> {
    let _guard = tracing_test_guard().lock().await;
    let mut harness = TracingHarness::new().await?;
    let thread_start_response = harness.start_thread(2, None).await;
    let thread_id = thread_start_response.thread.id.clone();

    harness.reset_tracing();

    let remote_trace_id = TraceId::from_hex("00000000000000000000000000000077").expect("trace id");
    let remote_parent_span_id = SpanId::from_hex("0000000000000088").expect("parent span id");
    let remote_trace = W3cTraceContext {
        traceparent: Some(format!("00-{remote_trace_id}-{remote_parent_span_id}-01")),
        tracestate: Some("vendor=value".to_string()),
    };
    let turn_start_request = JSONRPCRequest {
        id: RequestId::Integer(3),
        method: "turn/start".to_string(),
        params: Some(serde_json::to_value(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: None,
            service_tier: None,
            effort: None,
            summary: None,
            personality: None,
            output_schema: None,
            collaboration_mode: None,
        })?),
        trace: Some(remote_trace),
    };
    harness
        .processor
        .process_request(
            TEST_CONNECTION_ID,
            turn_start_request,
            AppServerTransport::Stdio,
            &mut harness.session,
        )
        .await;
    let turn_start_response: TurnStartResponse = read_response(&mut harness.outgoing_rx, 3).await;
    let spans = wait_for_exported_spans(harness.tracing, |spans| {
        spans
            .iter()
            .any(|span| span.name.as_ref() == "submission_dispatch")
            && spans
                .iter()
                .any(|span| span.name.as_ref() == "session_task.turn")
            && spans.iter().any(|span| span.name.as_ref() == "run_turn")
    })
    .await;
    let _ = turn_start_response;
    drop(harness.processor);
    tokio::task::yield_now().await;

    let server_request_span = find_rpc_span(&spans, SpanKind::Server, "turn/start");
    let submission_dispatch_span =
        find_span_by_name_with_trace(&spans, "submission_dispatch", remote_trace_id);
    let session_task_turn_span =
        find_span_by_name_with_trace(&spans, "session_task.turn", remote_trace_id);
    let run_turn_span = find_span_by_name_with_trace(&spans, "run_turn", remote_trace_id);

    assert_eq!(server_request_span.parent_span_id, remote_parent_span_id);
    assert!(server_request_span.parent_span_is_remote);
    assert_eq!(server_request_span.span_context.trace_id(), remote_trace_id);
    assert_span_descends_from(&spans, submission_dispatch_span, server_request_span);
    assert_span_descends_from(&spans, session_task_turn_span, server_request_span);
    assert_span_descends_from(&spans, run_turn_span, server_request_span);
    assert_span_descends_from(&spans, session_task_turn_span, submission_dispatch_span);
    assert_span_descends_from(&spans, run_turn_span, session_task_turn_span);

    Ok(())
}
