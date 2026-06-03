use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::HttpStateClearResponse;
use codex_app_server_protocol::HttpStateGetResponse;
use codex_app_server_protocol::HttpStateSetParams;
use codex_app_server_protocol::HttpStateSetResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const STATE_N: &str = "state-n";
const STATE_N_PLUS_ONE: &str = "state-n-plus-one";

#[tokio::test]
async fn http_state_bridge_reads_writes_rotates_and_clears_surface_state() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    let initialized = mcp
        .initialize_with_capabilities(client_info("codex_desktop_ssh"), /*capabilities*/ None)
        .await?;
    let JSONRPCMessage::Response(_) = initialized else {
        anyhow::bail!("expected initialize response, got {initialized:?}");
    };

    assert_eq!(
        get_state(&mut mcp).await?,
        HttpStateGetResponse { state: None }
    );
    assert_eq!(
        set_state(
            &mut mcp,
            HttpStateSetParams {
                state: STATE_N.to_string(),
                expected_state: None,
            },
        )
        .await?,
        HttpStateSetResponse { written: true }
    );
    assert_eq!(
        get_state(&mut mcp).await?,
        HttpStateGetResponse {
            state: Some(STATE_N.to_string()),
        }
    );
    assert_eq!(
        set_state(
            &mut mcp,
            HttpStateSetParams {
                state: STATE_N_PLUS_ONE.to_string(),
                expected_state: Some("stale-state".to_string()),
            },
        )
        .await?,
        HttpStateSetResponse { written: false }
    );
    assert_eq!(
        get_state(&mut mcp).await?,
        HttpStateGetResponse {
            state: Some(STATE_N.to_string()),
        }
    );
    assert_eq!(
        set_state(
            &mut mcp,
            HttpStateSetParams {
                state: STATE_N_PLUS_ONE.to_string(),
                expected_state: Some(STATE_N.to_string()),
            },
        )
        .await?,
        HttpStateSetResponse { written: true }
    );
    assert_eq!(
        get_state(&mut mcp).await?,
        HttpStateGetResponse {
            state: Some(STATE_N_PLUS_ONE.to_string()),
        }
    );

    let request_id = mcp
        .send_raw_request("httpState/clear", /*params*/ None)
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<HttpStateClearResponse>(response)?,
        HttpStateClearResponse {}
    );
    assert_eq!(
        get_state(&mut mcp).await?,
        HttpStateGetResponse { state: None }
    );
    assert!(
        !codex_home
            .path()
            .join("state/codex_desktop_ssh.json")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn http_state_bridge_rejects_unknown_client_surface() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    let initialized = mcp
        .initialize_with_client_info(client_info("third_party_client"))
        .await?;
    let JSONRPCMessage::Response(_) = initialized else {
        anyhow::bail!("expected initialize response, got {initialized:?}");
    };

    let request_id = mcp
        .send_raw_request("httpState/get", /*params*/ None)
        .await?;
    let error = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "HTTP state is unavailable for app-server client \"third_party_client\""
    );
    assert_eq!(error.error.data, None);

    Ok(())
}

async fn get_state(mcp: &mut TestAppServer) -> Result<HttpStateGetResponse> {
    let request_id = mcp
        .send_raw_request("httpState/get", /*params*/ None)
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn set_state(
    mcp: &mut TestAppServer,
    params: HttpStateSetParams,
) -> Result<HttpStateSetResponse> {
    let request_id = mcp
        .send_raw_request("httpState/set", Some(serde_json::to_value(params)?))
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn client_info(name: &str) -> ClientInfo {
    ClientInfo {
        name: name.to_string(),
        title: None,
        version: "1.0.0".to_string(),
    }
}
