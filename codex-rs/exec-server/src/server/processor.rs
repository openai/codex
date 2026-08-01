use std::sync::Arc;

use codex_exec_server_protocol::JSONRPCMessage;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::warn;

use crate::ExecServerRuntimePaths;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::rpc::RpcNotificationSender;
use crate::rpc::RpcServerOutboundMessage;
use crate::rpc::encode_server_message;
use crate::server::ExecServerHandler;
use crate::server::registry::build_router;
use crate::server::request_dispatcher::RequestDispatcher;
use crate::server::request_dispatcher::RequestTaskResult;
use crate::server::session_registry::SessionRegistry;
use crate::telemetry::ConnectionTransport;
use crate::telemetry::ExecServerTelemetry;
use codex_http_client::HttpClientFactory;

#[derive(Clone)]
pub(crate) struct ConnectionProcessor {
    session_registry: Arc<SessionRegistry>,
    runtime_paths: ExecServerRuntimePaths,
    telemetry: ExecServerTelemetry,
    http_client_factory: HttpClientFactory,
}

impl ConnectionProcessor {
    #[cfg(test)]
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self::new_with_telemetry(
            runtime_paths,
            ExecServerTelemetry::default(),
            codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
        )
    }

    pub(crate) fn new_with_telemetry(
        runtime_paths: ExecServerRuntimePaths,
        telemetry: ExecServerTelemetry,
        http_client_factory: HttpClientFactory,
    ) -> Self {
        Self {
            session_registry: SessionRegistry::new(telemetry.clone()),
            runtime_paths,
            telemetry,
            http_client_factory,
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
            self.http_client_factory.clone(),
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
    http_client_factory: HttpClientFactory,
    transport: ConnectionTransport,
) {
    let _connection_metrics = telemetry.connection_started(transport);
    let router = Arc::new(build_router());
    let JsonRpcConnection {
        outgoing_tx: json_outgoing_tx,
        mut incoming_rx,
        disconnected_rx,
        task_handles: connection_tasks,
        transport: _transport,
    } = connection;
    let (outgoing_tx, mut outgoing_rx) =
        mpsc::channel::<RpcServerOutboundMessage>(CHANNEL_CAPACITY);
    let notifications = RpcNotificationSender::new(outgoing_tx.clone());
    let requests = notifications.request_sender();
    let handler = Arc::new(ExecServerHandler::new(
        session_registry,
        notifications,
        runtime_paths,
        http_client_factory,
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

    let mut dispatcher = RequestDispatcher::new(
        router,
        Arc::clone(&handler),
        outgoing_tx.clone(),
        disconnected_rx,
        requests.clone(),
        telemetry,
    );

    // Process inbound events sequentially to preserve initialize/initialized ordering.
    while let Some(event) = incoming_rx.recv().await {
        if !handler.is_session_attached() {
            debug!("exec-server connection evicted after session resume");
            break;
        }
        let result = match event {
            JsonRpcConnectionEvent::MalformedMessage { reason } => {
                dispatcher.handle_malformed_message(reason).await
            }
            JsonRpcConnectionEvent::Message(message) => match message {
                JSONRPCMessage::Request(request) => dispatcher.dispatch_request(request).await,
                JSONRPCMessage::Notification(notification) => {
                    dispatcher.handle_notification(notification).await
                }
                JSONRPCMessage::Response(response) => dispatcher.handle_response(response),
                JSONRPCMessage::Error(error) => dispatcher.handle_error(error),
            },
            JsonRpcConnectionEvent::Disconnected { reason } => {
                if let Some(reason) = reason {
                    debug!("exec-server connection disconnected: {reason}");
                }
                break;
            }
        };
        if result == RequestTaskResult::ConnectionClosed {
            break;
        }
    }

    requests.close();
    drop(dispatcher);
    handler.shutdown().await;
    drop(handler);
    drop(requests);
    drop(outgoing_tx);
    for task in connection_tasks {
        task.abort();
        let _ = task.await;
    }
    let _ = outbound_task.await;
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
            codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
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
            network_proxy: None,
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
