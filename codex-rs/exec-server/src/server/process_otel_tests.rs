//! Verifies exported lifecycle and network logs retain launch identity without private payloads.

#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::RequestId;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::RemoteNetworkProxyConfig;
use codex_network_proxy::RemoteNetworkProxyLaunchConfig;
use codex_otel::OtelExporter;
use codex_otel::OtelHttpProtocol;
use codex_otel::OtelProvider;
use codex_otel::OtelSettings;
use codex_protocol::protocol::W3cTraceContext;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::Instrument;
use tracing_subscriber::prelude::*;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

use super::ExecServerHandler;
use super::registry::build_router;
use super::session_registry::SessionRegistry;
use crate::ExecServerRuntimePaths;
use crate::ExecServerTelemetry;
use crate::connection::JsonRpcConnectionEvent;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::ReadResponse;
use crate::rpc::RpcNotificationSender;
use crate::rpc::RpcRouter;
use crate::rpc::RpcServerOutboundMessage;
use crate::telemetry::ExecutorRegistration;

const PRIVATE_PAYLOAD: &str = "private-process-payload";
const TRACE_ID: &str = "11111111111111111111111111111111";
const LAUNCH_SPANS: [&str; 2] = ["2222222222222222", "3333333333333333"];
const THREADS: [&str; 2] = [
    "11111111-1111-4111-8111-111111111111",
    "22222222-2222-4222-8222-222222222222",
];
const CALLS: [&str; 2] = ["call-first", "call-second"];
const LATER_TRACE: &str = "00-44444444444444444444444444444444-5555555555555555-01";

