use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NativeIntegrityStateClearResponse;
use codex_app_server_protocol::NativeIntegrityStateReadResponse;
use codex_app_server_protocol::NativeIntegrityStateWriteParams;
use codex_app_server_protocol::NativeIntegrityStateWriteResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const STATE_N: &str = "ois1.a.b.c";
const STATE_N_PLUS_ONE: &str = "ois1.d.e.f";

#[tokio::test]
async fn native_integrity_state_bridge_reads_writes_rotates_and_clears_surface_state() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    let initialized = mcp
        .initialize_with_client_info(client_info("codex_desktop_ssh"))
        .await?;
    let JSONRPCMessage::Response(_) = initialized else {
        anyhow::bail!("expected initialize response, got {initialized:?}");
    };

    assert_eq!(
        read_state(&mut mcp).await?,
        NativeIntegrityStateReadResponse { state: None }
    );
    assert_eq!(
        write_state(
            &mut mcp,
            NativeIntegrityStateWriteParams {
                state: STATE_N.to_string(),
                expected_state: None,
            },
        )
        .await?,
        NativeIntegrityStateWriteResponse { written: true }
    );
    assert_eq!(
        read_state(&mut mcp).await?,
        NativeIntegrityStateReadResponse {
            state: Some(STATE_N.to_string()),
        }
    );
    assert_eq!(
        write_state(
            &mut mcp,
            NativeIntegrityStateWriteParams {
                state: STATE_N_PLUS_ONE.to_string(),
                expected_state: Some("stale-state".to_string()),
            },
        )
        .await?,
        NativeIntegrityStateWriteResponse { written: false }
    );
    assert_eq!(
        read_state(&mut mcp).await?,
        NativeIntegrityStateReadResponse {
            state: Some(STATE_N.to_string()),
        }
    );
    assert_eq!(
        write_state(
            &mut mcp,
            NativeIntegrityStateWriteParams {
                state: STATE_N_PLUS_ONE.to_string(),
                expected_state: Some(STATE_N.to_string()),
            },
        )
        .await?,
        NativeIntegrityStateWriteResponse { written: true }
    );
    assert_eq!(
        read_state(&mut mcp).await?,
        NativeIntegrityStateReadResponse {
            state: Some(STATE_N_PLUS_ONE.to_string()),
        }
    );

    let request_id = mcp
        .send_raw_request("nativeIntegrityState/clear", /*params*/ None)
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<NativeIntegrityStateClearResponse>(response)?,
        NativeIntegrityStateClearResponse {}
    );
    assert_eq!(
        read_state(&mut mcp).await?,
        NativeIntegrityStateReadResponse { state: None }
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
async fn native_integrity_state_bridge_rejects_unknown_client_surface() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    let initialized = mcp
        .initialize_with_client_info(client_info("third_party_client"))
        .await?;
    let JSONRPCMessage::Response(_) = initialized else {
        anyhow::bail!("expected initialize response, got {initialized:?}");
    };

    let request_id = mcp
        .send_raw_request("nativeIntegrityState/read", /*params*/ None)
        .await?;
    let error = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "native integrity state is unavailable for app-server client \"third_party_client\""
    );
    assert_eq!(error.error.data, None);

    Ok(())
}

async fn read_state(mcp: &mut TestAppServer) -> Result<NativeIntegrityStateReadResponse> {
    let request_id = mcp
        .send_raw_request("nativeIntegrityState/read", /*params*/ None)
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn write_state(
    mcp: &mut TestAppServer,
    params: NativeIntegrityStateWriteParams,
) -> Result<NativeIntegrityStateWriteResponse> {
    let request_id = mcp
        .send_raw_request(
            "nativeIntegrityState/write",
            Some(serde_json::to_value(params)?),
        )
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
