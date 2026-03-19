#![cfg(unix)]

mod common;

use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_exec_server::InitializeParams;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_stubs_process_start_over_websocket() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
            })?,
        )
        .await?;
    let _ = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &initialize_id
            )
        })
        .await?;

    let process_start_id = server
        .send_request(
            "process/start",
            serde_json::json!({
                "processId": "proc-1",
                "argv": ["true"],
                "cwd": std::env::current_dir()?,
                "env": {},
                "tty": false,
                "arg0": null
            }),
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Error(JSONRPCError { id, .. }) if id == &process_start_id
            )
        })
        .await?;
    let JSONRPCMessage::Error(JSONRPCError { id, error }) = response else {
        panic!("expected process/start stub error");
    };
    assert_eq!(id, process_start_id);
    assert_eq!(error.code, -32601);
    assert_eq!(
        error.message,
        "exec-server stub does not implement `process/start` yet"
    );

    server.shutdown().await?;
    Ok(())
}