#[test]
fn exported_process_and_network_logs_keep_the_validated_launch_reference() {
    // Keep the subscriber on the runtime's only thread, including proxy background tasks.
    for (export_traces, registered) in [(false, true), (true, true), (false, false)] {
        let records = exported_logs(export_traces, |runtime, _, _| {
            runtime.block_on(async {
                let server = TestServer::new();
                let mut handlers = Vec::new();
                let mut session_ids = Vec::new();
                for client_name in ["first-orchestrator", "second-orchestrator"] {
                    let (handler, session_id) = server.new_orchestrator(
                        client_name, registered.then_some("original"),
                    ).await;
                    handlers.push(handler);
                    session_ids.push(session_id);
                }
                let proxy = RemoteNetworkProxyLaunchConfig::new(
                    RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig {
                        enabled: true,
                        ..NetworkProxyConfig::default()
                    }).expect("proxy config"),
                );
                // Unsampled incoming context must still correlate logs-only export.
                let flags = if export_traces { "01" } else { "00" };
                let launches = LAUNCH_SPANS.map(|span| format!("00-{TRACE_ID}-{span}-{flags}"));
                for (batch, traces) in [
                    [Some(launches[0].as_str()), Some(launches[1].as_str())],
                    [None, Some(PRIVATE_PAYLOAD)],
                ].into_iter().enumerate() {
                    // Process IDs are session-scoped and deliberately reused across clients.
                    let process_id = format!("{PRIVATE_PAYLOAD}-{batch}");
                    for (index, (handler, trace)) in handlers.iter().zip(traces).enumerate() {
                        let mut params = process_start_params(
                            &process_id,
                            json!(["/bin/sh", "-c", "printf '%s\\n' \"$HTTP_PROXY\"; read ignored", PRIVATE_PAYLOAD]),
                            PathUri::from_host_native_path(std::env::current_dir().expect("cwd")).expect("cwd URI"),
                        );
                        params["metadata"] = if batch == 0 {
                            process_metadata(index)
                        } else {
                            json!({"toolCallId": if index == 0 { None } else { Some("private-process-payload\n") }})
                        };
                        params["metadata"]["executorRegistrationId"] = json!(PRIVATE_PAYLOAD);
                        params["metadata"]["environmentId"] = json!(PRIVATE_PAYLOAD);
                        params["executorRegistrationId"] = json!(PRIVATE_PAYLOAD);
                        params["environmentId"] = json!(PRIVATE_PAYLOAD);
                        params["pipeStdin"] = json!(true);
                        params["networkProxy"] = json!(proxy);
                        request(&server.router, handler, EXEC_METHOD, params, trace).await;
                    }
                    if batch == 0 {
                        handlers[0].shutdown().await;
                        let resumed = new_handler(
                            &server.sessions, server.outgoing.clone(), registered.then_some("resumed"),
                        );
                        resumed.initialize(InitializeParams {
                            client_name: "resumed-orchestrator".to_string(),
                            resume_session_id: Some(session_ids[0].clone()),
                        }).await.expect("resume session");
                        resumed.initialized().expect("initialized resumed orchestrator");
                        handlers[0] = resumed;
                    }
                    for handler in &handlers {
                        let output: ReadResponse = serde_json::from_value(request(
                            &server.router, handler, EXEC_READ_METHOD,
                            json!({"processId": process_id, "waitMs": 1000}), Some(LATER_TRACE),
                        ).await).expect("read proxy address");
                        let proxy_address = String::from_utf8(output.chunks.into_iter()
                            .flat_map(|chunk| chunk.chunk.into_inner()).collect()).expect("UTF-8 output");
                        let mut connection = tokio::net::TcpStream::connect(
                            proxy_address.trim().strip_prefix("http://").expect("HTTP proxy address"),
                        ).await.expect("connect to process proxy");
                        connection.write_all(b"CONNECT 8.8.8.8:443 HTTP/1.1\r\nHost: 8.8.8.8:443\r\n\r\n")
                            .await.expect("request denied destination");
                        let mut response = [0_u8; 256];
                        let length = tokio::time::timeout(Duration::from_secs(5), connection.read(&mut response))
                            .await.expect("proxy response timeout").expect("proxy response");
                        assert!(String::from_utf8_lossy(&response[..length]).starts_with("HTTP/1.1 403"));
                        request(&server.router, handler, EXEC_TERMINATE_METHOD,
                            json!({"processId": process_id}), Some(LATER_TRACE)).await;
                        read_until_closed(&server.router, handler, &process_id).await;
                    }
                }
                server.sessions.shutdown().await;
            });
        });
        let mut events = Vec::new();
        for record in &records {
            let name = attribute(record, "event.name").expect("event name");
            let trace = attribute(record, "launch.trace_id");
            let span = attribute(record, "launch.span_id");
            if export_traces
                && name.starts_with("codex.exec_server.process_")
                && let Some(span) = span
            {
                assert_eq!(record["traceId"].as_str(), trace);
                assert_ne!(record["spanId"].as_str().expect("native child span"), span);
            }
            events.push((
                name.to_string(),
                trace.map(str::to_string),
                span.map(str::to_string),
                attribute(record, "conversation.id").map(str::to_string),
                attribute(record, "tool.call_id").map(str::to_string),
                attribute(record, "executor.environment_id").map(str::to_string),
                attribute(record, "executor.registration_id").map(str::to_string),
            ));
        }
        let mut expected = Vec::new();
        for (index, span) in [Some(LAUNCH_SPANS[0]), Some(LAUNCH_SPANS[1]), None, None]
            .into_iter()
            .enumerate()
        {
            for name in [
                "codex.exec_server.process_start",
                "codex.exec_server.process_exit",
                "codex.network_proxy.policy_decision",
            ] {
                expected.push((
                    name.to_string(),
                    span.map(|_| TRACE_ID.to_string()),
                    span.map(str::to_string),
                    (index < 2).then(|| THREADS[index].to_string()),
                    (index < 2).then(|| CALLS[index].to_string()),
                    registered.then(|| "environment".to_string()),
                    registered.then(|| if index == 2 { "resumed" } else { "original" }.to_string()),
                ));
            }
        }
        events.sort();
        expected.sort();
        assert_eq!(events, expected);
    }
}

