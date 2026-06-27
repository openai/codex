use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server::in_process;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::EnvironmentAddParams;
use codex_app_server_protocol::EnvironmentAddResponse;
use codex_app_server_protocol::EnvironmentInfoParams;
use codex_app_server_protocol::EnvironmentInfoResponse;
use codex_app_server_protocol::EnvironmentShellInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_core::config::ConfigBuilder;
use codex_exec_server::EnvironmentManager;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::timeout;

use super::exec_server_test_support::accept_exec_server_environment;
use super::exec_server_test_support::accept_initialized_exec_server;
use super::exec_server_test_support::read_exec_server_json;

const RPC_TIMEOUT: Duration = Duration::from_secs(10);
const ENVIRONMENT_INFO_TIMEOUT: Duration = Duration::from_secs(30);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;
const INTERNAL_ERROR_CODE: i64 = -32603;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_info_returns_remote_environment_info() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let exec_server = tokio::spawn(async move {
        accept_exec_server_environment(
            listener,
            json!({
                "shell": {"name": "zsh", "path": "/bin/zsh"},
                "cwd": "file:///workspace",
            }),
        )
        .await?;
        Ok::<_, anyhow::Error>(())
    });

    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;
    add_environment(
        &mut app_server,
        &exec_server_url,
        /*connect_timeout_ms*/ None,
    )
    .await?;

    let request_id = app_server
        .send_raw_request(
            "environment/info",
            Some(json!({"environmentId": "remote-a"})),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<EnvironmentInfoResponse>(response)?,
        EnvironmentInfoResponse {
            shell: EnvironmentShellInfo {
                name: "zsh".to_string(),
                path: "/bin/zsh".to_string(),
            },
            cwd: Some(PathUri::parse("file:///workspace")?),
        }
    );
    timeout(RPC_TIMEOUT, exec_server).await???;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_info_accepts_missing_cwd() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let exec_server = tokio::spawn(async move {
        accept_exec_server_environment(
            listener,
            json!({"shell": {"name": "zsh", "path": "/bin/zsh"}}),
        )
        .await?;
        Ok::<_, anyhow::Error>(())
    });

    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;
    add_environment(
        &mut app_server,
        &exec_server_url,
        /*connect_timeout_ms*/ None,
    )
    .await?;

    let request_id = app_server
        .send_raw_request(
            "environment/info",
            Some(json!({"environmentId": "remote-a"})),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<EnvironmentInfoResponse>(response)?,
        EnvironmentInfoResponse {
            shell: EnvironmentShellInfo {
                name: "zsh".to_string(),
                path: "/bin/zsh".to_string(),
            },
            cwd: None,
        }
    );
    timeout(RPC_TIMEOUT, exec_server).await???;
    Ok(())
}

#[tokio::test]
async fn environment_info_rejects_unknown_environment() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;

    let request_id = app_server
        .send_raw_request(
            "environment/info",
            Some(json!({"environmentId": "missing"})),
        )
        .await?;
    let error = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error,
        JSONRPCError {
            id: RequestId::Integer(request_id),
            error: JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "unknown environment id `missing`".to_string(),
                data: None,
            },
        }
    );
    Ok(())
}

#[tokio::test]
async fn environment_info_reports_connection_failure() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;
    add_environment(&mut app_server, &exec_server_url, Some(50)).await?;

    let request_id = app_server
        .send_raw_request(
            "environment/info",
            Some(json!({"environmentId": "remote-a"})),
        )
        .await?;
    let error = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, INTERNAL_ERROR_CODE);
    assert!(
        error
            .error
            .message
            .contains("failed to get info for environment `remote-a`")
    );
    Ok(())
}

