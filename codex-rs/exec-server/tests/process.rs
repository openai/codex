mod common;

use std::collections::HashMap;

use codex_exec_server::EnvironmentInfo;
use codex_exec_server::EnvironmentStatus;
use codex_exec_server::EnvironmentStatusKind;
use codex_exec_server::ExecResponse;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadResponse;
use codex_exec_server::TerminateResponse;
use codex_exec_server::WriteResponse;
use codex_exec_server::WriteStatus;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::ProcessSandboxType;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use common::exec_server::exec_server_with_env;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_starts_process_over_websocket() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let process_argv = if cfg!(windows) {
        vec!["cmd.exe", "/D", "/C", "exit 0"]
    } else {
        vec!["true"]
    };
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
                "argv": process_argv,
                "cwd": PathUri::from_host_native_path(std::env::current_dir()?)?,
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
            process_id: ProcessId::from("proc-1"),
            sandbox_type: Some(ProcessSandboxType::None),
        }
    );

    server.shutdown().await?;
    Ok(())
}

/// Ordinary requests run one at a time when concurrent processing is not enabled.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_runs_ordinary_requests_serially_by_default() -> anyhow::Result<()> {
    let temporary_directory = tempfile::tempdir()?;
    let temporary_directory_env_vars: &[&str] = if cfg!(windows) {
        &["TEMP", "TMP"]
    } else {
        &["TMPDIR"]
    };
    let mut server = exec_server_with_env(
        temporary_directory_env_vars
            .iter()
            .map(|name| (*name, temporary_directory.path())),
        &[],
    )
    .await?;
    let process_argv = if cfg!(windows) {
        vec!["cmd.exe", "/D", "/C", "ping -n 601 127.0.0.1 >NUL"]
    } else {
        vec![
            "/bin/sh",
            "-c",
            "parent=$PPID; while kill -0 \"$parent\" 2>/dev/null; do sleep 1; done",
        ]
    };
    let process_env = if cfg!(windows) {
        serde_json::json!({ "PATH": std::env::var("PATH")? })
    } else {
        serde_json::json!({})
    };
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
                "processId": "proc-serial-read",
                "argv": process_argv,
                "cwd": PathUri::from_host_native_path(std::env::current_dir()?)?,
                "env": process_env,
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

    let read_id = server
        .send_request(
            "process/read",
            serde_json::json!({
                "processId": "proc-serial-read",
                "afterSeq": null,
                "maxBytes": null,
                "waitMs": 250
            }),
        )
        .await?;
    let queued_environment_info_id = server
        .send_request("environment/info", serde_json::json!({}))
        .await?;
    let queued_start_id = server
        .send_request(
            "process/start",
            serde_json::json!({
                "processId": "proc-serial-queued",
                "argv": process_argv,
                "cwd": PathUri::from_host_native_path(std::env::current_dir()?)?,
                "env": process_env,
                "tty": false,
                "pipeStdin": false,
                "arg0": null
            }),
        )
        .await?;

    let response = server
        .wait_for_event(|event| matches!(event, JSONRPCMessage::Response(_)))
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, .. }) = response else {
        panic!("expected the blocked process/read to finish before the queued process/start");
    };
    assert_eq!(id, read_id);
    let response = server
        .wait_for_event(|event| matches!(event, JSONRPCMessage::Response(_)))
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected the queued environment/info response after process/read");
    };
    assert_eq!(id, queued_environment_info_id);
    let mut expected_environment_info = EnvironmentInfo::local();
    expected_environment_info.temporary_directories = Some(vec![PathUri::from_host_native_path(
        temporary_directory.path(),
    )?]);
    assert_eq!(
        serde_json::from_value::<EnvironmentInfo>(result)?,
        expected_environment_info
    );
    let response = server
        .wait_for_event(|event| matches!(event, JSONRPCMessage::Response(_)))
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected the queued process/start response after process/read");
    };
    assert_eq!(id, queued_start_id);
    assert_eq!(
        serde_json::from_value::<ExecResponse>(result)?,
        ExecResponse {
            process_id: ProcessId::from("proc-serial-queued"),
            sandbox_type: Some(ProcessSandboxType::None),
        }
    );

    for process_id in ["proc-serial-read", "proc-serial-queued"] {
        let terminate_id = server
            .send_request(
                "process/terminate",
                serde_json::json!({ "processId": process_id }),
            )
            .await?;
        server
            .wait_for_event(|event| {
                matches!(
                    event,
                    JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &terminate_id
                )
            })
            .await?;
    }

    server.shutdown().await?;
    Ok(())
}

