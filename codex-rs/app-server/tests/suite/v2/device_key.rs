use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::read_error_for_id;
use super::connection_handling_websocket::read_response_for_id;
use super::connection_handling_websocket::send_initialize_request;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

async fn initialized_mcp(codex_home: &TempDir) -> Result<McpProcess> {
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    Ok(mcp)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn device_key_create_rejects_empty_account_user_id() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let request_id = mcp
        .send_raw_request(
            "device/key/create",
            Some(json!({
                "accountUserId": "",
                "clientId": "cli_123",
            })),
        )
        .await?;
    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "invalid device key payload: accountUserId must not be empty"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn device_key_methods_are_rejected_over_websocket() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;
    let mut ws = connect_websocket(bind_addr).await?;
    send_initialize_request(&mut ws, /*id*/ 1, "device_key_ws_test").await?;
    let initialize_response = read_response_for_id(&mut ws, /*id*/ 1).await?;
    assert_eq!(initialize_response.id, RequestId::Integer(1));

    let cases = [
        (
            "device/key/create",
            json!({
                "accountUserId": "acct_123",
                "clientId": "cli_123",
            }),
        ),
        (
            "device/key/public",
            json!({
                "keyId": "device-key-123",
            }),
        ),
        (
            "device/key/sign",
            json!({
                "keyId": "device-key-123",
                "payload": {
                    "type": "remoteControlClientConnection",
                    "nonce": "nonce-123",
                    "audience": "remote_control_client_websocket",
                    "sessionId": "wssess_123",
                    "targetOrigin": "https://chatgpt.com",
                    "targetPath": "/api/codex/remote/control/client",
                    "accountUserId": "acct_123",
                    "clientId": "cli_123",
                    "tokenExpiresAt": 4_102_444_800i64,
                    "tokenSha256Base64url": "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU",
                    "scopes": ["remote_control_controller_websocket"],
                },
            }),
        ),
    ];

    for (index, (method, params)) in cases.into_iter().enumerate() {
        let id = 2 + index as i64;
        send_request(&mut ws, method, id, Some(params)).await?;
        let error = read_error_for_id(&mut ws, id).await?;

        assert_eq!(error.error.code, -32600);
        assert_eq!(
            error.error.message,
            format!("{method} is not available over remote transports")
        );
    }

    process.kill().await?;
    Ok(())
}
