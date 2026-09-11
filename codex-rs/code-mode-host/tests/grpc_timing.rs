//! Checks that remote timing measures each observation, not client delays or cell lifetime.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_code_mode::NoopCodeModeSessionDelegate;
use codex_code_mode::ProcessOwnedCodeModeSessionProvider;
use codex_code_mode::RuntimeResponse;
use codex_code_mode::WaitOutcome;
use codex_code_mode::WaitRequest;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::grpc::code_mode_host_client::CodeModeHostClient;
use pretty_assertions::assert_eq;
use tokio::time::sleep;
use tokio::time::timeout;

#[path = "support/host.rs"]
mod host;
#[expect(dead_code, reason = "Other suites use the shared cell_id helper.")]
#[path = "support/recording_delegate.rs"]
mod recording_delegate;

use host::HostHarness;
use recording_delegate::RecordingDelegate;

const TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 20);

/// Successful and failed JavaScript include computation and timers in their host timing.
/// Waiting to read a finished execution must not increase that measurement.
#[tokio::test]
async fn execution_timing_includes_javascript_but_excludes_delayed_reads() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let mut client = CodeModeHostClient::connect(host.endpoint).await?;
    let mut events = client
        .open_session(grpc::OpenSessionRequest {
            cell_execution_limits: None,
        })
        .await?
        .into_inner();
    let opened = timeout(TEST_TIMEOUT, events.message())
        .await??
        .context("session ended")?;
    let Some(grpc::session_event::Event::Opened(opened)) = opened.event else {
        anyhow::bail!("expected session opening event");
    };

    let endings = ["", "throw new Error('timed failure');"];
    for (index, suffix) in endings.into_iter().enumerate() {
        let observed = Instant::now();
        let mut execution = client
            .execute(grpc::ExecuteRequest {
                session_id: opened.session_id.clone(),
                execution_id: format!("execution-{index}"),
                tool_call_id: format!("call-{index}"),
                source: format!(
                    "const until = Date.now() + 150; while (Date.now() < until) {{}} \
                     await new Promise(resolve => setTimeout(resolve, 150)); {suffix}"
                ),
                enabled_tools: Vec::new(),
                yield_time_ms: Some(/*value*/ 5_000),
                max_output_tokens: Some(/*value*/ 1_000),
            })
            .await?
            .into_inner();
        let started = timeout(TEST_TIMEOUT, execution.message())
            .await??
            .context("execution ended")?;
        let Some(grpc::execute_event::Event::Started(started)) = started.event else {
            anyhow::bail!("expected execution admission");
        };
        let closed = timeout(TEST_TIMEOUT, events.message())
            .await??
            .context("session ended")?;
        assert!(
            matches!(closed.event, Some(grpc::session_event::Event::CellClosed(closed))
            if closed.cell_id == started.cell_id)
        );

        // CellClosed confirms the JS finished before introducing a client-only delay.
        sleep(Duration::from_millis(/*millis*/ 600)).await;
        let result = timeout(TEST_TIMEOUT, execution.message())
            .await??
            .context("execution ended")?;
        let Some(grpc::execute_event::Event::Outcome(outcome)) = result.event else {
            anyhow::bail!("expected execution outcome");
        };
        let duration = Duration::from_nanos(outcome.code_mode_host_duration_ns);
        assert!(duration >= Duration::from_millis(/*millis*/ 250));
        assert!(duration + Duration::from_millis(/*millis*/ 300) <= observed.elapsed());
        let Some(grpc::execution_outcome::Outcome::Completed(completed)) = outcome.outcome else {
            anyhow::bail!("expected completed execution");
        };
        if suffix.is_empty() {
            assert_eq!(completed.error_text, None);
        } else {
            assert!(
                completed
                    .error_text
                    .context("expected JS failure")?
                    .contains("timed failure")
            );
        }
    }
    Ok(())
}