#[tokio::test]
async fn environment_info_timeout_releases_environment_serialization_queue() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let (request_received_tx, request_received_rx) = oneshot::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let exec_server = tokio::spawn(async move {
        let mut websocket = accept_initialized_exec_server(listener).await?;
        let request = read_exec_server_json(&mut websocket).await?;
        assert_eq!(request["method"], "environment/info");
        request_received_tx
            .send(())
            .map_err(|()| anyhow::anyhow!("environment info request receiver dropped"))?;
        shutdown_rx.await?;
        Ok::<_, anyhow::Error>(())
    });

    let codex_home = TempDir::new()?;
    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .loader_overrides(loader_overrides.clone())
        .build()
        .await?;
    let app_server = in_process::start(InProcessStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config: Arc::new(config),
        cli_overrides: Vec::new(),
        loader_overrides,
        strict_config: false,
        cloud_config_bundle: CloudConfigBundleLoader::default(),
        thread_config_loader: Arc::new(codex_config::NoopThreadConfigLoader),
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db: None,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        config_warnings: Vec::new(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        initialize: InitializeParams {
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
        channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?;

    let add_response = app_server
        .request(ClientRequest::EnvironmentAdd {
            request_id: RequestId::Integer(1),
            params: EnvironmentAddParams {
                environment_id: "remote-a".to_string(),
                exec_server_url,
                connect_timeout_ms: None,
            },
        })
        .await?
        .expect("environment/add should succeed");
    assert_eq!(
        serde_json::from_value::<EnvironmentAddResponse>(add_response)?,
        EnvironmentAddResponse {}
    );

    let info_sender = app_server.sender();
    let info_task = tokio::spawn(async move {
        info_sender
            .request(ClientRequest::EnvironmentInfo {
                request_id: RequestId::Integer(2),
                params: EnvironmentInfoParams {
                    environment_id: "remote-a".to_string(),
                },
            })
            .await
    });
    timeout(RPC_TIMEOUT, request_received_rx).await??;

    let replacement_sender = app_server.sender();
    let replacement_request = replacement_sender.request(ClientRequest::EnvironmentAdd {
        request_id: RequestId::Integer(3),
        params: EnvironmentAddParams {
            environment_id: "remote-a".to_string(),
            exec_server_url: "ws://127.0.0.1:1".to_string(),
            connect_timeout_ms: Some(50),
        },
    });
    tokio::pin!(replacement_request);
    tokio::select! {
        response = &mut replacement_request => {
            anyhow::bail!("environment/add completed before environment/info released the queue: {response:?}");
        }
        () = tokio::task::yield_now() => {}
    }

    tokio::time::pause();
    tokio::time::advance(ENVIRONMENT_INFO_TIMEOUT + Duration::from_millis(1)).await;
    tokio::time::resume();
    let error = timeout(RPC_TIMEOUT, info_task)
        .await???
        .expect_err("environment/info should time out");
    assert_eq!(error.code, INTERNAL_ERROR_CODE);
    assert!(
        error
            .message
            .contains("timed out waiting for exec-server `environment/info` response")
    );

    let replacement_response = timeout(RPC_TIMEOUT, &mut replacement_request)
        .await??
        .expect("environment/add should succeed after the timeout");
    assert_eq!(
        serde_json::from_value::<EnvironmentAddResponse>(replacement_response)?,
        EnvironmentAddResponse {}
    );

    shutdown_tx
        .send(())
        .map_err(|()| anyhow::anyhow!("exec-server shutdown receiver dropped"))?;
    timeout(RPC_TIMEOUT, exec_server).await???;
    app_server.shutdown().await?;
    Ok(())
}

async fn add_environment(
    app_server: &mut TestAppServer,
    exec_server_url: &str,
    connect_timeout_ms: Option<u64>,
) -> Result<()> {
    let request_id = app_server
        .send_raw_request(
            "environment/add",
            Some(json!({
                "environmentId": "remote-a",
                "execServerUrl": exec_server_url,
                "connectTimeoutMs": connect_timeout_ms,
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: EnvironmentAddResponse = to_response(response)?;
    Ok(())
}
