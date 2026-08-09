use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::grpc::code_mode_host_client::CodeModeHostClient;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::timeout;
use tonic::transport::Endpoint;

const TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);

#[tokio::test]
async fn tcp_listener_opens_a_grpc_session() -> Result<()> {
    let mut host = Command::new(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?)
        .args(["--listen", "grpc://127.0.0.1:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(/*kill_on_drop*/ true)
        .spawn()
        .context("failed to start gRPC code-mode host")?;
    let stdout = host
        .stdout
        .take()
        .context("gRPC code-mode host stdout is unavailable")?;
    let mut stdout = BufReader::new(stdout);
    let mut endpoint = String::new();
    timeout(TEST_TIMEOUT, stdout.read_line(&mut endpoint))
        .await
        .context("gRPC code-mode host did not publish its endpoint")??;
    let endpoint = Endpoint::from_shared(endpoint.trim().to_string())
        .context("gRPC code-mode host published an invalid endpoint")?
        .connect_timeout(TEST_TIMEOUT)
        .timeout(TEST_TIMEOUT);
    let mut client = CodeModeHostClient::connect(endpoint)
        .await
        .context("failed to connect to gRPC code-mode host")?;
    let mut events = client
        .open_session(grpc::OpenSessionRequest {
            cell_execution_limits: None,
        })
        .await
        .context("failed to open gRPC code-mode session")?
        .into_inner();
    let event = timeout(TEST_TIMEOUT, events.message())
        .await
        .context("timed out waiting for gRPC code-mode session event")?
        .context("failed to read gRPC code-mode session event")?
        .context("gRPC code-mode session ended before opening")?;
    assert!(matches!(
        event.event,
        Some(grpc::session_event::Event::Opened(opened)) if !opened.session_id.is_empty()
    ));
    Ok(())
}
