#![cfg(unix)]

mod common;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_exec_server::ExecResponse;
use codex_exec_server::InitializeParams;
use codex_exec_server::ProcessId;
use codex_sandboxing::SandboxType;
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

fn sandbox_wire_test_mode() -> (&'static str, SandboxType) {
    if cfg!(target_os = "macos") {
        ("require", SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        ("disabled", SandboxType::None)
    } else {
        unreachable!("unix exec-server tests only run on macOS and Linux");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_starts_process_over_websocket() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    initialize_server(&mut server).await?;

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
            sandbox_type: SandboxType::None,
        }
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_starts_sandboxed_process_over_websocket() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    initialize_server(&mut server).await?;

    let cwd = std::env::current_dir()?;
    let (sandbox_mode, expected_sandbox_type) = sandbox_wire_test_mode();
    let process_start_id = server
        .send_request(
            "process/start",
            serde_json::json!({
                "processId": "proc-sandbox",
                "argv": ["true"],
                "cwd": cwd,
                "env": {},
                "tty": false,
                "arg0": null,
                "sandbox": {
                    "mode": sandbox_mode,
                    "policy": {
                        "type": "danger-full-access"
                    },
                    "fileSystemPolicy": {
                        "kind": "unrestricted",
                        "entries": []
                    },
                    "networkPolicy": "enabled",
                    "sandboxPolicyCwd": cwd,
                    "enforceManagedNetwork": false,
                    "windowsSandboxLevel": "disabled",
                    "windowsSandboxPrivateDesktop": false,
                    "useLegacyLandlock": false
                }
            }),
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
            process_id: ProcessId::from("proc-sandbox"),
            sandbox_type: expected_sandbox_type,
        }
    );

    server.shutdown().await?;
    Ok(())
}
