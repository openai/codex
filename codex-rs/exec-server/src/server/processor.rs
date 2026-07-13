use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::Instrument;
use tracing::debug;
use tracing::warn;

use crate::ExecServerRuntimePaths;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::rpc::RpcNotificationSender;
use crate::rpc::RpcServerOutboundMessage;
use crate::rpc::encode_server_message;
use crate::rpc::invalid_request;
use crate::rpc::method_not_found;
use crate::server::ExecServerHandler;
use crate::server::registry::build_router;
use crate::server::session_registry::SessionRegistry;
use crate::telemetry::ConnectionTransport;
use crate::telemetry::ExecServerTelemetry;

#[derive(Clone)]
pub(crate) struct ConnectionProcessor {
    session_registry: Arc<SessionRegistry>,
    runtime_paths: ExecServerRuntimePaths,
    telemetry: ExecServerTelemetry,
}

impl ConnectionProcessor {
    #[cfg(test)]
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self::new_with_telemetry(runtime_paths, ExecServerTelemetry::default())
    }

    pub(crate) fn new_with_telemetry(
        runtime_paths: ExecServerRuntimePaths,
        telemetry: ExecServerTelemetry,
    ) -> Self {
        Self {
            session_registry: SessionRegistry::new(telemetry.clone()),
            runtime_paths,
            telemetry,
        }
    }

    pub(crate) async fn run_connection(
        &self,
        connection: JsonRpcConnection,
        transport: ConnectionTransport,
    ) {
        run_connection(
            connection,
            Arc::clone(&self.session_registry),
            self.runtime_paths.clone(),
            self.telemetry.clone(),
            transport,
        )
        .await;
    }

    pub(crate) async fn shutdown(&self) {
        self.session_registry.shutdown().await;
    }
}

