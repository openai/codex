#![cfg(unix)]

use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecServerClient;
use codex_exec_server::ExecServerClientConnectOptions;
use codex_exec_server::ExecServerEvent;
use codex_exec_server::ExecServerLaunchCommand;
use codex_exec_server::FsCopyParams;
use codex_exec_server::FsCreateDirectoryParams;
use codex_exec_server::FsGetMetadataParams;
use codex_exec_server::FsReadDirectoryParams;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::FsRemoveParams;
use codex_exec_server::FsWriteFileParams;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server::RemoteExecServerConnectArgs;
use codex_exec_server::spawn_local_exec_server;
use codex_utils_cargo_bin::cargo_bin;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::broadcast;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_accepts_initialize_over_stdio() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let mut child = Command::new(binary);
    child.stdin(Stdio::piped());
    child.stdout(Stdio::piped());
    child.stderr(Stdio::inherit());
    let mut child = child.spawn()?;

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut stdout = BufReader::new(stdout).lines();

    let initialize = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "initialize".to_string(),
        params: Some(serde_json::to_value(InitializeParams {
            client_name: "exec-server-test".to_string(),
        })?),
        trace: None,
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&initialize)?).as_bytes())
        .await?;

    let response_line = timeout(Duration::from_secs(5), stdout.next_line()).await??;
    let response_line = response_line.expect("response line");
    let response: JSONRPCMessage = serde_json::from_str(&response_line)?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response");
    };
    assert_eq!(id, RequestId::Integer(1));
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;
    assert_eq!(initialize_response.protocol_version, "exec-server.v0");

    let initialized = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: Some(serde_json::json!({})),
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&initialized)?).as_bytes())
        .await?;

    child.start_kill()?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_client_streams_output_and_accepts_writes() -> anyhow::Result<()> {
    let mut env = std::collections::HashMap::new();
    if let Some(path) = std::env::var_os("PATH") {
        env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
    }

    let server = spawn_local_exec_server(
        ExecServerLaunchCommand {
            program: cargo_bin("codex-exec-server")?,
            args: Vec::new(),
        },
        ExecServerClientConnectOptions {
            client_name: "exec-server-test".to_string(),
            initialize_timeout: Duration::from_secs(5),
        },
    )
    .await?;

    let client = server.client();
    let mut events = client.event_receiver();
    let response = client
        .exec(ExecParams {
            process_id: "proc-1".to_string(),
            argv: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "printf 'ready\\n'; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done"
                    .to_string(),
            ],
            cwd: std::env::current_dir()?,
            env,
            tty: true,
            arg0: None,
        })
        .await?;
    let process_id = response.process_id;

    let (stream, ready_output) = recv_until_contains(&mut events, &process_id, "ready").await?;
    assert_eq!(stream, ExecOutputStream::Pty);
    assert!(
        ready_output.contains("ready"),
        "expected initial ready output"
    );

    client.write(&process_id, b"hello\n".to_vec()).await?;

    let (stream, echoed_output) =
        recv_until_contains(&mut events, &process_id, "echo:hello").await?;
    assert_eq!(stream, ExecOutputStream::Pty);
    assert!(
        echoed_output.contains("echo:hello"),
        "expected echoed output"
    );

    client.terminate(&process_id).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_client_connects_over_websocket() -> anyhow::Result<()> {
    let mut env = std::collections::HashMap::new();
    if let Some(path) = std::env::var_os("PATH") {
        env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
    }

    let binary = cargo_bin("codex-exec-server")?;
    let mut child = Command::new(binary);
    child.args(["--listen", "ws://127.0.0.1:0"]);
    child.stdin(Stdio::null());
    child.stdout(Stdio::null());
    child.stderr(Stdio::piped());
    let mut child = child.spawn()?;
    let stderr = child.stderr.take().expect("stderr");
    let mut stderr_lines = BufReader::new(stderr).lines();
    let websocket_url = read_websocket_url(&mut stderr_lines).await?;

    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs {
        websocket_url,
        client_name: "exec-server-test".to_string(),
        connect_timeout: Duration::from_secs(5),
        initialize_timeout: Duration::from_secs(5),
    })
    .await?;

    let mut events = client.event_receiver();
    let response = client
        .exec(ExecParams {
            process_id: "proc-1".to_string(),
            argv: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "printf 'ready\\n'; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done"
                    .to_string(),
            ],
            cwd: std::env::current_dir()?,
            env,
            tty: true,
            arg0: None,
        })
        .await?;
    let process_id = response.process_id;

    let (stream, ready_output) = recv_until_contains(&mut events, &process_id, "ready").await?;
    assert_eq!(stream, ExecOutputStream::Pty);
    assert!(
        ready_output.contains("ready"),
        "expected initial ready output"
    );

    client.write(&process_id, b"hello\n".to_vec()).await?;

    let (stream, echoed_output) =
        recv_until_contains(&mut events, &process_id, "echo:hello").await?;
    assert_eq!(stream, ExecOutputStream::Pty);
    assert!(
        echoed_output.contains("echo:hello"),
        "expected echoed output"
    );

    client.terminate(&process_id).await?;
    child.start_kill()?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_client_filesystem_round_trip_over_stdio() -> anyhow::Result<()> {
    let server = spawn_local_exec_server(
        ExecServerLaunchCommand {
            program: cargo_bin("codex-exec-server")?,
            args: Vec::new(),
        },
        ExecServerClientConnectOptions {
            client_name: "exec-server-test".to_string(),
            initialize_timeout: Duration::from_secs(5),
        },
    )
    .await?;
    let client = server.client();

    let root = std::env::temp_dir().join(format!(
        "codex-exec-server-fs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let directory = root.join("dir");
    let file_path = directory.join("hello.txt");
    let copy_path = directory.join("copy.txt");

    client
        .fs_create_directory(FsCreateDirectoryParams {
            path: directory.clone().try_into()?,
            recursive: Some(true),
        })
        .await?;

    client
        .fs_write_file(FsWriteFileParams {
            path: file_path.clone().try_into()?,
            data_base64: BASE64_STANDARD.encode(b"hello"),
        })
        .await?;

    let metadata = client
        .fs_get_metadata(FsGetMetadataParams {
            path: file_path.clone().try_into()?,
        })
        .await?;
    assert!(metadata.is_file);
    assert!(!metadata.is_directory);

    let read_file = client
        .fs_read_file(FsReadFileParams {
            path: file_path.clone().try_into()?,
        })
        .await?;
    assert_eq!(read_file.data_base64, BASE64_STANDARD.encode(b"hello"));

    let read_directory = client
        .fs_read_directory(FsReadDirectoryParams {
            path: directory.clone().try_into()?,
        })
        .await?;
    assert!(
        read_directory
            .entries
            .iter()
            .any(|entry| entry.file_name == "hello.txt" && entry.is_file)
    );

    client
        .fs_copy(FsCopyParams {
            source_path: file_path.clone().try_into()?,
            destination_path: copy_path.clone().try_into()?,
            recursive: false,
        })
        .await?;
    let copied = client
        .fs_read_file(FsReadFileParams {
            path: copy_path.clone().try_into()?,
        })
        .await?;
    assert_eq!(copied.data_base64, BASE64_STANDARD.encode(b"hello"));

    client
        .fs_remove(FsRemoveParams {
            path: root.clone().try_into()?,
            recursive: Some(true),
            force: Some(true),
        })
        .await?;

    assert!(
        !root.exists(),
        "filesystem cleanup should remove the test tree"
    );
    Ok(())
}

async fn read_websocket_url<R>(lines: &mut tokio::io::Lines<BufReader<R>>) -> anyhow::Result<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let line = timeout(Duration::from_secs(5), lines.next_line()).await??;
    let line = line.context("missing websocket startup banner")?;
    let websocket_url = line
        .split_whitespace()
        .find(|part| part.starts_with("ws://"))
        .context("missing websocket URL in startup banner")?;
    Ok(websocket_url.to_string())
}

async fn recv_until_contains(
    events: &mut broadcast::Receiver<ExecServerEvent>,
    process_id: &str,
    needle: &str,
) -> anyhow::Result<(ExecOutputStream, String)> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut collected = String::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = timeout(remaining, events.recv()).await??;
        if let ExecServerEvent::OutputDelta(output_event) = event
            && output_event.process_id == process_id
        {
            collected.push_str(&String::from_utf8_lossy(&output_event.chunk.into_inner()));
            if collected.contains(needle) {
                return Ok((output_event.stream, collected));
            }
        }
    }
}
