use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::DEFAULT_CLIENT_NAME;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RemoteControlClient;
use codex_app_server_protocol::RemoteControlClientsListOrder;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlDisableResponse;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_app_server_protocol::RemoteControlPairingStatusParams;
use codex_app_server_protocol::RemoteControlPairingStatusResponse;
use codex_app_server_protocol::RemoteControlStatusChangedNotification;
use codex_app_server_protocol::RemoteControlStatusReadResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_state::RemoteControlEnrollmentRecord;
use codex_state::StateRuntime;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_CONTROL_DISABLED_BY_REQUIREMENTS_MESSAGE: &str =
    "remote control is disabled by managed requirements";

async fn remote_control_preference(
    state_db: &StateRuntime,
    websocket_url: &str,
) -> Result<Option<bool>> {
    Ok(state_db
        .get_remote_control_enrollment(websocket_url, "account_id", Some(DEFAULT_CLIENT_NAME))
        .await?
        .context("enrollment should exist")?
        .remote_control_enabled)
}

async fn wait_for_response(mcp: &mut TestAppServer, request_id: i64) -> Result<JSONRPCResponse> {
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await?
}

async fn assert_remote_control_disabled_by_requirements(
    mcp: &mut TestAppServer,
    request_id: i64,
) -> Result<()> {
    let JSONRPCError { error, .. } = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.code, -32600);
    assert_eq!(
        error.message,
        REMOTE_CONTROL_DISABLED_BY_REQUIREMENTS_MESSAGE
    );
    Ok(())
}

#[tokio::test]
async fn managed_requirements_reject_all_remote_control_rpcs() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        "allow_remote_control = false\n",
    )?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("remoteControl/status/changed"),
    )
    .await??;
    let status: RemoteControlStatusChangedNotification = serde_json::from_value(
        notification
            .params
            .context("remote-control status notification should include params")?,
    )?;
    assert_eq!(status.status, RemoteControlConnectionStatus::Disabled);
    assert_eq!(status.environment_id, None);

    let request_ids = [
        mcp.send_remote_control_enable_request().await?,
        mcp.send_remote_control_disable_request().await?,
        mcp.send_remote_control_status_read_request().await?,
        mcp.send_remote_control_pairing_start_request(RemoteControlPairingStartParams {
            manual_code: false,
        })
        .await?,
        mcp.send_remote_control_pairing_status_request(RemoteControlPairingStatusParams {
            pairing_code: Some("pairing-code".to_string()),
            manual_pairing_code: None,
        })
        .await?,
        mcp.send_remote_control_clients_list_request(RemoteControlClientsListParams {
            environment_id: "environment-id".to_string(),
            cursor: None,
            limit: None,
            order: None,
        })
        .await?,
        mcp.send_remote_control_clients_revoke_request(RemoteControlClientsRevokeParams {
            environment_id: "environment-id".to_string(),
            client_id: "client-id".to_string(),
        })
        .await?,
    ];

    for request_id in request_ids {
        assert_remote_control_disabled_by_requirements(&mut mcp, request_id).await?;
    }

    Ok(())
}

#[tokio::test]
async fn managed_requirements_allow_remote_control_true_does_not_enable_or_block_it() -> Result<()>
{
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        "allow_remote_control = true\n",
    )?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_status_read_request().await?;
    let response = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlStatusReadResponse = to_response(response)?;
    assert_eq!(received.status, RemoteControlConnectionStatus::Disabled);
    Ok(())
}

#[tokio::test]
async fn listen_off_honors_persisted_remote_control_enable() -> Result<()> {
    let codex_home = TempDir::new()?;
    let listener = configured_remote_control_listener(codex_home.path()).await?;
    let websocket_url = format!(
        "ws://{}/backend-api/wham/remote/control/server",
        listener.local_addr()?
    );
    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string()).await?;
    state_db
        .upsert_remote_control_enrollment(&RemoteControlEnrollmentRecord {
            websocket_url,
            account_id: "account_id".to_string(),
            app_server_client_name: None,
            server_id: "server-id".to_string(),
            environment_id: "environment-id".to_string(),
            server_name: "server-name".to_string(),
            remote_control_enabled: Some(true),
        })
        .await?;

    let _app_server = TestAppServer::new_with_args(codex_home.path(), &["--listen", "off"]).await?;
    timeout(STARTUP_TIMEOUT, listener.accept()).await??;
    Ok(())
}