async fn run_connection(
    connection: JsonRpcConnection,
    session_registry: Arc<SessionRegistry>,
    runtime_paths: ExecServerRuntimePaths,
    telemetry: ExecServerTelemetry,
    transport: ConnectionTransport,
) {
    let _connection_metrics = telemetry.connection_started(transport);
    let router = Arc::new(build_router());
    let JsonRpcConnection {
        outgoing_tx: json_outgoing_tx,
        mut incoming_rx,
        mut disconnected_rx,
        task_handles: connection_tasks,
        transport: _transport,
    } = connection;
    let (outgoing_tx, mut outgoing_rx) =
        mpsc::channel::<RpcServerOutboundMessage>(CHANNEL_CAPACITY);
    let notifications = RpcNotificationSender::new(outgoing_tx.clone());
    let handler = Arc::new(ExecServerHandler::new(
        session_registry,
        notifications,
        runtime_paths,
    ));

    let outbound_task = tokio::spawn(async move {
        while let Some(message) = outgoing_rx.recv().await {
            let json_message = match encode_server_message(message) {
                Ok(json_message) => json_message,
                Err(err) => {
                    warn!("failed to serialize exec-server outbound message: {err}");
                    break;
                }
            };
            if json_outgoing_tx.send(json_message).await.is_err() {
                break;
            }
        }
    });

    // Process inbound events sequentially to preserve initialize/initialized ordering.
    while let Some(event) = incoming_rx.recv().await {
        if !handler.is_session_attached() {
            debug!("exec-server connection evicted after session resume");
            break;
        }
        match event {
            JsonRpcConnectionEvent::MalformedMessage { reason } => {
                warn!("ignoring malformed exec-server message: {reason}");
                if outgoing_tx
                    .send(RpcServerOutboundMessage::Error {
                        request_id: codex_exec_server_protocol::RequestId::Integer(-1),
                        error: invalid_request(reason),
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            JsonRpcConnectionEvent::Message(message) => match message {
                codex_exec_server_protocol::JSONRPCMessage::Request(request) => {
                    let request_started_at = Instant::now();
                    if let Some((method, route)) = router.request_route(request.method.as_str()) {
                        let request_span = request_span(method, &request);
                        let message = tokio::select! {
                            message = route(Arc::clone(&handler), request).instrument(request_span.clone()) => message,
                            _ = disconnected_rx.changed() => {
                                request_span.record("result", "disconnected");
                                telemetry.request_completed(
                                    method,
                                    "disconnected",
                                    request_started_at.elapsed(),
                                );
                                debug!("exec-server transport disconnected while handling request");
                                break;
                            }
                        };
                        let result = request_result(&message);
                        if let Some(message) = message
                            && outgoing_tx.send(message).await.is_err()
                        {
                            request_span.record("result", "disconnected");
                            telemetry.request_completed(
                                method,
                                "disconnected",
                                request_started_at.elapsed(),
                            );
                            break;
                        }
                        request_span.record("result", result);
                        telemetry.request_completed(method, result, request_started_at.elapsed());
                        drop(request_span);
                    } else {
                        let method = "unknown";
                        let request_span = request_span(method, &request);
                        if outgoing_tx
                            .send(RpcServerOutboundMessage::Error {
                                request_id: request.id,
                                error: method_not_found(format!(
                                    "exec-server stub does not implement `{}` yet",
                                    request.method
                                )),
                            })
                            .await
                            .is_err()
                        {
                            request_span.record("result", "disconnected");
                            telemetry.request_completed(
                                method,
                                "disconnected",
                                request_started_at.elapsed(),
                            );
                            break;
                        }
                        request_span.record("result", "error");
                        telemetry.request_completed(method, "error", request_started_at.elapsed());
                    }
                }
                codex_exec_server_protocol::JSONRPCMessage::Notification(notification) => {
                    let Some(route) = router.notification_route(notification.method.as_str())
                    else {
                        warn!(
                            "closing exec-server connection after unexpected notification: {}",
                            notification.method
                        );
                        break;
                    };
                    let result = tokio::select! {
                        result = route(Arc::clone(&handler), notification) => result,
                        _ = disconnected_rx.changed() => {
                            debug!(
                                "exec-server transport disconnected while handling notification"
                            );
                            break;
                        }
                    };
                    if let Err(err) = result {
                        warn!("closing exec-server connection after protocol error: {err}");
                        break;
                    }
                }
                codex_exec_server_protocol::JSONRPCMessage::Response(response) => {
                    warn!(
                        "closing exec-server connection after unexpected client response: {:?}",
                        response.id
                    );
                    break;
                }
                codex_exec_server_protocol::JSONRPCMessage::Error(error) => {
                    warn!(
                        "closing exec-server connection after unexpected client error: {:?}",
                        error.id
                    );
                    break;
                }
            },
            JsonRpcConnectionEvent::Disconnected { reason } => {
                if let Some(reason) = reason {
                    debug!("exec-server connection disconnected: {reason}");
                }
                break;
            }
        }
    }

    handler.shutdown().await;
    drop(handler);
    drop(outgoing_tx);
    for task in connection_tasks {
        task.abort();
        let _ = task.await;
    }
    let _ = outbound_task.await;
}

fn request_span(
    span_name: &str,
    request: &codex_exec_server_protocol::JSONRPCRequest,
) -> tracing::Span {
    let method = request.method.as_str();
    let span = tracing::info_span!(
        "codex.exec_server.request",
        otel.kind = "server",
        otel.name = span_name,
        method,
        result = tracing::field::Empty,
    );
    if let Some(trace) = &request.trace
        && !codex_otel::set_parent_from_w3c_trace_context(&span, trace)
    {
        warn!(method, "ignoring invalid inbound exec-server trace carrier");
    }
    span
}

fn request_result(message: &Option<RpcServerOutboundMessage>) -> &'static str {
    match message {
        Some(RpcServerOutboundMessage::Error { .. }) => "error",
        Some(
            RpcServerOutboundMessage::Response { .. } | RpcServerOutboundMessage::Notification(_),
        )
        | None => "success",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use codex_exec_server_protocol::JSONRPCMessage;
    use codex_exec_server_protocol::JSONRPCNotification;
    use codex_exec_server_protocol::JSONRPCRequest;
    use codex_exec_server_protocol::JSONRPCResponse;
    use codex_exec_server_protocol::RequestId;
    use codex_utils_path_uri::PathUri;
    use opentelemetry::trace::SpanId;
    use opentelemetry::trace::TraceId;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::InMemorySpanExporter;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::io::DuplexStream;
    use tokio::io::Lines;
    use tokio::io::duplex;
    use tokio::task::JoinHandle;
    use tokio::time::timeout;
    use tracing_subscriber::filter::filter_fn;
    use tracing_subscriber::prelude::*;

    use super::request_span;
    use super::run_connection;
    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use crate::connection::JsonRpcConnection;
    use crate::protocol::ENVIRONMENT_INFO_METHOD;
    use crate::protocol::ENVIRONMENT_STATUS_METHOD;
    use crate::protocol::EXEC_METHOD;
    use crate::protocol::EXEC_READ_METHOD;
    use crate::protocol::EXEC_TERMINATE_METHOD;
    use crate::protocol::EnvironmentInfo;
    use crate::protocol::EnvironmentStatus;
    use crate::protocol::EnvironmentStatusKind;
    use crate::protocol::ExecParams;
    use crate::protocol::ExecResponse;
    use crate::protocol::INITIALIZE_METHOD;
    use crate::protocol::INITIALIZED_METHOD;
    use crate::protocol::InitializeParams;
    use crate::protocol::InitializeResponse;
    use crate::protocol::ReadParams;
    use crate::protocol::TerminateParams;
    use crate::protocol::TerminateResponse;
    use crate::server::session_registry::SessionRegistry;

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
            let request_span = request_span("unknown", &request);
            request_span.in_scope(|| {});
            drop(request_span);
        });

        tracer_provider.force_flush().expect("flush traces");
        let spans = span_exporter.get_finished_spans().expect("span export");
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

    #[tokio::test]
    async fn connection_accepts_pipelined_scalar_requests() {
        let registry = SessionRegistry::new(crate::ExecServerTelemetry::default());
        let (mut writer, mut lines, task) = spawn_test_connection(registry, "pipelined-scalar");

        send_request(
            &mut writer,
            /*id*/ 1,
            INITIALIZE_METHOD,
            &InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            },
        )
        .await;
        let _: InitializeResponse = read_response(&mut lines, /*expected_id*/ 1).await;
        send_notification(&mut writer, INITIALIZED_METHOD, &()).await;

        send_request(&mut writer, /*id*/ 2, ENVIRONMENT_INFO_METHOD, &()).await;
        send_request(&mut writer, /*id*/ 3, ENVIRONMENT_INFO_METHOD, &()).await;
        send_request(&mut writer, /*id*/ 4, ENVIRONMENT_STATUS_METHOD, &()).await;

        let _: EnvironmentInfo = read_response(&mut lines, /*expected_id*/ 2).await;
        let _: EnvironmentInfo = read_response(&mut lines, /*expected_id*/ 3).await;
        assert_eq!(
            read_response::<EnvironmentStatus>(&mut lines, /*expected_id*/ 4).await,
            EnvironmentStatus {
                status: EnvironmentStatusKind::Ready,
            }
        );

        drop(writer);
        drop(lines);
        timeout(Duration::from_secs(1), task)
            .await
            .expect("processor should exit")
            .expect("processor should join");
    }

    #[tokio::test]
    async fn transport_disconnect_detaches_session_during_in_flight_read() {
        let registry = SessionRegistry::new(crate::ExecServerTelemetry::default());
        let (mut first_writer, mut first_lines, first_task) =
            spawn_test_connection(Arc::clone(&registry), "first");

        send_request(
            &mut first_writer,
            /*id*/ 1,
            INITIALIZE_METHOD,
            &InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            },
        )
        .await;
        let initialize_response: InitializeResponse =
            read_response(&mut first_lines, /*expected_id*/ 1).await;
        send_notification(&mut first_writer, INITIALIZED_METHOD, &()).await;

        let process_id = ProcessId::from("proc-long-poll");
        send_request(
            &mut first_writer,
            /*id*/ 2,
            EXEC_METHOD,
            &exec_params(process_id.clone()),
        )
        .await;
        let _: ExecResponse = read_response(&mut first_lines, /*expected_id*/ 2).await;

        send_request(
            &mut first_writer,
            /*id*/ 3,
            EXEC_READ_METHOD,
            &ReadParams {
                process_id: process_id.clone(),
                after_seq: None,
                max_bytes: None,
                wait_ms: Some(5_000),
            },
        )
        .await;
        drop(first_writer);
        tokio::time::sleep(Duration::from_millis(25)).await;

        let (mut second_writer, mut second_lines, second_task) =
            spawn_test_connection(Arc::clone(&registry), "second");
        send_request(
            &mut second_writer,
            /*id*/ 1,
            INITIALIZE_METHOD,
            &InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: Some(initialize_response.session_id.clone()),
            },
        )
        .await;
        let second_initialize_response = timeout(
            Duration::from_secs(1),
            read_response::<InitializeResponse>(&mut second_lines, /*expected_id*/ 1),
        )
        .await
        .expect("resume initialize should not wait for the old read to finish");
        assert_eq!(
            second_initialize_response.session_id,
            initialize_response.session_id
        );
        timeout(Duration::from_secs(1), first_task)
            .await
            .expect("first processor should exit")
            .expect("first processor should join");
        send_notification(&mut second_writer, INITIALIZED_METHOD, &()).await;

        send_request(
            &mut second_writer,
            /*id*/ 2,
            EXEC_TERMINATE_METHOD,
            &TerminateParams { process_id },
        )
        .await;
        let _: TerminateResponse = read_response(&mut second_lines, /*expected_id*/ 2).await;

        drop(second_writer);
        drop(second_lines);
        timeout(Duration::from_secs(1), second_task)
            .await
            .expect("second processor should exit")
            .expect("second processor should join");
    }

    fn spawn_test_connection(
        registry: Arc<SessionRegistry>,
        label: &str,
    ) -> (DuplexStream, Lines<BufReader<DuplexStream>>, JoinHandle<()>) {
        let (client_writer, server_reader) = duplex(1 << 20);
        let (server_writer, client_reader) = duplex(1 << 20);
        let connection =
            JsonRpcConnection::from_stdio(server_reader, server_writer, label.to_string());
        let task = tokio::spawn(run_connection(
            connection,
            registry,
            test_runtime_paths(),
            crate::ExecServerTelemetry::default(),
            crate::telemetry::ConnectionTransport::Stdio,
        ));
        (client_writer, BufReader::new(client_reader).lines(), task)
    }

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    async fn send_request<P: Serialize>(
        writer: &mut DuplexStream,
        id: i64,
        method: &str,
        params: &P,
    ) {
        write_message(
            writer,
            &JSONRPCMessage::Request(JSONRPCRequest {
                id: RequestId::Integer(id),
                method: method.to_string(),
                params: Some(serde_json::to_value(params).expect("serialize params")),
                trace: None,
            }),
        )
        .await;
    }

    async fn send_notification<P: Serialize>(writer: &mut DuplexStream, method: &str, params: &P) {
        write_message(
            writer,
            &JSONRPCMessage::Notification(JSONRPCNotification {
                method: method.to_string(),
                params: Some(serde_json::to_value(params).expect("serialize params")),
            }),
        )
        .await;
    }

    async fn write_message(writer: &mut DuplexStream, message: &JSONRPCMessage) {
        let encoded = serde_json::to_vec(message).expect("serialize JSON-RPC message");
        writer.write_all(&encoded).await.expect("write request");
        writer.write_all(b"\n").await.expect("write newline");
    }

    async fn read_response<T: DeserializeOwned>(
        lines: &mut Lines<BufReader<DuplexStream>>,
        expected_id: i64,
    ) -> T {
        let line = lines
            .next_line()
            .await
            .expect("read response")
            .expect("response line");
        match serde_json::from_str::<JSONRPCMessage>(&line).expect("decode JSON-RPC response") {
            JSONRPCMessage::Response(JSONRPCResponse { id, result }) => {
                assert_eq!(id, RequestId::Integer(expected_id));
                serde_json::from_value(result).expect("decode response result")
            }
            JSONRPCMessage::Error(error) => panic!("unexpected JSON-RPC error: {error:?}"),
            other => panic!("expected JSON-RPC response, got {other:?}"),
        }
    }

    fn exec_params(process_id: ProcessId) -> ExecParams {
        let mut env = HashMap::new();
        if let Some(path) = std::env::var_os("PATH") {
            env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
        }
        ExecParams {
            process_id,
            argv: sleep_then_print_argv(),
            cwd: PathUri::from_host_native_path(std::env::current_dir().expect("cwd"))
                .expect("cwd URI"),
            env_policy: None,
            env,
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
        }
    }

    fn sleep_then_print_argv() -> Vec<String> {
        if cfg!(windows) {
            vec![
                std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string()),
                "/C".to_string(),
                "ping -n 3 127.0.0.1 >NUL && echo late".to_string(),
            ]
        } else {
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 1; printf late".to_string(),
            ]
        }
    }
}
