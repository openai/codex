use std::sync::Arc;
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::RequestId;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use opentelemetry::trace::SpanId;
use opentelemetry::trace::TraceId;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::prelude::*;

use super::ConcurrentRequestLimit;
use super::RequestDispatchMode;
use super::RequestDispatcher;
use super::RequestTaskResult;
use crate::ExecServerRuntimePaths;
use crate::connection::JsonRpcConnectionEvent;
use crate::rpc::RpcNotificationSender;
use crate::rpc::RpcRouter;
use crate::rpc::RpcServerOutboundMessage;
use crate::server::ExecServerHandler;
use crate::server::session_registry::SessionRegistry;
use crate::telemetry::ExecServerTelemetry;

/// Public limits reject values that cannot safely enable semaphore-backed concurrency.
#[test]
fn concurrent_request_limit_rejects_invalid_values() {
    assert_eq!(
        ConcurrentRequestLimit::new(/*max_concurrent_requests*/ 0),
        None
    );
    assert_eq!(
        ConcurrentRequestLimit::new(/*max_concurrent_requests*/ 1),
        None
    );
    assert_eq!(
        ConcurrentRequestLimit::new(Semaphore::MAX_PERMITS.saturating_add(1)),
        None
    );
    assert_eq!(
        ConcurrentRequestLimit::new(/*max_concurrent_requests*/ 2).map(ConcurrentRequestLimit::get),
        Some(2)
    );
}

/// CLI parsing keeps one request inline and bounds larger positive concurrency limits.
#[test]
fn request_dispatch_mode_parses_bounded_concurrency() {
    assert!(matches!("1".parse(), Ok(RequestDispatchMode::Inline)));
    assert!("0".parse::<RequestDispatchMode>().is_err());

    let oversized_limit = Semaphore::MAX_PERMITS.saturating_add(1).to_string();
    let mode = oversized_limit
        .parse::<RequestDispatchMode>()
        .expect("parse oversized concurrent request limit");
    let RequestDispatchMode::Concurrent {
        max_concurrent_requests,
    } = mode
    else {
        panic!("expected concurrent request dispatch");
    };
    assert_eq!(max_concurrent_requests.get(), Semaphore::MAX_PERMITS);
}

/// End-to-end request spans retain the wire method and inbound trace with bounded names.
#[test]
fn request_span_uses_bounded_name_wire_method_and_inbound_trace_parent() {
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("exec-server-test");
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(filter_fn(codex_otel::OtelProvider::trace_export_filter)),
    );
    let trace_id = TraceId::from_hex("00000000000000000000000000000001").expect("trace id");
    let parent_span_id = SpanId::from_hex("0000000000000002").expect("span id");
    let trace = codex_protocol::protocol::W3cTraceContext {
        traceparent: Some(format!("00-{trace_id}-{parent_span_id}-01")),
        tracestate: None,
    };

    let method = "custom/method";
    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let request = JSONRPCRequest {
            id: RequestId::Integer(1),
            method: method.to_string(),
            params: None,
            trace: Some(trace),
        };
        let JsonRpcConnectionEvent::QueuedRequest { request_span, .. } =
            JsonRpcConnectionEvent::message(JSONRPCMessage::Request(request))
        else {
            panic!("requests should start a server span before dispatch");
        };
        assert!(
            span_exporter
                .get_finished_spans()
                .expect("request span export")
                .is_empty(),
            "the request span must remain open while the request is waiting"
        );
        request_span.record("otel.name", "unknown");
        request_span.in_scope(|| {});
        drop(request_span);
    });

    tracer_provider.force_flush().expect("flush traces");
    let spans = span_exporter.get_finished_spans().expect("span export");
    assert_eq!(spans.len(), 1);
    let request_span = spans
        .iter()
        .find(|span| span.name.as_ref() == "unknown")
        .expect("unknown method span");
    assert_eq!(
        request_span
            .attributes
            .iter()
            .find(|attribute| attribute.key.as_str() == "method")
            .map(|attribute| attribute.value.clone()),
        Some(opentelemetry::Value::String(method.into()))
    );
    assert_eq!(request_span.span_context.trace_id(), trace_id);
    assert_eq!(request_span.parent_span_id, parent_span_id);
}

