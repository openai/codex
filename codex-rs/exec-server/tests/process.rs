#![cfg(unix)]

mod common;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecResponse;
use codex_exec_server::InitializeParams;
use codex_exec_server::ProcessId;
use codex_sandboxing::SandboxLaunchConfig;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

async fn initialize_server(
    server: &mut common::exec_server::ExecServerHarness,
) -> anyhow::Result<()> {
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

    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_starts_process_over_websocket() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    initialize_server(&mut server).await?;

    let process_start_id = server
        .send_request(
            "process/start",
            serde_json::to_value(ExecParams {
                process_id: ProcessId::from("proc-1"),
                argv: vec!["true".to_string()],
                cwd: std::env::current_dir()?,
                env: Default::default(),
                tty: false,
                arg0: None,
                sandbox: SandboxLaunchConfig::no_sandbox(std::env::current_dir()?),
            })?,
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &process_start_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected process/start response");
    };
    assert_eq!(id, process_start_id);
    let process_start_response: ExecResponse = serde_json::from_value(result)?;
    assert_eq!(
        process_start_response,
        ExecResponse {
            process_id: ProcessId::from("proc-1"),
            sandbox: codex_sandboxing::SandboxType::None,
        }
    );

    server.shutdown().await?;
    Ok(())
}