/// The process-owned host freezes successful and failed execution timing before
/// the client reads its initial response, including JavaScript computation and timers.
#[tokio::test]
async fn stdio_execution_timing_includes_javascript_but_excludes_delayed_reads() -> Result<()> {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?,
    );
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;

    let endings = ["", "throw new Error('timed failure');"];
    for (index, suffix) in endings.into_iter().enumerate() {
        let observed = Instant::now();
        let started = session
            .execute(
                ExecuteRequest {
                    tool_call_id: format!("call-{index}"),
                    source: format!(
                        "const until = Date.now() + 150; while (Date.now() < until) {{}} \
                     await new Promise(resolve => setTimeout(resolve, 150)); \
                     notify('finished'); {suffix}"
                    ),
                    enabled_tools: Vec::new(),
                    yield_time_ms: Some(/*value*/ 5_000),
                    max_output_tokens: Some(/*value*/ 1_000),
                },
                delegate.clone(),
            )
            .await
            .map_err(anyhow::Error::msg)?;
        timeout(TEST_TIMEOUT, delegate.notification_delivered.notified())
            .await
            .context("JS did not report completion of its timed work")?;

        // Delay only after the host has finished the computation and timer.
        sleep(Duration::from_millis(/*millis*/ 600)).await;
        let response = timeout(TEST_TIMEOUT, started.initial_response())
            .await?
            .map_err(anyhow::Error::msg)?;
        let duration = response
            .code_mode_host_duration()
            .context("missing stdio exec timing")?;
        assert!(duration >= Duration::from_millis(/*millis*/ 250));
        assert!(duration + Duration::from_millis(/*millis*/ 300) <= observed.elapsed());
        let RuntimeResponse::Result { error_text, .. } = response else {
            anyhow::bail!("expected completed execution");
        };
        if suffix.is_empty() {
            assert_eq!(error_text, None);
        } else {
            assert!(
                error_text
                    .context("expected JS failure")?
                    .contains("timed failure")
            );
        }
    }
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

/// Every wait and termination measures its own request, including missing-cell outcomes.
/// Earlier execution and background time must not be charged again by later observations.
#[tokio::test]
async fn observation_timing_excludes_previous_requests_and_background_time() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let providers: [Arc<dyn CodeModeSessionProvider>; 2] = [
        Arc::new(ProcessOwnedCodeModeSessionProvider::with_host_program(
            codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?,
        )),
        Arc::new(GrpcCodeModeSessionProvider::new(host.endpoint)),
    ];
    for provider in providers {
        let session = provider
            .create_session()
            .await
            .map_err(anyhow::Error::msg)?;
        let started = session
            .execute(
                ExecuteRequest {
                    tool_call_id: "call-1".to_string(),
                    source: "await new Promise(() => {});".to_string(),
                    enabled_tools: Vec::new(),
                    yield_time_ms: Some(/*value*/ 200),
                    max_output_tokens: Some(/*value*/ 1_000),
                },
                Arc::new(NoopCodeModeSessionDelegate),
            )
            .await
            .map_err(anyhow::Error::msg)?;
        let cell_id = started.cell_id.clone();
        let initial = started
            .initial_response()
            .await
            .map_err(anyhow::Error::msg)?;
        let duration = initial
            .code_mode_host_duration()
            .context("missing exec timing")?;
        assert!(duration >= Duration::from_millis(/*millis*/ 150));
        let yielded = RuntimeResponse::Yielded {
            cell_id: cell_id.clone(),
            content_items: Vec::new(),
            code_mode_host_duration: Some(duration),
        };
        assert_eq!(initial, yielded);

        for _ in 0..2 {
            sleep(Duration::from_millis(/*millis*/ 300)).await;
            let observed = Instant::now();
            let outcome = session
                .wait(WaitRequest {
                    cell_id: cell_id.clone(),
                    yield_time_ms: 50,
                })
                .await
                .map_err(anyhow::Error::msg)?;
            let duration = outcome
                .code_mode_host_duration()
                .context("missing wait timing")?;
            assert!(duration >= Duration::from_millis(/*millis*/ 40));
            assert!(
                duration <= observed.elapsed(),
                "wait included time before its request: {duration:?}"
            );
            assert_eq!(
                outcome,
                WaitOutcome::LiveCell(RuntimeResponse::Yielded {
                    cell_id: cell_id.clone(),
                    content_items: Vec::new(),
                    code_mode_host_duration: Some(duration),
                })
            );
        }

        let observed = Instant::now();
        let terminated = session
            .terminate(cell_id.clone())
            .await
            .map_err(anyhow::Error::msg)?;
        let duration = terminated
            .code_mode_host_duration()
            .context("missing termination timing")?;
        assert!(duration <= observed.elapsed());
        assert_eq!(
            terminated,
            WaitOutcome::LiveCell(RuntimeResponse::Terminated {
                cell_id: cell_id.clone(),
                content_items: Vec::new(),
                code_mode_host_duration: Some(duration),
            })
        );

        let observed = Instant::now();
        let missing = session
            .wait(WaitRequest {
                cell_id: cell_id.clone(),
                yield_time_ms: 50,
            })
            .await
            .map_err(anyhow::Error::msg)?;
        let duration = missing
            .code_mode_host_duration()
            .context("missing absent-cell timing")?;
        assert!(duration <= observed.elapsed());
        assert_eq!(
            missing,
            WaitOutcome::MissingCell(RuntimeResponse::Result {
                error_text: Some(format!("exec cell {cell_id} not found")),
                cell_id,
                content_items: Vec::new(),
                code_mode_host_duration: Some(duration),
            })
        );
        session.shutdown().await.map_err(anyhow::Error::msg)?;
    }
    Ok(())
}
