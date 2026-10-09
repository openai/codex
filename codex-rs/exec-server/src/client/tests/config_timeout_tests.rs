//! Config-read deadlines must preserve non-reconnectable stdio connections.

use super::read_jsonrpc_line;
use super::write_jsonrpc_line;
use crate::ExecServerClient;
use crate::ExecServerClientConnectOptions;
use crate::ExecServerError;
use crate::connection::JsonRpcConnection;
use crate::protocol::ENVIRONMENT_CONFIG_READ_METHOD;
use crate::protocol::EnvironmentConfigLayerStack;
use crate::protocol::EnvironmentConfigReadParams;
use crate::protocol::EnvironmentConfigReadResponse;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeResponse;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::io::duplex;
use tokio::time::Duration;

#[tokio::test(start_paused = true)]
async fn config_read_timeout_keeps_stdio_connection_usable() {
    let (client_stdin, server_reader) = duplex(4096);
    let (mut server_writer, client_stdout) = duplex(4096);
    let mut lines = BufReader::new(server_reader).lines();
    let connect = ExecServerClient::connect(
        JsonRpcConnection::from_stdio(client_stdout, client_stdin, "config-timeout".to_string()),
        ExecServerClientConnectOptions::default(),
    );
    tokio::pin!(connect);
    assert!(futures::poll!(connect.as_mut()).is_pending());
    let initialize = match read_jsonrpc_line(&mut lines).await {
        JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
        other => panic!("expected initialize request, got {other:?}"),
    };
    write_jsonrpc_line(
        &mut server_writer,
        JSONRPCMessage::Response(JSONRPCResponse {
            id: initialize.id,
            result: serde_json::to_value(InitializeResponse {
                session_id: "config-timeout".to_string(),
                environment_info: None,
            })
            .expect("initialize response should serialize"),
        }),
    )
    .await;
    let client = connect.await.expect("stdio client should connect");
    assert!(matches!(
        read_jsonrpc_line(&mut lines).await,
        JSONRPCMessage::Notification(notification) if notification.method == INITIALIZED_METHOD
    ));

    let cwd = PathUri::from_host_native_path(std::env::current_dir().expect("current directory"))
        .expect("current directory URI");
    let params = EnvironmentConfigReadParams {
        cwd: cwd.clone(),
        config_paths: vec![vec!["features".to_string()]],
        requirements_paths: Vec::new(),
    };
    let mut expected = EnvironmentConfigReadResponse {
        user_home_dir: None,
        codex_home_dir: cwd,
        hostname: Some("original".to_string()),
        config: EnvironmentConfigLayerStack {
            layers: Vec::new(),
            cloud_insertion_index: 0,
        },
        requirements: EnvironmentConfigLayerStack {
            layers: Vec::new(),
            cloud_insertion_index: 0,
        },
    };

    // Model inline dispatch by holding valid work while the timed read queues behind it.
    let original = client.read_environment_config(params.clone());
    tokio::pin!(original);
    assert!(futures::poll!(original.as_mut()).is_pending());
    let original_request = match read_jsonrpc_line(&mut lines).await {
        JSONRPCMessage::Request(request) if request.method == ENVIRONMENT_CONFIG_READ_METHOD => {
            request
        }
        other => panic!("expected original config read, got {other:?}"),
    };
    let rpc_timeout = Duration::from_secs(1);
    let timed = client.read_environment_config_with_timeout(params.clone(), rpc_timeout);
    tokio::pin!(timed);
    assert!(futures::poll!(timed.as_mut()).is_pending());
    tokio::time::advance(rpc_timeout).await;
    assert!(matches!(
        timed.await,
        Err(ExecServerError::RpcTimedOut { method, timeout })
            if method == ENVIRONMENT_CONFIG_READ_METHOD && timeout == rpc_timeout
    ));
    assert!(
        futures::poll!(original.as_mut()).is_pending(),
        "a queued config-read timeout must not disconnect valid in-flight work"
    );

    write_jsonrpc_line(
        &mut server_writer,
        JSONRPCMessage::Response(JSONRPCResponse {
            id: original_request.id,
            result: serde_json::to_value(&expected).expect("config response should serialize"),
        }),
    )
    .await;
    assert_eq!(
        original.await.expect("original read should finish"),
        expected
    );

    // Finish the expired request, then serve a fresh read over the same connection.
    let retry = client.read_environment_config_with_timeout(params, rpc_timeout);
    tokio::pin!(retry);
    assert!(futures::poll!(retry.as_mut()).is_pending());
    for hostname in ["late", "fresh"] {
        let request = match read_jsonrpc_line(&mut lines).await {
            JSONRPCMessage::Request(request)
                if request.method == ENVIRONMENT_CONFIG_READ_METHOD =>
            {
                request
            }
            other => panic!("expected queued config read, got {other:?}"),
        };
        expected.hostname = Some(hostname.to_string());
        write_jsonrpc_line(
            &mut server_writer,
            JSONRPCMessage::Response(JSONRPCResponse {
                id: request.id,
                result: serde_json::to_value(&expected).expect("config response should serialize"),
            }),
        )
        .await;
    }
    assert_eq!(
        retry.await.expect("stdio connection should be reusable"),
        expected
    );
}
