//! Exercises the process-owned gRPC transport against the standalone host.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode::CodeModeSession;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::NoopCodeModeSessionDelegate;
use codex_code_mode::RuntimeResponse;
#[cfg(unix)]
use core_test_support::code_mode_host::CodeModeHostRecorder;
use pretty_assertions::assert_eq;
use tokio::time::timeout;

use super::HostConnection;
use super::ProcessOwnedGrpcCodeModeSessionProvider;

const TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 20);

async fn execute(
    session: &Arc<dyn CodeModeSession>,
    source: &str,
) -> Result<Vec<FunctionCallOutputContentItem>> {
    let response = timeout(TEST_TIMEOUT, async {
        session
            .execute(
                ExecuteRequest {
                    tool_call_id: "call-1".to_string(),
                    enabled_tools: Vec::new(),
                    source: source.to_string(),
                    yield_time_ms: Some(/*value*/ 5_000),
                    max_output_tokens: Some(/*value*/ 1_000),
                },
                Arc::new(NoopCodeModeSessionDelegate),
                /*preempt*/ None,
            )
            .await?
            .initial_response()
            .await
    })
    .await
    .context("timed out executing a stdio gRPC cell")?
    .map_err(anyhow::Error::msg)?;
    match response {
        RuntimeResponse::Result {
            content_items,
            error_text: None,
            ..
        } => Ok(content_items),
        other => anyhow::bail!("unexpected code-mode response: {other:?}"),
    }
}

#[tokio::test]
async fn grpc_stdio_sessions_share_a_host_and_isolate_state() -> Result<()> {
    #[cfg(unix)]
    let recorder = CodeModeHostRecorder::new()?;
    #[cfg(unix)]
    let host_program = recorder.program();
    #[cfg(not(unix))]
    let host_program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let provider = ProcessOwnedGrpcCodeModeSessionProvider::with_host_program(host_program);
    let (first, second) = tokio::try_join!(provider.create_session(), provider.create_session())
        .map_err(anyhow::Error::msg)?;
    assert_eq!(
        execute(&first, "store(\"stored\", 42); text(load(\"stored\"));").await?,
        vec![FunctionCallOutputContentItem::InputText {
            text: "42".to_string()
        }]
    );
    assert_eq!(
        execute(&second, "text(typeof load(\"stored\"));").await?,
        vec![FunctionCallOutputContentItem::InputText {
            text: "undefined".to_string()
        }]
    );
    first.shutdown().await.map_err(anyhow::Error::msg)?;
    assert_eq!(
        execute(&second, "text(\"still open\");").await?,
        vec![FunctionCallOutputContentItem::InputText {
            text: "still open".to_string()
        }]
    );
    second.shutdown().await.map_err(anyhow::Error::msg)?;
    #[cfg(unix)]
    assert_eq!(
        recorder.invocations()?,
        vec!["--listen grpc+stdio://".to_string()],
        "both sessions must share one gRPC host process"
    );
    Ok(())
}

#[tokio::test]
async fn grpc_stdio_host_exits_when_stdin_closes() -> Result<()> {
    let mut connection =
        HostConnection::spawn(&codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?)?;
    let mut child = connection.child.take().context("host process")?;
    drop(connection);
    let status = timeout(TEST_TIMEOUT, child.wait())
        .await
        .context("host did not exit on stdin EOF")??;
    assert!(status.success());
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn hung_grpc_stdio_host_is_reaped_and_the_session_recovers() -> Result<()> {
    let recorder = CodeModeHostRecorder::new()?;
    let provider = ProcessOwnedGrpcCodeModeSessionProvider::with_host_program(recorder.program());
    let session = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;
    let pid = recorder.pid()?;
    // Suspended processes keep their pipes open: EOF cannot detect this failure.
    if unsafe { libc::kill(pid as libc::pid_t, libc::SIGSTOP) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    timeout(Duration::from_secs(/*secs*/ 55), async {
        while unsafe {
            libc::kill(pid as libc::pid_t, /*sig*/ 0)
        } == 0
        {
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await
    .context("unresponsive host was not killed and reaped")?;
    assert_eq!(
        execute(&session, "text(\"recovered\");").await?,
        vec![FunctionCallOutputContentItem::InputText {
            text: "recovered".to_string()
        }]
    );
    assert_eq!(
        recorder.invocations()?,
        vec!["--listen grpc+stdio://".to_string(); 2],
        "the session must reconnect to a fresh gRPC host"
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}
