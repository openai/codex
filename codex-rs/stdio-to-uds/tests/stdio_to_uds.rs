use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::anyhow;
use pretty_assertions::assert_eq;

#[cfg(unix)]
use std::os::unix::net::UnixListener;

#[cfg(windows)]
use uds_windows::UnixListener;

#[test]
fn pipes_stdin_and_stdout_through_socket() -> anyhow::Result<()> {
    // This test intentionally avoids `read_to_end()` on the server side because
    // waiting for EOF can race with socket half-close behavior on slower runners.
    // Reading the exact request length keeps the test deterministic.
    //
    // We also use `std::process::Command` (instead of `assert_cmd`) so we can
    // poll/kill on timeout and include incremental server events + stderr in
    // failure output, which makes flaky failures actionable to debug.
    let dir = tempfile::TempDir::new().context("failed to create temp dir")?;
    let socket_path = dir.path().join("socket");
    let request = b"request";
    let request_path = dir.path().join("request.txt");
    std::fs::write(&request_path, request).context("failed to write child stdin fixture")?;
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
    let (event_tx, event_rx) = mpsc::channel();
    let server_thread = thread::spawn(move || -> anyhow::Result<()> {
        let _ = event_tx.send("waiting for accept".to_string());
        let (mut connection, _) = listener
            .accept()
            .context("failed to accept test connection")?;
        let _ = event_tx.send("accepted connection".to_string());
        let mut received = vec![0; request.len()];
        connection
            .read_exact(&mut received)
            .context("failed to read data from client")?;
        let _ = event_tx.send(format!("read {} bytes", received.len()));
        tx.send(received)
            .map_err(|_| anyhow!("failed to send received bytes to test thread"))?;
        connection
            .write_all(b"response")
            .context("failed to write response to client")?;
        let _ = event_tx.send("wrote response".to_string());
        Ok(())
    });

    let stdin = std::fs::File::open(&request_path).context("failed to open child stdin fixture")?;
    let mut child = Command::new(codex_utils_cargo_bin::cargo_bin("codex-stdio-to-uds")?)
        .arg(&socket_path)
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn codex-stdio-to-uds")?;

    let mut child_stdout = child.stdout.take().context("missing child stdout")?;
    let mut child_stderr = child.stderr.take().context("missing child stderr")?;
    let (stdout_tx, stdout_rx) = mpsc::channel();
    let (stderr_tx, stderr_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut stdout = Vec::new();
        let result = child_stdout.read_to_end(&mut stdout).map(|_| stdout);
        let _ = stdout_tx.send(result);
    });
    thread::spawn(move || {
        let mut stderr = Vec::new();
        let result = child_stderr.read_to_end(&mut stderr).map(|_| stderr);
        let _ = stderr_tx.send(result);
    });

    let mut server_events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        while let Ok(event) = event_rx.try_recv() {
            server_events.push(event);
        }

        if let Some(status) = child.try_wait().context("failed to poll child status")? {
            break status;
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = stderr_rx
                .recv_timeout(Duration::from_secs(1))
                .context("timed out waiting for child stderr after kill")?
                .context("failed to read child stderr")?;
            anyhow::bail!(
                "codex-stdio-to-uds did not exit in time; server events: {:?}; stderr: {}",
                server_events,
                String::from_utf8_lossy(&stderr).trim_end()
            );
        }

        thread::sleep(Duration::from_millis(25));
    };

    let stdout = stdout_rx
        .recv_timeout(Duration::from_secs(1))
        .context("timed out waiting for child stdout")?
        .context("failed to read child stdout")?;
    let stderr = stderr_rx
        .recv_timeout(Duration::from_secs(1))
        .context("timed out waiting for child stderr")?
        .context("failed to read child stderr")?;
    assert!(
        status.success(),
        "codex-stdio-to-uds exited with {status}; server events: {:?}; stderr: {}",
        server_events,
        String::from_utf8_lossy(&stderr).trim_end()
    );
    assert_eq!(stdout, b"response");

    let received = rx
        .recv_timeout(Duration::from_secs(5))
        .context("server did not receive data in time")?;
    assert_eq!(received, request);

    let server_result = server_thread
        .join()
        .map_err(|_| anyhow!("server thread panicked"))?;
    server_result.context("server failed")?;

    Ok(())
}
