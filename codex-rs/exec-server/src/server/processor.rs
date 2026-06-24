use std::future::Future;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::Instrument;
use tracing::debug;
use tracing::warn;

use crate::ExecServerRuntimePaths;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::protocol::INITIALIZE_METHOD;
use crate::rpc::RpcNotificationSender;
use crate::rpc::RpcServerOutboundMessage;
use crate::rpc::encode_server_message;
use crate::rpc::invalid_request;
use crate::rpc::method_not_found;
use crate::server::ExecServerHandler;
use crate::server::registry::build_router;
use crate::server::session_registry::SessionRegistry;

type RequestTaskResult = Result<(), mpsc::error::SendError<RpcServerOutboundMessage>>;
type RequestTaskJoinResult = Result<RequestTaskResult, tokio::task::JoinError>;

enum ConnectionActivity {
    Incoming(Option<JsonRpcConnectionEvent>),
    RequestTask(RequestTaskJoinResult),
    RequestTasksDrained,
    Disconnected,
}

#[derive(Clone)]
pub(crate) struct ConnectionProcessor {
    session_registry: Arc<SessionRegistry>,
    runtime_paths: ExecServerRuntimePaths,
}

impl ConnectionProcessor {
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            session_registry: SessionRegistry::new(),
            runtime_paths,
        }
    }

    pub(crate) async fn run_connection(&self, connection: JsonRpcConnection) {
        run_connection(
            connection,
            Arc::clone(&self.session_registry),
            self.runtime_paths.clone(),
        )
        .await;
    }
}