#[tokio::test]
async fn listen_off_ignores_persisted_enable_when_disabled_by_requirements() -> Result<()> {
    let codex_home = TempDir::new()?;
    let listener = configured_remote_control_listener(codex_home.path()).await?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        "allow_remote_control = false\n",
    )?;
    let websocket_url = format!(
        "ws://{}/backend-api/wham/remote/control/server",
        listener.local_addr()?
    );
    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string()).await?;
    state_db
        .upsert_remote_control_enrollment(&RemoteControlEnrollmentRecord {
            websocket_url: websocket_url.clone(),
            account_id: "account_id".to_string(),
            app_server_client_name: None,
            server_id: "server-id".to_string(),
            environment_id: "environment-id".to_string(),
            server_name: "server-name".to_string(),
            remote_control_enabled: Some(true),
        })
        .await?;

    let mut app_server =
        TestAppServer::new_with_args(codex_home.path(), &["--listen", "off"]).await?;
    let status = timeout(STARTUP_TIMEOUT, app_server.wait_for_exit()).await??;
    assert!(!status.success());
    timeout(Duration::from_millis(100), listener.accept())
        .await
        .expect_err("managed requirements should prevent a remote-control connection");
    assert_eq!(
        state_db
            .get_remote_control_enrollment(
                &websocket_url,
                "account_id",
                /*app_server_client_name*/ None
            )
            .await?
            .context("enrollment should remain persisted")?
            .remote_control_enabled,
        Some(true)
    );
    Ok(())
}

#[tokio::test]
async fn listen_off_exits_without_persisted_remote_control_enable() -> Result<()> {
    for persisted_preference in [None, Some(false)] {
        let codex_home = TempDir::new()?;
        let listener = configured_remote_control_listener(codex_home.path()).await?;
        if let Some(remote_control_enabled) = persisted_preference {
            let websocket_url = format!(
                "ws://{}/backend-api/wham/remote/control/server",
                listener.local_addr()?
            );
            let state_db =
                StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string())
                    .await?;
            state_db
                .upsert_remote_control_enrollment(&RemoteControlEnrollmentRecord {
                    websocket_url,
                    account_id: "account_id".to_string(),
                    app_server_client_name: None,
                    server_id: "server-id".to_string(),
                    environment_id: "environment-id".to_string(),
                    server_name: "server-name".to_string(),
                    remote_control_enabled: Some(remote_control_enabled),
                })
                .await?;
        }

        let mut app_server =
            TestAppServer::new_with_args(codex_home.path(), &["--listen", "off"]).await?;
        let status = timeout(STARTUP_TIMEOUT, app_server.wait_for_exit()).await??;
        assert!(!status.success());
    }
    Ok(())
}