/// One end-to-end request span covers queueing and execution; the metric isolates admission wait.
#[tokio::test]
async fn request_queue_waits_for_dispatcher_admission_before_recording_telemetry() {
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(tracer_provider.tracer("exec-server-test"))
            .with_filter(filter_fn(codex_otel::OtelProvider::trace_export_filter)),
    );
    let _subscriber = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-exec-server",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )
    .expect("metrics client");
    let telemetry = ExecServerTelemetry::new(metrics.clone());
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*buffer*/ 1);
    let notifications = RpcNotificationSender::new(outgoing_tx.clone());
    let requests = notifications.request_sender();
    let handler = Arc::new(ExecServerHandler::new(
        SessionRegistry::new(telemetry.clone()),
        notifications,
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current executable"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths"),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ));
    let mut router = RpcRouter::new();
    let execution_started = Arc::new(Notify::new());
    let release_execution = Arc::new(Notify::new());
    let notify_execution_started = Arc::clone(&execution_started);
    let wait_for_execution_release = Arc::clone(&release_execution);
    let route_setup_duration = Duration::from_millis(200);
    router.request(
        "test/queued",
        move |_handler: Arc<ExecServerHandler>, _params: ()| {
            let execution_started = Arc::clone(&notify_execution_started);
            let release_execution = Arc::clone(&wait_for_execution_release);
            std::thread::sleep(route_setup_duration);
            async move {
                execution_started.notify_one();
                release_execution.notified().await;
                Ok::<_, codex_exec_server_protocol::JSONRPCErrorError>(())
            }
        },
    );
    let (_disconnected_tx, disconnected_rx) = watch::channel(/*init*/ false);
    let mut dispatcher = RequestDispatcher::new(
        Arc::new(router),
        handler,
        outgoing_tx,
        disconnected_rx,
        requests,
        telemetry,
        RequestDispatchMode::Concurrent {
            max_concurrent_requests: ConcurrentRequestLimit::new(
                /*max_concurrent_requests*/ 2,
            )
            .expect("valid request limit"),
        },
    );
    dispatcher.initialized = true;
    let admission = Arc::clone(
        &dispatcher
            .lanes
            .as_ref()
            .expect("concurrent request lanes")
            .ordinary,
    );
    let occupied_permits = admission
        .acquire_many_owned(/*n*/ 2)
        .await
        .expect("occupy the request admission lane");
    let JsonRpcConnectionEvent::QueuedRequest {
        request,
        request_span,
        queued_at,
    } = JsonRpcConnectionEvent::message(JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "test/queued".to_string(),
        params: None,
        trace: None,
    }))
    else {
        panic!("requests should start a server span before dispatch");
    };

    assert!(matches!(
        dispatcher
            .dispatch_request(request, request_span, queued_at)
            .await,
        RequestTaskResult::Completed
    ));
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(25)).await;

    assert!(
        span_exporter
            .get_finished_spans()
            .expect("request span export")
            .is_empty(),
        "the end-to-end request span must remain open before admission"
    );
    let queued_snapshot = metrics.snapshot().expect("queued metrics snapshot");
    assert!(
        !queued_snapshot
            .scope_metrics()
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .any(|metric| metric.name() == "exec_server_request_queue_duration_seconds"),
        "queue latency must not be recorded before request admission"
    );

    drop(occupied_permits);
    timeout(Duration::from_secs(1), execution_started.notified())
        .await
        .expect("queued request should execute after admission");
    assert!(
        span_exporter
            .get_finished_spans()
            .expect("executing span export")
            .is_empty(),
        "the same request span must remain open during execution"
    );

    let snapshot = metrics.snapshot().expect("metrics snapshot");
    let queue_metric = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == "exec_server_request_queue_duration_seconds")
        .expect("request queue duration metric");
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = queue_metric.data() else {
        panic!("request queue duration should be an f64 histogram");
    };
    let data_point = histogram
        .data_points()
        .next()
        .expect("request queue duration data point");
    let queue_duration = Duration::from_secs_f64(data_point.sum());

    assert_eq!(data_point.count(), 1);
    assert!(
        queue_duration >= Duration::from_millis(25),
        "queue latency should include the occupied admission lane"
    );
    assert_eq!(
        data_point
            .attributes()
            .find(|attribute| attribute.key.as_str() == "method")
            .map(|attribute| attribute.value.as_str().into_owned()),
        Some("test/queued".to_string())
    );

    release_execution.notify_one();
    let response = timeout(Duration::from_secs(1), outgoing_rx.recv())
        .await
        .expect("queued request should send its response")
        .expect("queued request response");
    assert!(matches!(
        response,
        RpcServerOutboundMessage::Response {
            request_id: RequestId::Integer(1),
            ..
        }
    ));
    assert!(matches!(
        dispatcher.join_next().await,
        RequestTaskResult::Completed
    ));

    tracer_provider.force_flush().expect("flush traces");
    let spans = span_exporter.get_finished_spans().expect("span export");
    assert_eq!(spans.len(), 1);
    let request_span = spans.first().expect("end-to-end request span");
    assert_eq!(request_span.name.as_ref(), "test/queued");
    let request_duration = request_span
        .end_time
        .duration_since(request_span.start_time)
        .expect("request span should have a valid interval");
    assert!(
        request_duration >= Duration::from_millis(25),
        "the end-to-end request span should include the admission wait"
    );
    assert!(
        queue_duration
            <= request_duration.saturating_sub(route_setup_duration) + Duration::from_millis(5),
        "queue latency must exclude synchronous request decoding and route setup"
    );

    metrics.shutdown().expect("shutdown metrics");
}