#[test]
fn launch_rpc_span_finishes_before_its_process_exits() {
    let mut observed = None;
    let records = exported_logs(/*export_traces*/ true, |runtime, otel, collector| {
        runtime.block_on(async {
            let server = TestServer::new();
            let (handler, _) = server
                .new_orchestrator("process-span-orchestrator", Some("original"))
                .await;
            let launch = format!("00-{TRACE_ID}-{}-01", LAUNCH_SPANS[0]);
            let mut params = process_start_params(
                PRIVATE_PAYLOAD,
                json!(["/bin/sh", "-c", "read ignored", PRIVATE_PAYLOAD]),
                PathUri::from_host_native_path(std::env::current_dir().expect("cwd"))
                    .expect("cwd URI"),
            );
            params["metadata"] = process_metadata(/*index*/ 0);
            params["pipeStdin"] = json!(true);
            request(&server.router, &handler, EXEC_METHOD, params, Some(&launch)).await;

            // Flush completed spans while the process is still waiting on its open stdin.
            // The old task's in_current_span() retains the RPC span, so this snapshot lacks it.
            let spans_before_exit = flushed_spans(otel, collector).await;
            let running: ReadResponse = serde_json::from_value(
                request(
                    &server.router,
                    &handler,
                    EXEC_READ_METHOD,
                    json!({"processId": PRIVATE_PAYLOAD, "waitMs": 0}),
                    Some(LATER_TRACE),
                )
                .await,
            )
            .expect("read running process");
            request(
                &server.router,
                &handler,
                EXEC_TERMINATE_METHOD,
                json!({"processId": PRIVATE_PAYLOAD}),
                Some(LATER_TRACE),
            )
            .await;
            let exited = read_until_closed(&server.router, &handler, PRIVATE_PAYLOAD).await;
            server.sessions.shutdown().await;
            let spans_after_exit = flushed_spans(otel, collector).await;
            // Finish cleanup before asserting the regression so a failure leaves no child behind.
            observed = Some((running, exited, spans_before_exit, spans_after_exit));
        });
    });
    let (running, exited, spans_before_exit, spans_after_exit) =
        observed.expect("process observations");
    assert_eq!(
        (running.exited, running.closed, running.exit_code),
        (false, false, None)
    );
    let launch_spans: Vec<_> = spans_before_exit
        .iter()
        .filter(|span| span["name"] == EXEC_METHOD)
        .collect();
    assert_eq!(
        launch_spans.len(),
        1,
        "process/start span must export before child exit"
    );
    let launch_span = launch_spans[0];
    assert_eq!(
        (
            launch_span["traceId"].as_str(),
            launch_span["parentSpanId"].as_str()
        ),
        (Some(TRACE_ID), Some(LAUNCH_SPANS[0]))
    );
    let exit_record = records
        .iter()
        .find(|record| attribute(record, "event.name") == Some("codex.exec_server.process_exit"))
        .expect("process exit log");
    let exit_span_id = exit_record["spanId"]
        .as_str()
        .expect("process exit span ID");
    // ProcessMetricGuard has the same span name; select the span that emitted the exit log.
    let process_spans: Vec<_> = spans_after_exit
        .iter()
        .filter(|span| span["spanId"].as_str() == Some(exit_span_id))
        .collect();
    assert_eq!(
        process_spans.len(),
        1,
        "process completion has its own span"
    );
    let process_span = process_spans[0];
    assert_eq!(process_span["name"], "codex.exec_server.process");
    assert_eq!(
        (
            process_span["traceId"].as_str(),
            process_span["parentSpanId"].as_str()
        ),
        (Some(TRACE_ID), Some(LAUNCH_SPANS[0]))
    );
    assert_ne!(process_span["spanId"], launch_span["spanId"]);
    let mut expected_exit =
        expected_process_attributes("codex.exec_server.process_exit", /*index*/ 0, "None");
    expected_exit["process.exit_code"] =
        json!({"intValue": exited.exit_code.expect("exit code").to_string()});
    expected_exit["process.termination_requested"] = json!({"boolValue": true});
    assert_exported_attributes(
        &records,
        vec![
            expected_process_attributes(
                "codex.exec_server.process_start",
                /*index*/ 0,
                "None",
            ),
            expected_exit,
        ],
    );
    for record in &records {
        assert_eq!(record["traceId"].as_str(), Some(TRACE_ID));
        let span = if attribute(record, "event.name") == Some("codex.exec_server.process_start") {
            launch_span
        } else {
            process_span
        };
        assert_eq!(record["spanId"], span["spanId"]);
    }
}

async fn flushed_spans(otel: &OtelProvider, collector: &MockServer) -> Vec<Value> {
    let provider = otel
        .tracer_provider
        .as_ref()
        .expect("trace exporter")
        .clone();
    // The collector uses this runtime, so the synchronous export flush must not block it.
    tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || provider.force_flush()),
    )
    .await
    .expect("trace flush timeout")
    .expect("trace flush task")
    .expect("trace flush");
    let mut spans = Vec::new();
    for request in collector
        .received_requests()
        .await
        .expect("exported requests")
    {
        let body: Value = serde_json::from_slice(&request.body).expect("OTLP JSON");
        let Some(resources) = body["resourceSpans"].as_array() else {
            continue;
        };
        for resource in resources {
            for scope in resource["scopeSpans"].as_array().expect("scope spans") {
                spans.extend(scope["spans"].as_array().expect("spans").iter().cloned());
            }
        }
    }
    spans
}

