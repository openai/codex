//! Stdio shutdown cleans up owned commands and bounds teardown when I/O stalls.

use anyhow::Context;
use anyhow::Result;
use app_test_support::DISABLE_PLUGIN_STARTUP_TASKS_ARG;
use app_test_support::TestAppServer;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cargo_bin::cargo_bin;
use core_test_support::skip_if_remote;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::io::ErrorKind;
use std::io::Read;
use std::process::Command;
use std::process::Stdio;
use std::time::Instant;
use tempfile::TempDir;
use test_case::test_case;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::unix::pipe;
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;

#[tokio::test]
async fn stdio_sigterm_exits_with_open_pipes() -> Result<()> {
    let mut app = TestAppServer::builder().build_initialized().await?;

    app.send_sigterm()?;
    // wait_for_exit leaves both pipes open and does not drain stdout. Success catches an
    // unhandled SIGTERM, which exits before any cleanup can run.
    let status = timeout(Duration::from_secs(10), app.wait_for_exit())
        .await
        .context("app-server did not exit after SIGTERM")??;
    assert!(
        status.success(),
        "app-server did not exit cleanly: {status}"
    );
    Ok(())
}

#[tokio::test]
async fn stdio_sigterm_times_out_with_blocked_stderr() -> Result<()> {
    // TestAppServer drains stderr, so keep this subprocess's stderr pipe unread.
    let codex_home = TempDir::new()?;
    let mut process = tokio::process::Command::new(cargo_bin("codex-app-server")?)
        .arg(DISABLE_PLUGIN_STARTUP_TASKS_ARG)
        .env("CODEX_HOME", codex_home.path())
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.path().join("managed_config.toml"),
        )
        .env("RUST_LOG", "codex_app_server_transport=error")
        .current_dir(codex_home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = process.stdin.take().context("app-server has no stdin")?;
    let mut stdout =
        BufReader::new(process.stdout.take().context("app-server has no stdout")?).lines();
    let initialize = json!({
        "id": 1,
        "method": "initialize",
        "params": { "clientInfo": { "name": "stdio-stderr-test", "version": "1" } }
    });
    stdin
        .write_all(format!("{initialize}\n").as_bytes())
        .await?;
    let response = timeout(Duration::from_secs(10), stdout.next_line())
        .await??
        .context("app-server did not initialize")?;
    assert!(matches!(
        serde_json::from_str::<JSONRPCMessage>(&response)?,
        JSONRPCMessage::Response(_)
    ));

    // Invalid JSON logs synchronously in the reader. Fill stderr until the
    // transport stops consuming stdin, then keep all three pipes open.
    let invalid_input = b"{\n".repeat(1024 * 1024);
    assert!(
        timeout(Duration::from_secs(2), stdin.write_all(&invalid_input))
            .await
            .is_err(),
        "expected unread stderr to block the transport"
    );
    let signalled_at = Instant::now();
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(process.id().context("app-server has no pid")?.to_string())
        .status()?;
    assert!(status.success(), "failed to signal app-server: {status}");
    let status = timeout(Duration::from_secs(55), process.wait())
        .await
        .context("blocked stderr prevented SIGTERM shutdown")??;
    assert_eq!(status.code(), Some(1));
    assert!(signalled_at.elapsed() >= Duration::from_secs(45));
    drop(stdin);
    Ok(())
}

