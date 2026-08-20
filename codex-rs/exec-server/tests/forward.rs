mod common;

#[path = "common/relay.rs"]
mod relay_support;

use std::collections::HashMap;

use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecServerError;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::FsWriteFileParams;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadParams;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use relay_support::RelayTest;
use relay_support::TEST_TIMEOUT;
use tempfile::TempDir;
use tokio::time::timeout;
use tokio_util::task::AbortOnDropHandle;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn forwarder_runs_commands_and_transfers_files() -> Result<()> {
    let relay = RelayTest::new().await?;
    let mut destination = common::exec_server::exec_server().await?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let forwarder = AbortOnDropHandle::new(tokio::spawn(
        codex_exec_server::run_remote_environment_forward_until_shutdown(
            relay.config()?,
            destination.websocket_url().to_string(),
            async move {
                let _ = shutdown_rx.await;
            },
        ),
    ));
    let connection = relay.connect().await?;
    let client = &connection.client;

    for (process_id, expected_output, expected_exit) in [
        ("forward-success", "forwarded output", 0),
        ("forward-failure", "nonzero output", 7),
    ] {
        let process_id = ProcessId::from(process_id);
        let argv = if cfg!(windows) {
            vec![
                "cmd.exe".to_string(),
                "/D".to_string(),
                "/C".to_string(),
                format!("echo {expected_output}& exit /B {expected_exit}"),
            ]
        } else {
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                format!("printf '%s\\n' '{expected_output}'; exit {expected_exit}"),
            ]
        };
        timeout(
            TEST_TIMEOUT,
            client.exec(ExecParams {
                process_id: process_id.clone(),
                argv,
                cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
                shell_snapshot: None,
                env_policy: None,
                env: HashMap::new(),
                tty: false,
                pipe_stdin: false,
                arg0: None,
                sandbox: None,
                enforce_managed_network: false,
                managed_network: None,
                network_proxy: None,
            }),
        )
        .await??;
        let (output, exit_code) = timeout(TEST_TIMEOUT, async {
            let mut output = Vec::new();
            let mut after_seq = None;
            loop {
                let response = client
                    .read(ReadParams {
                        process_id: process_id.clone(),
                        after_seq,
                        max_bytes: None,
                        wait_ms: Some(1_000),
                    })
                    .await?;
                assert_eq!(response.failure, None);
                for chunk in response.chunks {
                    assert_eq!(chunk.stream, ExecOutputStream::Stdout);
                    output.extend(chunk.chunk.0);
                }
                if response.closed {
                    break Ok::<_, ExecServerError>((output, response.exit_code));
                }
                after_seq = response.next_seq.checked_sub(1);
            }
        })
        .await??;
        assert_eq!(
            (String::from_utf8(output)?.replace("\r\n", "\n"), exit_code),
            (format!("{expected_output}\n"), Some(expected_exit))
        );
    }

    // Base64 exceeds the destination's 16 MiB frame limit.
    let temp_dir = TempDir::new()?;
    let path = temp_dir.path().join("large-request.bin");
    let contents = vec![0xa5; 13 * 1024 * 1024];
    timeout(
        TEST_TIMEOUT,
        client.fs_write_file(FsWriteFileParams {
            path: PathUri::from_host_native_path(&path)?,
            follow_symlinks: None,
            data_base64: STANDARD.encode(&contents),
            sandbox: None,
        }),
    )
    .await??;
    // A following request must still arrive as its own complete message.
    let read_response = client
        .fs_read_file(FsReadFileParams {
            path: PathUri::from_host_native_path(path)?,
            follow_symlinks: None,
            sandbox: None,
        })
        .await?;
    assert_eq!(STANDARD.decode(read_response.data_base64)?, contents);
    connection.assert_encrypted()?;
    connection.close().await;
    let _ = shutdown_tx.send(());
    timeout(TEST_TIMEOUT, forwarder).await???;
    destination.shutdown().await?;
    Ok(())
}