async fn run_connection(
    connection: JsonRpcConnection,
    session_registry: Arc<SessionRegistry>,
    runtime_paths: ExecServerRuntimePaths,
) {
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

    // Run requests independently so one slow request does not block the connection, up to the
    // transport channel capacity per connection.
    let mut request_tasks = JoinSet::<RequestTaskResult>::new();
    'connection: loop {
        let event = match wait_for_connection_activity(
            &mut incoming_rx,
            &mut disconnected_rx,
            &mut request_tasks,
        )
        .await
        {
            ConnectionActivity::Incoming(event) => event,
            ConnectionActivity::RequestTask(Ok(Ok(())))
            | ConnectionActivity::RequestTasksDrained => continue 'connection,
            ConnectionActivity::RequestTask(Ok(Err(_))) => {
                debug!("closing exec-server connection after response channel closed");
                break 'connection;
            }
            ConnectionActivity::RequestTask(Err(err)) => {
                warn!(error = %err, "exec-server request task failed");
                break 'connection;
            }
            ConnectionActivity::Disconnected => {
                debug!("exec-server transport disconnected");
                break 'connection;
            }
        };
        match event {
            Some(_) if !handler.is_session_attached() => {
                warn!("exec-server connection evicted after session resume");
                break 'connection;
            }
            Some(JsonRpcConnectionEvent::MalformedMessage { reason }) => {
                warn!("ignoring malformed exec-server message: {reason}");
                if outgoing_tx
                    .send(RpcServerOutboundMessage::Error {
                        request_id: codex_exec_server_protocol::RequestId::Integer(-1),
                        error: invalid_request(reason),
                    })
                    .await
                    .is_err()
                {
                    break 'connection;
                }
            }
            Some(JsonRpcConnectionEvent::Message(message)) => match message {
                codex_exec_server_protocol::JSONRPCMessage::Request(request) => {
                    // Capture protocol-ordering violations before spawning. Otherwise a later
                    // `initialized` notification could make an early request appear valid before
                    // this task is first polled.
                    let is_initialize = request.method == INITIALIZE_METHOD;
                    let route = router.request_route(request.method.as_str());
                    let initialization_error = route.and_then(|_| {
                        handler.request_initialization_error(request.method.as_str())
                    });
                    let span_name = if route.is_some() {
                        request.method.as_str()
                    } else {
                        "unknown"
                    };
                    let request_span = request_span(span_name, &request);
                    let request_id = request.id.clone();
                    let request_method = request.method.clone();
                    let routed_response = if initialization_error.is_none() {
                        route.map(|route| route(Arc::clone(&handler), request))
                    } else {
                        None
                    };

                    let outgoing_tx = outgoing_tx.clone();
                    let task_span = request_span.clone();
                    let request_task = async move {
                        let message = if let Some(error) = initialization_error {
                            Some(RpcServerOutboundMessage::Error { request_id, error })
                        } else if let Some(response) = routed_response {
                            response.await
                        } else {
                            Some(RpcServerOutboundMessage::Error {
                                request_id,
                                error: method_not_found(format!(
                                    "exec-server stub does not implement `{request_method}` yet"
                                )),
                            })
                        };
                        let result = request_result(&message);
                        if let Some(message) = message {
                            // The sole receiver belongs to the outbound encoder task. A send error
                            // means the connection cannot deliver any more responses.
                            outgoing_tx.send(message).await?;
                        }
                        request_span.record("result", result);
                        Ok(())
                    }
                    .instrument(task_span);

                    if is_initialize {
                        // `initialize` claims connection state inside its route. Await it before
                        // admitting another frame so duplicate handshakes are ordered by the wire,
                        // rather than by which spawned task the scheduler polls first.
                        let Some(result) =
                            await_or_disconnect(request_task, &mut disconnected_rx).await
                        else {
                            debug!("exec-server transport disconnected while handling initialize");
                            break 'connection;
                        };
                        if result.is_err() {
                            debug!("closing exec-server connection after response channel closed");
                            break 'connection;
                        }
                    } else {
                        request_tasks.spawn(request_task);
                    }
                }
                codex_exec_server_protocol::JSONRPCMessage::Notification(notification) => {
                    // Notifications stay inline because `initialized` is an ordering barrier:
                    // later requests must latch state after it runs, and an invalid notification
                    // must close the connection before another frame is admitted. It is currently
                    // the only notification route and performs no asynchronous work.
                    let Some(route) = router.notification_route(notification.method.as_str())
                    else {
                        warn!(
                            "closing exec-server connection after unexpected notification: {}",
                            notification.method
                        );
                        break 'connection;
                    };
                    let Some(result) = await_or_disconnect(
                        route(Arc::clone(&handler), notification),
                        &mut disconnected_rx,
                    )
                    .await
                    else {
                        debug!("exec-server transport disconnected while handling notification");
                        break 'connection;
                    };
                    if let Err(err) = result {
                        warn!("closing exec-server connection after protocol error: {err}");
                        break 'connection;
                    }
                }
                codex_exec_server_protocol::JSONRPCMessage::Response(response) => {
                    warn!(
                        "closing exec-server connection after unexpected client response: {:?}",
                        response.id
                    );
                    break 'connection;
                }
                codex_exec_server_protocol::JSONRPCMessage::Error(error) => {
                    warn!(
                        "closing exec-server connection after unexpected client error: {:?}",
                        error.id
                    );
                    break 'connection;
                }
            },
            Some(JsonRpcConnectionEvent::Disconnected { reason }) => {
                if let Some(reason) = reason {
                    debug!("exec-server connection disconnected: {reason}");
                }
                break 'connection;
            }
            None => {
                debug!("exec-server incoming event channel closed");
                break 'connection;
            }
        }
    }

    // Abort and await requests before handler shutdown clears any incomplete process starts and
    // detaches the session. Long polls therefore cannot delay resume, and no request can mutate the
    // session after its `Starting` entries have been swept.
    request_tasks.shutdown().await;
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
        // An aborted request drops the span with this fallback. Completed requests overwrite it.
        result = "disconnected",
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