#[tokio::test]
async fn remote_control_disable_returns_disabled_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    let _listener = configured_remote_control_listener(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_disable_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlDisableResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Disabled);
    assert!(!received.server_name.is_empty());
    assert_eq!(received.environment_id, None);
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_control_status_read_returns_disabled_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_status_read_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlStatusReadResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Disabled);
    assert!(!received.server_name.is_empty());
    assert_eq!(received.environment_id, None);
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_control_enable_returns_connecting_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = BlockingRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_enable_request().await?;
    assert_eq!(
        timeout(DEFAULT_TIMEOUT, backend.wait_for_enroll_request()).await??,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    timeout(
        Duration::from_millis(100),
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect_err("enable response should wait for enrollment");
    backend.complete_enrollment()?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlEnableResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Connecting);
    assert!(!received.server_name.is_empty());
    assert_eq!(received.environment_id.as_deref(), Some("environment-id"));
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn disable_waits_for_in_flight_durable_enable() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = BlockingRemoteControlBackend::start(codex_home.path()).await?;
    let websocket_url = backend.websocket_url().to_string();
    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    mcp.send_remote_control_enable_request().await?;
    timeout(DEFAULT_TIMEOUT, backend.wait_for_enroll_request()).await??;
    let disable_request_id = mcp.send_remote_control_disable_request().await?;
    timeout(
        Duration::from_millis(100),
        mcp.read_stream_until_response_message(RequestId::Integer(disable_request_id)),
    )
    .await
    .expect_err("disable response should wait for the in-flight enable");

    backend.complete_enrollment()?;
    let response = wait_for_response(&mut mcp, disable_request_id).await?;
    let received: RemoteControlDisableResponse = to_response(response)?;
    assert_eq!(received.status, RemoteControlConnectionStatus::Disabled);
    assert_eq!(
        remote_control_preference(&state_db, &websocket_url).await?,
        Some(false)
    );
    Ok(())
}

#[tokio::test]
async fn rpc_updates_durable_preference_but_ephemeral_does_not() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = BlockingRemoteControlBackend::start(codex_home.path()).await?;
    let websocket_url = backend.websocket_url().to_string();
    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string()).await?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_enable_request().await?;
    assert_eq!(
        timeout(DEFAULT_TIMEOUT, backend.wait_for_enroll_request()).await??,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    backend.complete_enrollment()?;
    wait_for_response(&mut mcp, request_id).await?;
    assert_eq!(
        remote_control_preference(&state_db, &websocket_url).await?,
        Some(true)
    );

    let request_id = mcp.send_remote_control_ephemeral_disable_request().await?;
    wait_for_response(&mut mcp, request_id).await?;
    assert_eq!(
        remote_control_preference(&state_db, &websocket_url).await?,
        Some(true)
    );

    let request_id = mcp.send_remote_control_disable_request().await?;
    wait_for_response(&mut mcp, request_id).await?;
    assert_eq!(
        remote_control_preference(&state_db, &websocket_url).await?,
        Some(false)
    );

    let request_id = mcp.send_remote_control_enable_request().await?;
    wait_for_response(&mut mcp, request_id).await?;
    assert_eq!(
        remote_control_preference(&state_db, &websocket_url).await?,
        Some(true)
    );

    let request_id = mcp.send_remote_control_disable_request().await?;
    wait_for_response(&mut mcp, request_id).await?;
    assert_eq!(
        remote_control_preference(&state_db, &websocket_url).await?,
        Some(false)
    );

    let request_id = mcp.send_remote_control_ephemeral_enable_request().await?;
    wait_for_response(&mut mcp, request_id).await?;
    assert_eq!(
        remote_control_preference(&state_db, &websocket_url).await?,
        Some(false)
    );

    Ok(())
}