#[test_case(Shutdown::Sigterm; "sigterm")]
#[test_case(Shutdown::Eof; "eof_then_sigterm")]
#[tokio::test]
async fn stdio_shutdown_times_out_with_blocked_file_write(shutdown: Shutdown) -> Result<()> {
    let codex_home = TempDir::new()?;
    let path = codex_home.path().join("blocked-write");
    let status = Command::new("mkfifo").arg(&path).status()?;
    assert!(status.success(), "failed to create FIFO: {status}");
    let mut reader = std::fs::File::from(
        pipe::OpenOptions::new()
            .open_receiver(&path)?
            .into_nonblocking_fd()?,
    );
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        // The FIFO belongs to the app-server's local filesystem, even in remote CI.
        .without_auto_env()
        .build_initialized()
        .await?;
    app.send_fs_write_file_request(FsWriteFileParams {
        path: AbsolutePathBuf::try_from(path)?,
        data_base64: STANDARD.encode(vec![b'x'; 1024 * 1024]),
    })
    .await?;

    // Confirm the write has started, then leave the FIFO unread. The blocking
    // filesystem write cannot finish, even if its async request is cancelled.
    let mut first_byte = [0];
    timeout(Duration::from_secs(10), async {
        loop {
            match reader.read(&mut first_byte) {
                Ok(0) => {}
                Ok(_) => break,
                Err(err) if err.kind() == ErrorKind::WouldBlock => {}
                Err(err) => return Err(err),
            }
            sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, std::io::Error>(())
    })
    .await
    .context("filesystem write did not start")??;
    assert_eq!(first_byte, [b'x']);

    let shutdown_at = Instant::now();
    match shutdown {
        Shutdown::Sigterm => app.send_sigterm()?,
        Shutdown::Eof => {
            // Close stdin, then let the 30s RPC drain finish and enter runtime
            // teardown before SIGTERM. Keep the FIFO blocked throughout.
            assert!(
                timeout(Duration::from_secs(35), app.shutdown_gracefully())
                    .await
                    .is_err(),
                "app-server exited before allowing graceful cleanup"
            );
            app.send_sigterm()?;
        }
    }
    let remaining = Duration::from_secs(55).saturating_sub(shutdown_at.elapsed());
    let status = timeout(remaining, app.wait_for_exit())
        .await
        .context("app-server did not exit within the shutdown deadline")??;
    assert_eq!(status.code(), Some(1));
    assert!(
        shutdown_at.elapsed() >= Duration::from_secs(45),
        "app-server exited before allowing graceful cleanup"
    );
    drop(reader);
    Ok(())
}

#[derive(Clone, Copy)]
enum Shutdown {
    Eof,
    Sigterm,
}

#[test_case(Shutdown::Eof; "eof")]
#[test_case(Shutdown::Sigterm; "sigterm")]
#[tokio::test]
async fn stdio_shutdown_terminates_commands_and_children(shutdown: Shutdown) -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "asserts that processes owned by the local app-server exit"
    );
    let workspace = TempDir::new()?;
    let mut app = TestAppServer::builder().build_initialized().await?;

    let script = r#"sleep 120 & child=$!; printf '%s %s\n' "$$" "$child" > pids; wait "$child""#;
    app.send_command_exec_request(CommandExecParams {
        command: vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()],
        process_id: Some("shutdown-test".to_string()),
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: true,
        output_bytes_cap: None,
        disable_output_cap: false,
        disable_timeout: true,
        timeout_ms: None,
        cwd: Some(workspace.path().to_path_buf()),
        env: None,
        size: None,
        sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
        permission_profile: None,
    })
    .await?;

    let mut children = ProcessCleanup(Vec::new());
    timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(pids) = tokio::fs::read_to_string(workspace.path().join("pids")).await {
                let pids = pids
                    .split_whitespace()
                    .map(str::parse::<u32>)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if pids.len() == 2 {
                    children.0 = pids;
                    break;
                }
            }
            sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("command did not start")??;
    children.0.sort_unstable();
    assert_eq!(running_process_ids(&children.0)?, children.0);

    if matches!(shutdown, Shutdown::Sigterm) {
        let path = workspace.path().join("large-response");
        std::fs::write(&path, vec![b'x'; 1024 * 1024])?;
        app.send_fs_read_file_request(FsReadFileParams {
            path: AbsolutePathBuf::try_from(path)?,
        })
        .await?;
        timeout(Duration::from_secs(10), async {
            while !String::from_utf8_lossy(app.peek_stdout().await?).contains("dataBase64") {
                app.read_next_message().await?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("large response did not start")??;
        // Leave the response unread so stdout remains blocked during shutdown.
    }

    let status = match shutdown {
        Shutdown::Eof => timeout(Duration::from_secs(10), app.shutdown_gracefully()).await??,
        Shutdown::Sigterm => {
            app.send_sigterm()?;
            timeout(Duration::from_secs(10), app.wait_for_exit())
                .await
                .context("app-server did not exit after SIGTERM")??
        }
    };
    assert!(
        status.success(),
        "app-server did not exit cleanly: {status}"
    );

    timeout(Duration::from_secs(5), async {
        loop {
            if running_process_ids(&children.0)?.is_empty() {
                children.0.clear();
                return Ok::<_, anyhow::Error>(());
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("command or its child survived app-server shutdown")??;
    Ok(())
}

fn running_process_ids(pids: &[u32]) -> Result<Vec<u32>> {
    let output = Command::new("ps")
        .args(["-o", "pid=,stat=", "-p"])
        .arg(
            pids.iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
        .output()?;
    anyhow::ensure!(
        output.status.success() || output.status.code() == Some(1),
        "ps failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    let mut running = Vec::new();
    for line in String::from_utf8(output.stdout)?.lines() {
        let mut fields = line.split_whitespace();
        let pid = fields.next().context("missing process id")?.parse()?;
        let state = fields.next().context("missing process state")?;
        // An orphaned zombie has exited but may not yet have been reaped
        // by init (notably in Linux test containers).
        if !state.starts_with('Z') {
            running.push(pid);
        }
    }
    running.sort_unstable();
    Ok(running)
}

struct ProcessCleanup(Vec<u32>);

impl Drop for ProcessCleanup {
    fn drop(&mut self) {
        for pid in &self.0 {
            let _ = Command::new("kill")
                .arg("-KILL")
                .arg(pid.to_string())
                .output();
        }
    }
}