#[test]
fn exported_spawn_failure_keeps_launch_identity_without_outcome_or_error_text() {
    let records = exported_logs(/*export_traces*/ false, |runtime, _, _| {
        runtime.block_on(async {
            let server = TestServer::new();
            let (handler, _) = server
                .new_orchestrator("spawn-failure-orchestrator", Some("original"))
                .await;
            let directory = tempfile::tempdir().expect("process directory");
            let missing_executable = directory.path().join(PRIVATE_PAYLOAD);
            let launch = format!("00-{TRACE_ID}-{}-00", LAUNCH_SPANS[0]);
            let mut params = process_start_params(
                PRIVATE_PAYLOAD,
                json!([missing_executable, PRIVATE_PAYLOAD]),
                PathUri::from_host_native_path(directory.path()).expect("cwd URI"),
            );
            params["metadata"] = process_metadata(/*index*/ 0);
            let response =
                request_result(&server.router, &handler, EXEC_METHOD, params, Some(&launch)).await;
            let Some(RpcServerOutboundMessage::Error { error, .. }) = response else {
                panic!("missing executable must fail to spawn: {response:?}");
            };
            assert_eq!(error.code, -32603);
            assert!(
                !error.message.is_empty(),
                "caller still receives the spawn error"
            );
            server.sessions.shutdown().await;
        });
    });
    assert_exported_attributes(
        &records,
        vec![expected_process_attributes(
            "codex.exec_server.process_spawn_failed",
            /*index*/ 0,
            "None",
        )],
    );
}

#[cfg(target_os = "macos")]
#[test]
fn exported_sandbox_denial_keeps_launch_identity_and_separate_exit_outcome() {
    let records = exported_logs(/*export_traces*/ false, |runtime, _, _| {
        runtime.block_on(async {
            let server = TestServer::new();
            let (handler, _) = server.new_orchestrator("sandbox-denial-orchestrator", Some("original")).await;
            let directory = tempfile::tempdir().expect("process directory");
            let private_file = directory.path().join(PRIVATE_PAYLOAD);
            std::fs::write(&private_file, PRIVATE_PAYLOAD).expect("write test file");
            let cwd = PathUri::from_host_native_path(directory.path()).expect("cwd URI");
            let sandbox = crate::FileSystemSandboxContext::from_legacy_sandbox_policy(
                codex_protocol::protocol::SandboxPolicy::new_read_only_policy(), cwd.clone(),
            ).expect("read-only sandbox");
            for (index, span) in LAUNCH_SPANS.into_iter().enumerate() {
                let process_id = format!("{PRIVATE_PAYLOAD}-{index}");
                let launch = format!("00-{TRACE_ID}-{span}-00");
                let argv = if index == 0 {
                    json!(["/bin/cat", private_file])
                } else {
                    // Emit private output, then attempt a real denied write. Exit 23 is produced
                    // by the child shell only after that write fails, not by sandbox startup.
                    json!(["/bin/sh", "-c",
                        "printf '%s' \"$PRIVATE_TEST_VALUE\"; if printf changed > \"$1\"; then exit 0; else exit 23; fi",
                        PRIVATE_PAYLOAD, private_file])
                };
                let mut params = process_start_params(&process_id, argv, cwd.clone());
                params["metadata"] = process_metadata(index);
                params["env"]["LC_ALL"] = json!("C");
                params["sandbox"] = json!(sandbox);
                let started: crate::protocol::ExecResponse = serde_json::from_value(request(
                    &server.router, &handler, EXEC_METHOD, params, Some(&launch),
                ).await).expect("start sandboxed process");
                assert_eq!(started.sandbox_type, Some(crate::protocol::ProcessSandboxType::MacosSeatbelt));
                let output = read_until_closed(&server.router, &handler, &process_id).await;
                let stdout: Vec<u8> = output.chunks.iter()
                    .filter(|chunk| chunk.stream == crate::protocol::ExecOutputStream::Stdout)
                    .flat_map(|chunk| chunk.chunk.0.iter().copied()).collect();
                assert_eq!(stdout, PRIVATE_PAYLOAD.as_bytes());
                assert_eq!((output.exit_code, output.sandbox_denied), if index == 0 {
                    (Some(0), false)
                } else {
                    (Some(23), true)
                });
                if index == 1 {
                    let stderr: Vec<u8> = output.chunks.iter()
                        .filter(|chunk| chunk.stream == crate::protocol::ExecOutputStream::Stderr)
                        .flat_map(|chunk| chunk.chunk.0.iter().copied()).collect();
                    assert!(String::from_utf8_lossy(&stderr).contains("Operation not permitted"));
                }
            }
            assert_eq!(std::fs::read(&private_file).expect("read test file"), PRIVATE_PAYLOAD.as_bytes());
            server.sessions.shutdown().await;
        });
    });
    let mut expected = Vec::new();
    for index in 0..2 {
        expected.push(expected_process_attributes(
            "codex.exec_server.process_start",
            index,
            "MacosSeatbelt",
        ));
        let mut exited =
            expected_process_attributes("codex.exec_server.process_exit", index, "MacosSeatbelt");
        exited["process.exit_code"] = json!({"intValue": if index == 0 { "0" } else { "23" }});
        exited["process.termination_requested"] = json!({"boolValue": false});
        expected.push(exited);
    }
    let mut denied = expected_process_attributes(
        "codex.exec_server.sandbox_denied",
        /*index*/ 1,
        "MacosSeatbelt",
    );
    denied["reason"] = json!({"stringValue": "inferred_denial"});
    expected.push(denied);
    assert_exported_attributes(&records, expected);
}

