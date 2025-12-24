use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Context;
use assert_cmd::Command;
use pretty_assertions::assert_eq;

#[cfg(unix)]
use std::os::unix::net::UnixListener;

#[cfg(windows)]
use uds_windows::UnixListener;

#[test]
fn pipes_stdin_and_stdout_through_socket() -> anyhow::Result<()> {
    let dir = tempfile::TempDir::new().context("failed to create temp dir")?;
    let socket_path = dir.path().join("socket");
    let listener = match UnixListener::bind(&socket_path) {
        Ok(listener) => listener,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping test: failed to bind unix socket: {err}");
            return Ok(());
        }
        Err(err) => {
            return Err(err).context("failed to bind test unix socket");
        }
    };

    let (tx, rx) = mpsc::channel();
    let server_thread = thread::spawn(move || -> anyhow::Result<()> {
        let (mut connection, _) = listener
            .accept()
            .context("failed to accept test connection")?;
        let mut received = Vec::new();
        connection
            .read_to_end(&mut received)
            .context("failed to read data from client")?;
        tx.send(received)
            .map_err(|_| anyhow::anyhow!("failed to send received bytes to test thread"))?;
        connection
            .write_all(b"response")
            .context("failed to write response to client")?;
        Ok(())
    });

    Command::new(codex_utils_cargo_bin::cargo_bin("codex-stdio-to-uds")?)
        .arg(&socket_path)
        .write_stdin("request")
        .assert()
        .success()
        .stdout("response");

    let received = rx
        .recv_timeout(Duration::from_secs(1))
        .context("server did not receive data in time")?;
    assert_eq!(received, b"request");

    let server_result = server_thread
        .join()
        .map_err(|_| anyhow::anyhow!("server thread panicked"))?;
    server_result.context("server failed")?;

    Ok(())
}