#[tokio::test]
async fn remote_control_status_read_returns_connecting_status_after_enable() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = BlockingRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_enable_request().await?;
    let enroll_request = timeout(DEFAULT_TIMEOUT, backend.wait_for_enroll_request()).await??;
    assert_eq!(
        enroll_request,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    backend.complete_enrollment()?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let request_id = mcp.send_remote_control_status_read_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlStatusReadResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Connecting);
    assert!(!received.server_name.is_empty());
    assert_eq!(received.environment_id.as_deref(), Some("environment-id"));
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_control_pairing_start_returns_pairing_artifacts() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = PairingRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_enable_request().await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        timeout(DEFAULT_TIMEOUT, backend.wait_for_enroll_request()).await??,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_matching_notification(
            "remoteControl/status/changed enrolled",
            |notification| {
                notification.method == "remoteControl/status/changed"
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("environmentId"))
                        .and_then(serde_json::Value::as_str)
                        == Some("environment-id")
            },
        ),
    )
    .await??;

    let request_id = mcp
        .send_remote_control_pairing_start_request(RemoteControlPairingStartParams {
            manual_code: true,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(response.result.get("serverId"), None);
    let received: RemoteControlPairingStartResponse = to_response(response)?;

    assert_eq!(
        received,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "environment-id".to_string(),
            expires_at: 33_336_362_096,
        }
    );

    let request_id = mcp
        .send_remote_control_pairing_status_request(RemoteControlPairingStatusParams {
            pairing_code: Some("pairing-code".to_string()),
            manual_pairing_code: None,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(response.result.get("serverId"), None);
    let received: RemoteControlPairingStatusResponse = to_response(response)?;

    assert_eq!(
        received,
        RemoteControlPairingStatusResponse { claimed: true }
    );

    let request_id = mcp
        .send_remote_control_pairing_status_request(RemoteControlPairingStatusParams {
            pairing_code: None,
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(response.result.get("serverId"), None);
    let received: RemoteControlPairingStatusResponse = to_response(response)?;

    assert_eq!(
        received,
        RemoteControlPairingStatusResponse { claimed: true }
    );
    Ok(())
}

#[tokio::test]
async fn pairing_start_works_after_ephemeral_enable() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = PairingRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let request_id = mcp.send_remote_control_ephemeral_enable_request().await?;
    wait_for_response(&mut mcp, request_id).await?;

    let request_id = mcp
        .send_remote_control_pairing_start_request(RemoteControlPairingStartParams {
            manual_code: true,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        timeout(DEFAULT_TIMEOUT, backend.wait_for_enroll_request()).await??,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    assert_eq!(response.result.get("serverId"), None);
    let received: RemoteControlPairingStartResponse = to_response(response)?;

    assert_eq!(
        received,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "environment-id".to_string(),
            expires_at: 33_336_362_096,
        }
    );
    Ok(())
}

#[tokio::test]
async fn remote_control_client_management_works_while_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = ClientManagementRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_remote_control_clients_list_request(RemoteControlClientsListParams {
            environment_id: "environment-id".to_string(),
            cursor: Some("cursor-id".to_string()),
            limit: Some(10),
            order: Some(RemoteControlClientsListOrder::Desc),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlClientsListResponse = to_response(response)?;
    assert_eq!(
        received,
        RemoteControlClientsListResponse {
            data: vec![RemoteControlClient {
                client_id: "client-id".to_string(),
                display_name: Some("Anton Phone".to_string()),
                device_type: Some("phone".to_string()),
                platform: Some("ios".to_string()),
                os_version: Some("19.0".to_string()),
                device_model: Some("iPhone".to_string()),
                app_version: Some("1.2.3".to_string()),
                last_seen_at: Some(1_772_694_000),
            }],
            next_cursor: Some("next-cursor".to_string()),
        }
    );

    let request_id = mcp
        .send_remote_control_clients_revoke_request(RemoteControlClientsRevokeParams {
            environment_id: "environment-id".to_string(),
            client_id: "client-id".to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlClientsRevokeResponse = to_response(response)?;
    assert_eq!(received, RemoteControlClientsRevokeResponse {});
    assert_eq!(
        timeout(DEFAULT_TIMEOUT, backend.wait_for_requests()).await??,
        vec![
            "GET /backend-api/wham/remote/control/environments/environment-id/clients?cursor=cursor-id&limit=10&order=desc HTTP/1.1".to_string(),
            "DELETE /backend-api/wham/remote/control/environments/environment-id/clients/client-id HTTP/1.1".to_string(),
        ]
    );
    Ok(())
}

struct BlockingRemoteControlBackend {
    enroll_request_rx: Option<oneshot::Receiver<Result<String>>>,
    enroll_response_tx: Option<oneshot::Sender<()>>,
    websocket_url: String,
    server_task: JoinHandle<()>,
}

struct ClientManagementRemoteControlBackend {
    requests_rx: Option<oneshot::Receiver<Result<Vec<String>>>>,
    server_task: JoinHandle<()>,
}

impl ClientManagementRemoteControlBackend {
    async fn start(codex_home: &std::path::Path) -> Result<Self> {
        let listener = configured_remote_control_listener(codex_home).await?;
        let (requests_tx, requests_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let result = async {
                let list_request = read_http_request(&listener).await?;
                let list_request_line = list_request.request_line;
                respond_with_json(
                    list_request.reader.into_inner(),
                    serde_json::json!({
                        "items": [{
                            "client_id": "client-id",
                            "account_user_id": "user-id",
                            "enrollment_status": "enrolled_device_key",
                            "display_name": "Anton Phone",
                            "device_type": "phone",
                            "platform": "ios",
                            "os_version": "19.0",
                            "device_model": "iPhone",
                            "app_version": "1.2.3",
                            "last_seen_at": "2026-03-05T07:00:00Z",
                            "last_seen_city": "San Francisco",
                        }],
                        "cursor": "next-cursor",
                    }),
                )
                .await?;

                let revoke_request = read_http_request(&listener).await?;
                let revoke_request_line = revoke_request.request_line;
                respond_with_status(revoke_request.reader.into_inner(), "204 No Content", "")
                    .await?;

                Ok(vec![list_request_line, revoke_request_line])
            }
            .await;
            let _ = requests_tx.send(result);
        });
        Ok(Self {
            requests_rx: Some(requests_rx),
            server_task,
        })
    }

    async fn wait_for_requests(&mut self) -> Result<Vec<String>> {
        self.requests_rx
            .take()
            .context("requests should only be awaited once")?
            .await?
    }
}

impl BlockingRemoteControlBackend {
    async fn start(codex_home: &std::path::Path) -> Result<Self> {
        let listener = configured_remote_control_listener(codex_home).await?;
        let websocket_url = format!(
            "ws://{}/backend-api/wham/remote/control/server",
            listener.local_addr()?
        );

        let (enroll_request_tx, enroll_request_rx) = oneshot::channel();
        let (enroll_response_tx, enroll_response_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            match read_enroll_request(&listener).await {
                Ok((request_line, reader)) => {
                    let _ = enroll_request_tx.send(Ok(request_line));
                    if enroll_response_rx.await.is_err() {
                        return;
                    }
                    if respond_with_json(
                        reader.into_inner(),
                        serde_json::json!({
                            "server_id": "server-id",
                            "environment_id": "environment-id",
                            "remote_control_token": "remote-control-token",
                            "expires_at": "3026-05-22T12:34:56Z",
                        }),
                    )
                    .await
                    .is_err()
                    {
                        return;
                    }
                    let Ok(_websocket) = listener.accept().await else {
                        return;
                    };
                    std::future::pending::<()>().await;
                }
                Err(err) => {
                    let _ = enroll_request_tx.send(Err(err));
                }
            }
        });

        Ok(Self {
            enroll_request_rx: Some(enroll_request_rx),
            enroll_response_tx: Some(enroll_response_tx),
            websocket_url,
            server_task,
        })
    }

    async fn wait_for_enroll_request(&mut self) -> Result<String> {
        let rx = self
            .enroll_request_rx
            .take()
            .context("enroll request should only be awaited once")?;
        rx.await?
    }

    fn complete_enrollment(&mut self) -> Result<()> {
        self.enroll_response_tx
            .take()
            .context("enrollment should only complete once")?
            .send(())
            .map_err(|()| anyhow::anyhow!("enrollment response receiver dropped"))
    }

    fn websocket_url(&self) -> &str {
        &self.websocket_url
    }
}

struct PairingRemoteControlBackend {
    enroll_request_rx: Option<oneshot::Receiver<Result<String>>>,
    server_task: JoinHandle<()>,
}

impl PairingRemoteControlBackend {
    async fn start(codex_home: &std::path::Path) -> Result<Self> {
        let listener = configured_remote_control_listener(codex_home).await?;
        let (enroll_request_tx, enroll_request_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let mut enroll_request_tx = Some(enroll_request_tx);
            let result = async {
                let enroll_request = read_http_request(&listener).await?;
                if let Some(enroll_request_tx) = enroll_request_tx.take() {
                    let _ = enroll_request_tx.send(Ok(enroll_request.request_line.clone()));
                }
                respond_with_json(
                    enroll_request.reader.into_inner(),
                    serde_json::json!({
                        "server_id": "server-id",
                        "environment_id": "environment-id",
                        "remote_control_token": "remote-control-token",
                        "expires_at": "3026-05-22T12:34:56Z",
                    }),
                )
                .await?;

                let request_after_enroll = read_http_request(&listener).await?;
                let pair_http_request = if request_after_enroll.request_line.starts_with("GET ") {
                    read_http_request(&listener).await?
                } else {
                    request_after_enroll
                };
                respond_with_json(
                    pair_http_request.reader.into_inner(),
                    serde_json::json!({
                        "pairing_code": "pairing-code",
                        "manual_pairing_code": "ABCD-EFGH",
                        "server_id": "server-id",
                        "environment_id": "environment-id",
                        "expires_at": "3026-05-22T12:34:56Z",
                    }),
                )
                .await?;
                for expected_body in [
                    serde_json::json!({ "pairing_code": "pairing-code" }),
                    serde_json::json!({ "manual_pairing_code": "ABCD-EFGH" }),
                ] {
                    let status_http_request = read_http_request(&listener).await?;
                    assert_eq!(
                        status_http_request.request_line,
                        "POST /backend-api/wham/remote/control/server/pair/status HTTP/1.1"
                    );
                    assert_eq!(
                        serde_json::from_str::<serde_json::Value>(&status_http_request.body)?,
                        expected_body
                    );
                    respond_with_json(
                        status_http_request.reader.into_inner(),
                        serde_json::json!({ "claimed": true }),
                    )
                    .await?;
                }
                std::future::pending::<()>().await;
                Ok::<(), anyhow::Error>(())
            }
            .await;

            if let Err(err) = result {
                let err = err.to_string();
                if let Some(enroll_request_tx) = enroll_request_tx {
                    let _ = enroll_request_tx.send(Err(anyhow::anyhow!(err)));
                }
            }
        });

        Ok(Self {
            enroll_request_rx: Some(enroll_request_rx),
            server_task,
        })
    }

    async fn wait_for_enroll_request(&mut self) -> Result<String> {
        self.enroll_request_rx
            .take()
            .context("enroll request should only be awaited once")?
            .await?
    }
}

impl Drop for PairingRemoteControlBackend {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

impl Drop for BlockingRemoteControlBackend {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

impl Drop for ClientManagementRemoteControlBackend {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

struct HttpRequest {
    request_line: String,
    body: String,
    reader: BufReader<TcpStream>,
}

async fn configured_remote_control_listener(codex_home: &std::path::Path) -> Result<TcpListener> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let remote_control_url = format!("http://{}/backend-api/", listener.local_addr()?);
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home,
        &remote_control_url,
        &remote_control_url,
    )?;
    write_chatgpt_auth(
        codex_home,
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account_id")
            .chatgpt_account_id("account_id"),
        AuthCredentialsStoreMode::File,
    )?;
    Ok(listener)
}

async fn read_enroll_request(listener: &TcpListener) -> Result<(String, BufReader<TcpStream>)> {
    let request = read_http_request(listener).await?;
    Ok((request.request_line, request.reader))
}

async fn read_http_request(listener: &TcpListener) -> Result<HttpRequest> {
    let (stream, _) = listener.accept().await?;
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line
            .trim_end()
            .strip_prefix("content-length:")
            .or_else(|| line.trim_end().strip_prefix("Content-Length:"))
        {
            content_length = value.trim().parse::<usize>()?;
        }
    }
    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).await?;
    }

    Ok(HttpRequest {
        request_line: request_line.trim_end().to_string(),
        body: String::from_utf8(body)?,
        reader,
    })
}

async fn respond_with_json(stream: TcpStream, body: serde_json::Value) -> Result<()> {
    let body = body.to_string();
    let mut stream = stream;
    stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await?;
    Ok(())
}

async fn respond_with_status(mut stream: TcpStream, status: &str, body: &str) -> Result<()> {
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await?;
    Ok(())
}
