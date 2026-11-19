//! Exercises the process-owned gRPC transport against the standalone host.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode::CodeModeProcessMetricsSample;
use codex_code_mode::CodeModeSession;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_code_mode::NoopCodeModeSessionDelegate;
use codex_code_mode::RuntimeResponse;
use codex_utils_process::background_command;
#[cfg(unix)]
use core_test_support::code_mode_host::CodeModeHostRecorder;
use hyper_util::rt::TokioIo;
use pretty_assertions::assert_eq;
use tokio::process::Command;
use tokio::time::timeout;
use tonic::transport::Endpoint;
use tower::service_fn;

use super::ProcessOwnedGrpcCodeModeSessionProvider;

const TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 20);

#[cfg(unix)]
#[path = "code_mode_host_lifecycle_tests.rs"]
mod lifecycle;

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

#[test]
fn unavailable_host_keeps_a_bounded_utf8_path_suffix() {
    let executable = "codex-code-mode-host-does-not-exist";
    for directory in ["missing-directory/", "🦀"] {
        let path = format!("{}{executable}", directory.repeat(512));
        let provider = ProcessOwnedGrpcCodeModeSessionProvider::with_host_program(path.into());
        let error = provider.availability().expect_err("missing host");
        let displayed = error
            .strip_prefix("failed to spawn code-mode host ")
            .and_then(|message| message.strip_suffix(": host executable was not found"))
            .expect("missing-host diagnostic includes its path");
        assert!(displayed.starts_with("..."));
        assert!(displayed.ends_with(executable));
        assert!(displayed.len() <= 512);
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
    assert_eq!(
        provider
            .sample_process_metrics()
            .await
            .map_err(anyhow::Error::msg)?,
        CodeModeProcessMetricsSample::NotRunning
    );
    #[cfg(unix)]
    assert!(recorder.invocations()?.is_empty());
    let (first, second) = tokio::try_join!(provider.create_session(), provider.create_session())
        .map_err(anyhow::Error::msg)?;
    assert_eq!(
        provider
            .sample_process_metrics()
            .await
            .map_err(anyhow::Error::msg)?,
        CodeModeProcessMetricsSample::Unsupported
    );
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
    drop(first);
    drop(second);
    drop(provider);
    #[cfg(unix)]
    {
        let pid = recorder.pid()?;
        timeout(TEST_TIMEOUT, async {
            while unsafe {
                libc::kill(pid as libc::pid_t, /*sig*/ 0)
            } == 0
            {
                tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
            }
        })
        .await
        .context("metrics retained the host after provider teardown")?;
    }
    Ok(())
}

#[tokio::test]
async fn default_host_serves_grpc_and_exits_when_stdin_closes() -> Result<()> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut child = Command::from(background_command(program))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(/*kill_on_drop*/ true)
        .spawn()?;
    let stdout = child.stdout.take().context("host stdout")?;
    let stdin = child.stdin.take().context("host stdin")?;
    let mut pipes = Some(TokioIo::new(tokio::io::join(stdout, stdin)));
    let channel = timeout(
        TEST_TIMEOUT,
        Endpoint::from_static("http://code-mode-host").connect_with_connector(service_fn(
            move |_| {
                std::future::ready(
                    pipes
                        .take()
                        .ok_or_else(|| std::io::Error::other("host pipes already used")),
                )
            },
        )),
    )
    .await??;
    let provider = GrpcCodeModeSessionProvider::with_channel(channel);
    let session = timeout(TEST_TIMEOUT, provider.create_session())
        .await
        .context("timed out opening a session on the default host")?
        .map_err(anyhow::Error::msg)?;
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    drop(session);
    drop(provider);
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
    // A metrics deadline cancels only its stream. It must neither reap the
    // suspended child nor advance the independent 30s/10s keepalive deadline.
    assert!(provider.sample_process_metrics().await.is_err());
    assert_eq!(
        unsafe {
            libc::kill(pid as libc::pid_t, /*sig*/ 0)
        },
        0
    );
    assert_eq!(recorder.invocations()?.len(), 1);
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
        provider
            .sample_process_metrics()
            .await
            .map_err(anyhow::Error::msg)?,
        CodeModeProcessMetricsSample::NotRunning
    );
    assert_eq!(
        recorder.invocations()?.len(),
        1,
        "metrics must not restart a dead host"
    );
    let reset = execute(&session, r#"store("recovery", "must not run");"#)
        .await
        .unwrap_err();
    assert!(
        reset
            .to_string()
            .contains("Stored values and running cells were lost")
    );
    assert_eq!(
        execute(&session, r#"text(load("recovery") ?? "recovered");"#).await?,
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