async fn wait_for_connection_activity(
    incoming_rx: &mut mpsc::Receiver<JsonRpcConnectionEvent>,
    disconnected_rx: &mut watch::Receiver<bool>,
    request_tasks: &mut JoinSet<RequestTaskResult>,
) -> ConnectionActivity {
    let has_request_tasks = !request_tasks.is_empty();
    let can_receive_event = request_tasks.len() < CHANNEL_CAPACITY;
    // All three futures are cancellation safe, so the branches that lose this race retain their
    // next event or task completion for the following iteration. At the request limit, stop
    // draining the bounded incoming channel until a task completes, propagating backpressure to
    // the transport. Ready task completions cannot starve input because no new request tasks are
    // added while this wait reaps them.
    tokio::select! {
        biased;
        _ = disconnected_rx.changed() => ConnectionActivity::Disconnected,
        result = request_tasks.join_next(), if has_request_tasks => {
            match result {
                Some(result) => ConnectionActivity::RequestTask(result),
                None => ConnectionActivity::RequestTasksDrained,
            }
        }
        // Keep incoming events last so a disconnect or terminal task failure stops the connection
        // before it admits another request.
        event = incoming_rx.recv(), if can_receive_event => ConnectionActivity::Incoming(event),
    }
}

async fn await_or_disconnect<F>(
    future: F,
    disconnected_rx: &mut watch::Receiver<bool>,
) -> Option<F::Output>
where
    F: Future,
{
    // `watch::Receiver::changed` is cancellation safe. The notification future is intentionally
    // dropped on disconnect because its result can no longer be delivered and connection teardown
    // starts immediately.
    tokio::select! {
        output = future => Some(output),
        _ = disconnected_rx.changed() => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use codex_exec_server_protocol::JSONRPCError;
    use codex_exec_server_protocol::JSONRPCErrorError;
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
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio::sync::watch;
    use tokio::task::JoinHandle;
    use tokio::task::JoinSet;
    use tokio::time::timeout;
    use tracing_subscriber::filter::filter_fn;
    use tracing_subscriber::prelude::*;

    use super::ConnectionActivity;
    use super::RequestTaskResult;
    use super::request_span;
    use super::run_connection;
    use super::wait_for_connection_activity;
    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use crate::connection::CHANNEL_CAPACITY;
    use crate::connection::JsonRpcConnection;
    use crate::connection::JsonRpcConnectionEvent;
    use crate::connection::JsonRpcTransport;
    use crate::protocol::ENVIRONMENT_INFO_METHOD;
    use crate::protocol::EXEC_METHOD;
    use crate::protocol::EXEC_READ_METHOD;
    use crate::protocol::EXEC_TERMINATE_METHOD;
    use crate::protocol::EnvironmentInfo;
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
        let registry = SessionRegistry::new();
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

        let (first_id, _first_response) =
            read_response_with_id::<EnvironmentInfo>(&mut lines).await;
        let (second_id, _second_response) =
            read_response_with_id::<EnvironmentInfo>(&mut lines).await;
        let mut response_ids = [first_id, second_id];
        response_ids.sort();
        assert_eq!(response_ids, [RequestId::Integer(2), RequestId::Integer(3)]);

        drop(writer);
        drop(lines);
        timeout(Duration::from_secs(1), task)
            .await
            .expect("processor should exit")
            .expect("processor should join");
    }

    #[tokio::test]
    async fn initialized_before_initialize_closes_connection() {
        let registry = SessionRegistry::new();
        let (mut writer, mut lines, task) =
            spawn_test_connection(registry, "initialized-before-initialize");

        send_notification(&mut writer, INITIALIZED_METHOD, &()).await;

        timeout(Duration::from_secs(1), task)
            .await
            .expect("processor should reject initialized before initialize")
            .expect("processor should join");
        assert_eq!(lines.next_line().await.expect("read connection EOF"), None);
        drop(writer);
    }

    #[tokio::test]
    async fn duplicate_initialize_requests_use_wire_order() {
        let registry = SessionRegistry::new();
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (_disconnected_tx, disconnected_rx) = watch::channel(false);
        for (id, resume_session_id) in [(1, None), (2, Some("must-not-be-used"))] {
            incoming_tx
                .send(JsonRpcConnectionEvent::Message(JSONRPCMessage::Request(
                    JSONRPCRequest {
                        id: RequestId::Integer(id),
                        method: INITIALIZE_METHOD.to_string(),
                        params: Some(
                            serde_json::to_value(InitializeParams {
                                client_name: format!("client-{id}"),
                                resume_session_id: resume_session_id.map(str::to_string),
                            })
                            .expect("serialize initialize params"),
                        ),
                        trace: None,
                    },
                )))
                .await
                .expect("incoming channel should remain open");
        }
        let connection = JsonRpcConnection {
            outgoing_tx,
            incoming_rx,
            disconnected_rx,
            task_handles: Vec::new(),
            transport: JsonRpcTransport::Plain,
        };
        let task = tokio::spawn(run_connection(connection, registry, test_runtime_paths()));

        let mut first_response = None;
        let mut second_error = None;
        for _ in 0..2 {
            let message = timeout(Duration::from_secs(1), outgoing_rx.recv())
                .await
                .expect("initialize request should complete")
                .expect("outgoing channel should remain open");
            match message {
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: RequestId::Integer(1),
                    result,
                }) => {
                    first_response = Some(
                        serde_json::from_value::<InitializeResponse>(result)
                            .expect("decode initialize response"),
                    );
                }
                JSONRPCMessage::Error(
                    error @ JSONRPCError {
                        id: RequestId::Integer(2),
                        ..
                    },
                ) => {
                    second_error = Some(error);
                }
                other => panic!("unexpected initialize result: {other:?}"),
            }
        }

        assert!(first_response.is_some());
        assert_eq!(
            second_error,
            Some(JSONRPCError {
                id: RequestId::Integer(2),
                error: JSONRPCErrorError {
                    code: -32600,
                    message: "initialize may only be sent once per connection".to_string(),
                    data: None,
                },
            })
        );

        drop(incoming_tx);
        drop(outgoing_rx);
        timeout(Duration::from_secs(1), task)
            .await
            .expect("processor should exit")
            .expect("processor should join");
    }

    #[tokio::test]
    async fn request_before_initialized_returns_error_without_closing_connection() {
        let registry = SessionRegistry::new();
        let (mut writer, mut lines, task) =
            spawn_test_connection(registry, "request-before-initialized");

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

        send_request(&mut writer, /*id*/ 2, ENVIRONMENT_INFO_METHOD, &()).await;
        // Pipeline `initialized` behind the invalid request. The request must still fail based on
        // its position in the incoming stream, even if its spawned task runs after the notification.
        send_notification(&mut writer, INITIALIZED_METHOD, &()).await;
        assert_eq!(
            read_error(&mut lines).await,
            JSONRPCError {
                id: RequestId::Integer(2),
                error: JSONRPCErrorError {
                    code: -32600,
                    data: None,
                    message: "client must send initialized before invoking `environment/info`"
                        .to_string(),
                },
            }
        );

        send_request(&mut writer, /*id*/ 3, ENVIRONMENT_INFO_METHOD, &()).await;
        let _: EnvironmentInfo = read_response(&mut lines, /*expected_id*/ 3).await;

        drop(writer);
        drop(lines);
        timeout(Duration::from_secs(1), task)
            .await
            .expect("processor should exit")
            .expect("processor should join");
    }

    #[tokio::test]
    async fn in_flight_read_does_not_block_independent_request() {
        let registry = SessionRegistry::new();
        let (mut writer, mut lines, task) =
            spawn_test_connection(Arc::clone(&registry), "concurrent-read");

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

        let process_id = ProcessId::from("proc-concurrent-read");
        send_request(
            &mut writer,
            /*id*/ 2,
            EXEC_METHOD,
            &exec_params_with_argv(process_id.clone(), long_running_process_argv()),
        )
        .await;
        let _: ExecResponse = read_response(&mut lines, /*expected_id*/ 2).await;

        send_request(
            &mut writer,
            /*id*/ 3,
            EXEC_READ_METHOD,
            &ReadParams {
                process_id: process_id.clone(),
                after_seq: None,
                max_bytes: None,
                wait_ms: Some(600_000),
            },
        )
        .await;
        send_request(&mut writer, /*id*/ 4, ENVIRONMENT_INFO_METHOD, &()).await;

        // The ordered transport admits request 3 first, but its process stays alive without output
        // and its read deadline is ten minutes away. Receiving response 4 next therefore proves
        // that the read did not block request handling without relying on a short wall-clock
        // assertion.
        let _: EnvironmentInfo = read_response(&mut lines, /*expected_id*/ 4).await;

        timeout(
            Duration::from_secs(2),
            terminate_process(&mut writer, &mut lines, /*request_id*/ 5, process_id),
        )
        .await
        .expect("process should terminate");

        drop(writer);
        drop(lines);
        timeout(Duration::from_secs(1), task)
            .await
            .expect("processor should exit")
            .expect("processor should join");
    }

    #[tokio::test]
    async fn response_sink_failure_closes_connection() {
        let registry = SessionRegistry::new();
        let (mut writer, mut lines, task) = spawn_test_connection(registry, "closed-response-sink");

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

        drop(lines);
        send_request(&mut writer, /*id*/ 2, ENVIRONMENT_INFO_METHOD, &()).await;
        timeout(Duration::from_secs(1), task)
            .await
            .expect("processor should exit after the response sink fails")
            .expect("processor should join");
        drop(writer);
    }

    #[tokio::test]
    async fn request_task_failure_wakes_idle_connection_wait() {
        let (_incoming_tx, mut incoming_rx) = mpsc::channel(1);
        let (_disconnected_tx, mut disconnected_rx) = watch::channel(false);
        let mut request_tasks = JoinSet::<RequestTaskResult>::new();
        request_tasks.spawn(async { panic!("intentional request task panic") });

        let activity = timeout(
            Duration::from_secs(1),
            wait_for_connection_activity(
                &mut incoming_rx,
                &mut disconnected_rx,
                &mut request_tasks,
            ),
        )
        .await
        .expect("request task failure should wake the idle connection wait");
        let ConnectionActivity::RequestTask(Err(err)) = activity else {
            panic!("expected failed request task activity");
        };
        assert!(err.is_panic());
    }

    #[tokio::test]
    async fn incoming_events_wait_for_request_capacity() {
        let (incoming_tx, mut incoming_rx) = mpsc::channel(1);
        let (_disconnected_tx, mut disconnected_rx) = watch::channel(false);
        let mut request_tasks = JoinSet::<RequestTaskResult>::new();
        incoming_tx
            .send(JsonRpcConnectionEvent::MalformedMessage {
                reason: "queued".to_string(),
            })
            .await
            .expect("incoming channel should remain open");

        for _ in 1..CHANNEL_CAPACITY {
            request_tasks.spawn(std::future::pending::<RequestTaskResult>());
        }
        let (release_tx, release_rx) = oneshot::channel();
        request_tasks.spawn(async move {
            release_rx.await.expect("request task should be released");
            Ok(())
        });
        assert_eq!(request_tasks.len(), CHANNEL_CAPACITY);

        let mut activity = Box::pin(wait_for_connection_activity(
            &mut incoming_rx,
            &mut disconnected_rx,
            &mut request_tasks,
        ));
        tokio::select! {
            biased;
            _ = &mut activity => panic!("incoming event bypassed request backpressure"),
            _ = tokio::task::yield_now() => {}
        }

        release_tx
            .send(())
            .expect("request task should remain live");
        let ConnectionActivity::RequestTask(Ok(Ok(()))) = activity.await else {
            panic!("expected the released request task to complete");
        };

        let activity = wait_for_connection_activity(
            &mut incoming_rx,
            &mut disconnected_rx,
            &mut request_tasks,
        )
        .await;
        let ConnectionActivity::Incoming(Some(JsonRpcConnectionEvent::MalformedMessage { reason })) =
            activity
        else {
            panic!("expected the queued incoming event after capacity opened");
        };
        assert_eq!(reason, "queued");

        request_tasks.shutdown().await;
    }

    #[tokio::test(start_paused = true)]
    async fn transport_disconnect_with_in_flight_read_allows_session_resume() {
        // Keep the test-only detached-session TTL from racing session resume on a loaded host.
        let registry = SessionRegistry::new();
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

        let process_id = ProcessId::from("proc-disconnect-read");
        send_request(
            &mut first_writer,
            /*id*/ 2,
            EXEC_METHOD,
            &exec_params_with_argv(process_id.clone(), long_running_process_argv()),
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
                wait_ms: Some(600_000),
            },
        )
        .await;
        // The malformed frame is processed inline after request 3. Its response is an ordered
        // barrier proving the long read was admitted to the task set before disconnect.
        send_malformed_message(&mut first_writer).await;
        let _ = read_error(&mut first_lines).await;

        drop(first_writer);
        drop(first_lines);
        timeout(Duration::from_secs(1), first_task)
            .await
            .expect("first processor should exit")
            .expect("first processor should join");

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
        let second_initialize_response: InitializeResponse =
            read_response(&mut second_lines, /*expected_id*/ 1).await;
        assert_eq!(
            second_initialize_response.session_id,
            initialize_response.session_id
        );
        send_notification(&mut second_writer, INITIALIZED_METHOD, &()).await;
        terminate_process(
            &mut second_writer,
            &mut second_lines,
            /*request_id*/ 2,
            process_id,
        )
        .await;

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
        let task = tokio::spawn(run_connection(connection, registry, test_runtime_paths()));
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

    async fn send_malformed_message(writer: &mut DuplexStream) {
        writer
            .write_all(b"not-json\n")
            .await
            .expect("write malformed message");
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
        let (id, response) = read_response_with_id(lines).await;
        assert_eq!(id, RequestId::Integer(expected_id));
        response
    }

    async fn read_response_with_id<T: DeserializeOwned>(
        lines: &mut Lines<BufReader<DuplexStream>>,
    ) -> (RequestId, T) {
        match read_message(lines).await {
            JSONRPCMessage::Response(JSONRPCResponse { id, result }) => {
                let response = serde_json::from_value(result).expect("decode response result");
                (id, response)
            }
            JSONRPCMessage::Error(error) => panic!("unexpected JSON-RPC error: {error:?}"),
            other => panic!("expected JSON-RPC response, got {other:?}"),
        }
    }

    async fn read_error(lines: &mut Lines<BufReader<DuplexStream>>) -> JSONRPCError {
        match read_message(lines).await {
            JSONRPCMessage::Error(error) => error,
            other => panic!("expected JSON-RPC error, got {other:?}"),
        }
    }

    async fn terminate_process(
        writer: &mut DuplexStream,
        lines: &mut Lines<BufReader<DuplexStream>>,
        request_id: i64,
        process_id: ProcessId,
    ) {
        send_request(
            writer,
            request_id,
            EXEC_TERMINATE_METHOD,
            &TerminateParams { process_id },
        )
        .await;

        loop {
            match read_message(lines).await {
                JSONRPCMessage::Response(JSONRPCResponse { id, result })
                    if id == RequestId::Integer(request_id) =>
                {
                    let _: TerminateResponse =
                        serde_json::from_value(result).expect("decode terminate response");
                    return;
                }
                JSONRPCMessage::Response(_) | JSONRPCMessage::Notification(_) => {}
                JSONRPCMessage::Error(error) => {
                    panic!("unexpected JSON-RPC error: {error:?}")
                }
                other => panic!("expected JSON-RPC response or notification, got {other:?}"),
            }
        }
    }

    async fn read_message(lines: &mut Lines<BufReader<DuplexStream>>) -> JSONRPCMessage {
        let line = lines
            .next_line()
            .await
            .expect("read response")
            .expect("response line");
        serde_json::from_str(&line).expect("decode JSON-RPC message")
    }

    fn exec_params_with_argv(process_id: ProcessId, argv: Vec<String>) -> ExecParams {
        let mut env = HashMap::new();
        if let Some(path) = std::env::var_os("PATH") {
            env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
        }
        ExecParams {
            process_id,
            argv,
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

    fn long_running_process_argv() -> Vec<String> {
        if cfg!(windows) {
            vec![
                std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string()),
                "/C".to_string(),
                "ping -n 3601 127.0.0.1 >NUL".to_string(),
            ]
        } else {
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 3600".to_string(),
            ]
        }
    }
}
