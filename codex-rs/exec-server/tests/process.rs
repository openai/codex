#![cfg(unix)]

mod common;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_exec_server::ExecResponse;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadResponse;
use codex_exec_server::TerminateResponse;
use codex_exec_server::WriteResponse;
use codex_exec_server::WriteStatus;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_starts_process_over_websocket() -> anyhow::Result<()> {
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

    let process_start_id = server
        .send_request(
            "process/start",
            serde_json::json!({
                "processId": "proc-1",
                "argv": ["true"],
                "cwd": std::env::current_dir()?,
                "env": {},
                "tty": false,
                "pipeStdin": false,
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
            process_id: ProcessId::from("proc-1")
        }
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_defaults_omitted_pipe_stdin_to_closed_stdin() -> anyhow::Result<()> {
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

    let process_start_id = server
        .send_request(
            "process/start",
            serde_json::json!({
                "processId": "proc-default-stdin",
                "argv": [
                    "/bin/sh",
                    "-c",
                    "sleep 0.3; if IFS= read -r line; then printf 'read:%s\\n' \"$line\"; else printf 'eof\\n'; fi"
                ],
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
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        panic!("expected process/start response");
    };
    let process_start_response: ExecResponse = serde_json::from_value(result)?;
    assert_eq!(
        process_start_response,
        ExecResponse {
            process_id: ProcessId::from("proc-default-stdin")
        }
    );

    let write_id = server
        .send_request(
            "process/write",
            serde_json::json!({
                "processId": "proc-default-stdin",
                "chunk": "aWdub3JlZAo="
            }),
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &write_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        panic!("expected process/write response");
    };
    let write_response: WriteResponse = serde_json::from_value(result)?;
    assert_eq!(
        write_response,
        WriteResponse {
            status: WriteStatus::StdinClosed
        }
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_resumes_detached_session_without_killing_processes() -> anyhow::Result<()> {
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
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &initialize_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        panic!("expected initialize response");
    };
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;

    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;

    let process_start_id = server
        .send_request(
            "process/start",
            serde_json::json!({
                "processId": "proc-resume",
                "argv": ["/bin/sh", "-c", "sleep 5"],
                "cwd": std::env::current_dir()?,
                "env": {},
                "tty": false,
                "pipeStdin": false,
                "arg0": null
            }),
        )
        .await?;
    let _ = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &process_start_id
            )
        })
        .await?;

    server.disconnect_websocket().await?;
    server.reconnect_websocket().await?;

    let resume_initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: Some(initialize_response.session_id.clone()),
            })?,
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &resume_initialize_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        panic!("expected resume initialize response");
    };
    let resumed_response: InitializeResponse = serde_json::from_value(result)?;
    assert_eq!(resumed_response, initialize_response);

    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;

    let process_read_id = server
        .send_request(
            "process/read",
            serde_json::json!({
                "processId": "proc-resume",
                "afterSeq": null,
                "maxBytes": null,
                "waitMs": 0
            }),
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &process_read_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        panic!("expected process/read response");
    };
    let process_read_response: ReadResponse = serde_json::from_value(result)?;
    assert!(process_read_response.failure.is_none());
    assert!(!process_read_response.exited);
    assert!(!process_read_response.closed);

    let terminate_id = server
        .send_request(
            "process/terminate",
            serde_json::json!({
                "processId": "proc-resume"
            }),
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &terminate_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        panic!("expected process/terminate response");
    };
    let terminate_response: TerminateResponse = serde_json::from_value(result)?;
    assert_eq!(terminate_response, TerminateResponse { running: true });

    server.shutdown().await?;
    Ok(())
}
