mod common;

use codex_exec_server::EnvironmentInfo;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCErrorError;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use common::exec_server::exec_server;
use common::exec_server::exec_server_with_env;
use pretty_assertions::assert_eq;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_accepts_initialize() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;

    let response = server.next_event().await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response");
    };
    assert_eq!(id, initialize_id);
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;
    Uuid::parse_str(&initialize_response.session_id)?;
    assert_eq!(
        initialize_response.environment_info,
        Some(EnvironmentInfo::local())
    );

    server.shutdown().await?;
    Ok(())
}

/// Requests retain their wire-order initialization errors even when later handshake messages are pipelined.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_rejects_pipelined_requests_before_initialized() -> anyhow::Result<()> {
    let mut server = exec_server_with_env(
        std::iter::empty::<(&str, &str)>(),
        &["--concurrent-requests", "32"],
    )
    .await?;
    let before_initialize_id = server
        .send_request("environment/info", serde_json::json!({}))
        .await?;
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;

    assert_eq!(
        server.next_event().await?,
        JSONRPCMessage::Error(JSONRPCError {
            id: before_initialize_id,
            error: JSONRPCErrorError {
                code: -32600,
                data: None,
                message: "client must call initialize before using environment info methods"
                    .to_string(),
            },
        })
    );
    let JSONRPCMessage::Response(JSONRPCResponse { id, .. }) = server.next_event().await? else {
        panic!("expected initialize response");
    };
    assert_eq!(id, initialize_id);

    let before_initialized_id = server
        .send_request("environment/info", serde_json::json!({}))
        .await?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    assert_eq!(
        server.next_event().await?,
        JSONRPCMessage::Error(JSONRPCError {
            id: before_initialized_id,
            error: JSONRPCErrorError {
                code: -32600,
                data: None,
                message: "client must send initialized before using environment info methods"
                    .to_string(),
            },
        })
    );

    server.shutdown().await?;
    Ok(())
}