/// A long read cannot block health checks; saturated ordinary work cannot block health checks or cleanup.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_keeps_control_requests_live_during_long_reads_and_queued_requests()
-> anyhow::Result<()> {
    let mut server = exec_server_with_env(
        std::iter::empty::<(&str, &str)>(),
        &["--concurrent-requests", "32"],
    )
    .await?;
    let process_argv = if cfg!(windows) {
        vec!["cmd.exe", "/D", "/C", "ping -n 601 127.0.0.1 >NUL"]
    } else {
        vec![
            "/bin/sh",
            "-c",
            "parent=$PPID; while kill -0 \"$parent\" 2>/dev/null; do sleep 1; done",
        ]
    };
    let process_env = if cfg!(windows) {
        serde_json::json!({ "PATH": std::env::var("PATH")? })
    } else {
        serde_json::json!({})
    };
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;
    assert!(matches!(
        server.next_event().await?,
        JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == initialize_id
    ));
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;

    let process_start_id = server
        .send_request(
            "process/start",
            serde_json::json!({
                "processId": "proc-capacity",
                "argv": process_argv,
                "cwd": PathUri::from_host_native_path(std::env::current_dir()?)?,
                "env": process_env,
                "tty": false,
                "pipeStdin": false,
                "arg0": null
            }),
        )
        .await?;
    assert!(matches!(
        server.next_event().await?,
        JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == process_start_id
    ));

    let read_params = serde_json::json!({
        "processId": "proc-capacity",
        "afterSeq": null,
        "maxBytes": null,
        "waitMs": 600_000
    });
    server
        .send_request("process/read", read_params.clone())
        .await?;
    let concurrent_start_id = server
        .send_request(
            "process/start",
            serde_json::json!({
                "processId": "proc-concurrent",
                "argv": process_argv,
                "cwd": PathUri::from_host_native_path(std::env::current_dir()?)?,
                "env": process_env,
                "tty": false,
                "pipeStdin": false,
                "arg0": null
            }),
        )
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = server.next_event().await?
    else {
        panic!("expected process/start to finish before the pending process/read");
    };
    assert_eq!(id, concurrent_start_id);
    assert_eq!(
        serde_json::from_value::<ExecResponse>(result)?,
        ExecResponse {
            process_id: ProcessId::from("proc-concurrent"),
            sandbox_type: Some(ProcessSandboxType::None),
        }
    );
    for _ in 1..32 {
        server
            .send_request("process/read", read_params.clone())
            .await?;
    }

    let queued_read_id = server.send_request("process/read", read_params).await?;

    let environment_info_id = server
        .send_request("environment/info", serde_json::json!({}))
        .await?;
    let response = server
        .wait_for_event(|event| matches!(event, JSONRPCMessage::Response(_)))
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected environment/info response at regular request capacity");
    };
    assert_eq!(id, environment_info_id);
    let _: EnvironmentInfo = serde_json::from_value(result)?;

    let environment_status_id = server
        .send_request("environment/status", serde_json::json!({}))
        .await?;
    let response = server
        .wait_for_event(|event| matches!(event, JSONRPCMessage::Response(_)))
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected environment/status response at regular request capacity");
    };
    assert_eq!(id, environment_status_id);
    assert_eq!(
        serde_json::from_value::<EnvironmentStatus>(result)?,
        EnvironmentStatus {
            status: EnvironmentStatusKind::Ready,
        }
    );

    let terminate_id = server
        .send_request(
            "process/terminate",
            serde_json::json!({ "processId": "proc-capacity" }),
        )
        .await?;
    let mut terminate_response = None;
    let mut queued_read_completed = false;
    while terminate_response.is_none() || !queued_read_completed {
        match server.next_event().await? {
            JSONRPCMessage::Response(JSONRPCResponse { id, result }) if id == terminate_id => {
                terminate_response = Some(serde_json::from_value::<TerminateResponse>(result)?);
            }
            JSONRPCMessage::Response(JSONRPCResponse { id, result }) if id == queued_read_id => {
                let _: ReadResponse = serde_json::from_value(result)?;
                queued_read_completed = true;
            }
            JSONRPCMessage::Error(error) => {
                anyhow::bail!("unexpected error while waiting for queued requests: {error:?}");
            }
            JSONRPCMessage::Request(_)
            | JSONRPCMessage::Response(_)
            | JSONRPCMessage::Notification(_) => {}
        }
    }
    assert_eq!(
        terminate_response,
        Some(TerminateResponse { running: true })
    );

    let terminate_id = server
        .send_request(
            "process/terminate",
            serde_json::json!({ "processId": "proc-concurrent" }),
        )
        .await?;
    server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &terminate_id
            )
        })
        .await?;

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_defaults_omitted_pipe_stdin_to_closed_stdin() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let process_argv = if cfg!(windows) {
        vec!["cmd.exe", "/D", "/C", "ping -n 2 127.0.0.1 >NUL"]
    } else {
        vec![
            "/bin/sh",
            "-c",
            "sleep 0.3; if IFS= read -r line; then printf 'read:%s\\n' \"$line\"; else printf 'eof\\n'; fi",
        ]
    };
    let process_env = if cfg!(windows) {
        serde_json::json!({ "PATH": std::env::var("PATH")? })
    } else {
        serde_json::json!({})
    };
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
                "argv": process_argv,
                "cwd": PathUri::from_host_native_path(std::env::current_dir()?)?,
                "env": process_env,
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
            process_id: ProcessId::from("proc-default-stdin"),
            sandbox_type: Some(ProcessSandboxType::None),
        }
    );

    let write_id = server
        .send_request(
            "process/write",
            serde_json::json!({
                "processId": "proc-default-stdin",
                "chunk": "aWdub3JlZAo=",
                "writeId": "write-default-stdin"
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
async fn exec_server_dedupes_retried_process_write_ids() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let process_argv = if cfg!(windows) {
        vec![
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::Out.WriteLine('line:' + [Console]::In.ReadLine()); [Console]::Out.WriteLine('line:' + [Console]::In.ReadLine())",
        ]
    } else {
        vec![
            "/bin/sh",
            "-c",
            "IFS= read -r first; printf 'line:%s\\n' \"$first\"; IFS= read -r second; printf 'line:%s\\n' \"$second\"",
        ]
    };
    let process_env = if cfg!(windows) {
        serde_json::to_value(std::env::vars().collect::<HashMap<_, _>>())?
    } else {
        serde_json::json!({})
    };
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
                "processId": "proc-write-id",
                "argv": process_argv,
                "cwd": PathUri::from_host_native_path(std::env::current_dir()?)?,
                "env": process_env,
                "tty": false,
                "pipeStdin": true,
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

    for (write_id, chunk) in [
        ("write-1", "Zmlyc3QK"),
        ("write-1", "Zmlyc3QK"),
        ("write-2", "c2Vjb25kCg=="),
    ] {
        let request_id = server
            .send_request(
                "process/write",
                serde_json::json!({
                    "processId": "proc-write-id",
                    "chunk": chunk,
                    "writeId": write_id
                }),
            )
            .await?;
        let response = server
            .wait_for_event(|event| {
                matches!(
                    event,
                    JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &request_id
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
                status: WriteStatus::Accepted
            }
        );
    }

    let mut after_seq = None;
    let mut output = Vec::new();
    for _ in 0..5 {
        let read_id = server
            .send_request(
                "process/read",
                serde_json::json!({
                    "processId": "proc-write-id",
                    "afterSeq": after_seq,
                    "maxBytes": null,
                    "waitMs": 1000
                }),
            )
            .await?;
        let response = server
            .wait_for_event(|event| {
                matches!(
                    event,
                    JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &read_id
                )
            })
            .await?;
        let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
            panic!("expected process/read response");
        };
        let read_response: ReadResponse = serde_json::from_value(result)?;
        output.extend(
            read_response
                .chunks
                .into_iter()
                .flat_map(|chunk| chunk.chunk.into_inner()),
        );
        after_seq = Some(read_response.next_seq.saturating_sub(1));
        if read_response.closed
            || output.ends_with(b"line:second\n")
            || output.ends_with(b"line:second\r\n")
        {
            break;
        }
    }

    assert_eq!(
        String::from_utf8(output)?.replace("\r\n", "\n"),
        "line:first\nline:second\n".to_string()
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_resumes_detached_session_without_killing_processes() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let process_argv = if cfg!(windows) {
        vec!["cmd.exe", "/D", "/C", "ping -n 6 127.0.0.1 >NUL"]
    } else {
        vec!["/bin/sh", "-c", "sleep 5"]
    };
    let process_env = if cfg!(windows) {
        serde_json::json!({ "PATH": std::env::var("PATH")? })
    } else {
        serde_json::json!({})
    };
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
                "argv": process_argv,
                "cwd": PathUri::from_host_native_path(std::env::current_dir()?)?,
                "env": process_env,
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