async fn read_until_closed(
    router: &RpcRouter<ExecServerHandler>,
    handler: &Arc<ExecServerHandler>,
    process_id: &str,
) -> ReadResponse {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let output: ReadResponse = serde_json::from_value(
                request(
                    router,
                    handler,
                    EXEC_READ_METHOD,
                    json!({"processId": process_id, "waitMs": 100}),
                    Some(LATER_TRACE),
                )
                .await,
            )
            .expect("read process exit");
            if output.closed {
                return output;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("process exits")
}

fn expected_process_attributes(name: &str, index: usize, sandbox: &str) -> Value {
    json!({
        "event.name": {"stringValue": name},
        "launch.trace_id": {"stringValue": TRACE_ID},
        "launch.span_id": {"stringValue": LAUNCH_SPANS[index]},
        "conversation.id": {"stringValue": THREADS[index]},
        "tool.call_id": {"stringValue": CALLS[index]},
        "executor.environment_id": {"stringValue": "environment"},
        "executor.registration_id": {"stringValue": "original"},
        "sandbox.type": {"stringValue": sandbox},
    })
}

fn assert_exported_attributes(records: &[Value], mut expected: Vec<Value>) {
    let mut actual: Vec<Value> = records
        .iter()
        .map(|record| {
            Value::Object(
                record["attributes"]
                    .as_array()
                    .expect("log attributes")
                    .iter()
                    .map(|attribute| {
                        (
                            attribute["key"]
                                .as_str()
                                .expect("attribute key")
                                .to_string(),
                            attribute["value"].clone(),
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    actual.sort_by_key(Value::to_string);
    expected.sort_by_key(Value::to_string);
    assert_eq!(actual, expected);
}

async fn request(
    router: &RpcRouter<ExecServerHandler>,
    handler: &Arc<ExecServerHandler>,
    method: &str,
    params: Value,
    traceparent: Option<&str>,
) -> Value {
    let response = request_result(router, handler, method, params, traceparent).await;
    let Some(RpcServerOutboundMessage::Response { result, .. }) = response else {
        panic!("request failed: {response:?}");
    };
    result
}

async fn request_result(
    router: &RpcRouter<ExecServerHandler>,
    handler: &Arc<ExecServerHandler>,
    method: &str,
    params: Value,
    traceparent: Option<&str>,
) -> Option<RpcServerOutboundMessage> {
    let request = JSONRPCRequest {
        id: RequestId::Integer(1),
        method: method.to_string(),
        params: Some(params),
        trace: traceparent.map(|traceparent| W3cTraceContext {
            traceparent: Some(traceparent.to_string()),
            tracestate: None,
        }),
    };
    let JsonRpcConnectionEvent::QueuedRequest {
        request,
        request_span,
        ..
    } = JsonRpcConnectionEvent::message(JSONRPCMessage::Request(request))
    else {
        panic!("request span")
    };
    let (_, route) = router
        .request_route(method)
        .expect("registered request route");
    request_span.record("otel.name", method);
    route(Arc::clone(handler), request)
        .instrument(request_span)
        .await
}

fn attribute<'a>(record: &'a Value, key: &str) -> Option<&'a str> {
    record["attributes"]
        .as_array()
        .expect("log attributes")
        .iter()
        .find(|attribute| attribute["key"] == key)
        .map(|attribute| {
            attribute["value"]["stringValue"]
                .as_str()
                .expect("string attribute")
        })
}

fn exported_logs(
    export_traces: bool,
    run: impl FnOnce(&tokio::runtime::Runtime, &OtelProvider, &MockServer),
) -> Vec<Value> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let collector = runtime.block_on(MockServer::start());
    runtime.block_on(
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&collector),
    );
    let exporter = OtelExporter::OtlpHttp {
        endpoint: format!("{}/v1/logs", collector.uri()),
        headers: HashMap::new(),
        protocol: OtelHttpProtocol::Json,
        tls: None,
    };
    let otel = OtelProvider::try_new(&OtelSettings {
        http_client_factory: codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
        environment: "test".to_string(),
        service_name: "codex-exec-server".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_home: std::env::current_dir().expect("cwd"),
        exporter,
        trace_exporter: if export_traces {
            OtelExporter::OtlpHttp {
                endpoint: format!("{}/v1/traces", collector.uri()),
                headers: HashMap::new(),
                protocol: OtelHttpProtocol::Json,
                tls: None,
            }
        } else {
            OtelExporter::None
        },
        metrics_exporter: OtelExporter::None,
        runtime_metrics: false,
        span_attributes: BTreeMap::new(),
        tracestate: BTreeMap::new(),
    })
    .expect("OTEL settings")
    .expect("OTEL provider");
    let subscriber = tracing_subscriber::registry()
        .with(otel.tracing_layer())
        .with(otel.logger_layer());
    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        run(&runtime, &otel, &collector);
    });
    runtime
        .block_on(otel.shutdown_with_timeout(Duration::from_secs(5)))
        .expect("flush OTEL");

    let mut records = Vec::new();
    for request in runtime
        .block_on(collector.received_requests())
        .expect("exported requests")
    {
        let body: Value = serde_json::from_slice(&request.body).expect("OTLP JSON");
        let Some(resources) = body["resourceLogs"].as_array() else {
            continue;
        };
        assert!(!String::from_utf8_lossy(&request.body).contains(PRIVATE_PAYLOAD));
        for resource in resources {
            for scope in resource["scopeLogs"].as_array().expect("scope logs") {
                for record in scope["logRecords"].as_array().expect("log records") {
                    assert!(record["body"].is_null(), "lifecycle logs have no raw body");
                    records.push(record.clone());
                }
            }
        }
    }
    records
}

struct TestServer {
    sessions: Arc<SessionRegistry>,
    router: RpcRouter<ExecServerHandler>,
    outgoing: mpsc::Sender<RpcServerOutboundMessage>,
    _notifications: mpsc::Receiver<RpcServerOutboundMessage>,
}

impl TestServer {
    fn new() -> Self {
        let (outgoing, notifications) = mpsc::channel(/*buffer*/ 128);
        Self {
            sessions: SessionRegistry::new(ExecServerTelemetry::default()),
            router: build_router(),
            outgoing,
            _notifications: notifications,
        }
    }

    async fn new_orchestrator(
        &self,
        client_name: &str,
        registration: Option<&str>,
    ) -> (Arc<ExecServerHandler>, String) {
        let handler = new_handler(&self.sessions, self.outgoing.clone(), registration);
        let initialized = handler
            .initialize(InitializeParams {
                client_name: client_name.to_string(),
                resume_session_id: None,
            })
            .await
            .expect("initialize orchestrator");
        handler.initialized().expect("initialized orchestrator");
        (handler, initialized.session_id)
    }
}

fn process_start_params(process_id: &str, argv: Value, cwd: PathUri) -> Value {
    json!({
        "processId": process_id,
        "argv": argv,
        "cwd": cwd,
        "env": {"PRIVATE_TEST_VALUE": PRIVATE_PAYLOAD},
        "tty": false,
        "arg0": null,
    })
}

fn process_metadata(index: usize) -> Value {
    json!({"threadId": THREADS[index], "toolCallId": CALLS[index]})
}

fn new_handler(
    sessions: &Arc<SessionRegistry>,
    outgoing: mpsc::Sender<RpcServerOutboundMessage>,
    registration: Option<&str>,
) -> Arc<ExecServerHandler> {
    let mut handler = ExecServerHandler::new(
        Arc::clone(sessions),
        RpcNotificationSender::new(outgoing),
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("test executable"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths"),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    handler.executor_registration = registration
        .and_then(|registration| {
            ExecutorRegistration::new("environment".to_string(), registration.to_string())
        })
        .map(Arc::new);
    Arc::new(handler)
}
