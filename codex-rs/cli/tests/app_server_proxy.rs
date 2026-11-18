use std::io::ErrorKind;

use anyhow::Context;
use codex_uds::UnixListener;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[tokio::test]
async fn pipes_stdin_and_stdout_through_socket() -> anyhow::Result<()> {
    const REQUEST: &[u8] = b"request";

    let dir = tempfile::TempDir::new().context("failed to create temp dir")?;
    let socket_path = dir.path().join("socket");
    let request_path = dir.path().join("request.txt");
    std::fs::write(&request_path, REQUEST).context("failed to write child stdin fixture")?;
    let mut listener = match UnixListener::bind(&socket_path).await {
        Ok(listener) => listener,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping test: failed to bind unix socket: {err}");
            return Ok(());
        }
        Err(err) => {
            return Err(err).context("failed to bind test unix socket");
        }
    };

    let exchange = async {
        let mut connection = listener
            .accept()
            .await
            .context("failed to accept test connection")?;
        // Reply as soon as the full request arrives, without waiting for EOF.
        let mut received = [0; REQUEST.len()];
        connection
            .read_exact(&mut received)
            .await
            .context("failed to read data from client")?;
        connection
            .write_all(b"response")
            .await
            .context("failed to write response to client")?;
        anyhow::Ok(received)
    };

    let proxy = async {
        let stdin =
            std::fs::File::open(&request_path).context("failed to open child stdin fixture")?;
        let command =
            codex_utils_process::background_command(codex_utils_cargo_bin::cargo_bin("codex")?);
        let output = Command::from(command)
            .args(["app-server", "proxy", "--sock"])
            .arg(&socket_path)
            .env("CODEX_HOME", dir.path())
            .stdin(stdin)
            .kill_on_drop(true)
            .output()
            .await
            .context("failed to run codex app-server proxy")?;
        anyhow::ensure!(
            output.status.success(),
            "codex app-server proxy exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim_end()
        );
        anyhow::Ok(output.stdout)
    };

    let (received, stdout) = tokio::try_join!(exchange, proxy)?;
    assert_eq!(
        (received.as_slice(), stdout.as_slice()),
        (REQUEST, b"response".as_slice())
    );
    Ok(())
}
