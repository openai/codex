use std::sync::Arc;
use std::sync::PoisonError;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode::CodeModeSession;
use codex_code_mode::CodeModeSessionCellExecutionLimits;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_code_mode::NoopCodeModeSessionDelegate;
use codex_code_mode::RuntimeResponse;
use codex_code_mode::WaitOutcome;
use codex_code_mode::WaitRequest;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::grpc::code_mode_host_client::CodeModeHostClient;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use tokio::time::timeout;
use tonic::Code;

#[path = "support/host.rs"]
mod host;
#[path = "support/recording_delegate.rs"]
mod recording_delegate;

use host::HostHarness;
use recording_delegate::RecordingDelegate;
use recording_delegate::cell_id;

const TEST_TIMEOUT: Duration = Duration::from_secs(20);

fn request(source: &str) -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
        yield_time_ms: Some(/*value*/ 5_000),
        max_output_tokens: Some(/*value*/ 1_000),
    }
}

fn text_response(cell: &str, value: &str) -> RuntimeResponse {
    RuntimeResponse::Result {
        cell_id: cell_id(cell),
        content_items: vec![FunctionCallOutputContentItem::InputText {
            text: value.to_string(),
        }],
        error_text: None,
    }
}

async fn execute(
    session: &Arc<dyn CodeModeSession>,
    request: ExecuteRequest,
) -> Result<RuntimeResponse> {
    timeout(TEST_TIMEOUT, async {
        session
            .execute(request)
            .await
            .map_err(anyhow::Error::msg)?
            .initial_response()
            .await
            .map_err(anyhow::Error::msg)
    })
    .await
    .context("timed out executing gRPC code-mode cell")?
}

async fn start_active_wait(
    session: Arc<dyn CodeModeSession>,
    request: WaitRequest,
) -> Result<tokio::task::JoinHandle<std::result::Result<WaitOutcome, String>>> {
    let (admitted_tx, admitted_rx) = tokio::sync::oneshot::channel();
    let wait = tokio::spawn(async move {
        let mut wait = session.wait(request);
        match wait.as_mut().now_or_never() {
            Some(result) => result,
            None => {
                let _ = admitted_tx.send(());
                wait.await
            }
        }
    });
    timeout(TEST_TIMEOUT, admitted_rx)
        .await
        .context("timed out waiting for observer admission")?
        .context("wait completed before its observer became active")?;
    Ok(wait)
}

#[tokio::test]
async fn tcp_session_persists_values_and_reports_cell_closure() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    assert!(host.endpoint.starts_with("http://127.0.0.1:"));
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;

    assert_eq!(
        execute(&session, request(r#"store("key", "persisted");"#)).await?,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            error_text: None,
        }
    );

    assert_eq!(
        execute(&session, request(r#"text(String(load("key")));"#)).await?,
        text_response("2", "persisted")
    );

    session.shutdown().await.map_err(anyhow::Error::msg)?;
    assert_eq!(
        *delegate
            .closed_cells
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        vec![cell_id("1"), cell_id("2")]
    );
    Ok(())
}

#[tokio::test]
async fn shutdown_immediately_rejects_new_operations() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let session = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .map_err(anyhow::Error::msg)?;

    let shutdown = session.shutdown();
    let expected = "code mode session is shutting down".to_string();
    assert_eq!(
        session.execute(request("text('too late');")).await.err(),
        Some(expected.clone())
    );
    assert_eq!(
        session
            .wait(WaitRequest {
                cell_id: cell_id("missing"),
                yield_time_ms: 1,
            })
            .await,
        Err(expected.clone())
    );
    assert_eq!(session.terminate(cell_id("missing")).await, Err(expected));

    shutdown.await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn cancelling_execution_before_admission_keeps_the_session_usable() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(/*value*/ 1);

    assert!(session.execute(pending).now_or_never().is_none());

    let abandoned_cell = cell_id("1");
    timeout(TEST_TIMEOUT, async {
        loop {
            if delegate
                .closed_cells
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&abandoned_cell)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("cancelled execution was never admitted and cleaned up")?;

    timeout(TEST_TIMEOUT, async {
        loop {
            match session
                .wait(WaitRequest {
                    cell_id: abandoned_cell.clone(),
                    yield_time_ms: 1,
                })
                .await
            {
                Ok(WaitOutcome::MissingCell(_))
                | Ok(WaitOutcome::LiveCell(RuntimeResponse::Terminated { .. })) => break Ok(()),
                Ok(WaitOutcome::LiveCell(RuntimeResponse::Yielded { .. })) => {
                    tokio::task::yield_now().await;
                }
                Ok(outcome) => anyhow::bail!("unexpected abandoned-cell outcome: {outcome:?}"),
                Err(error) if error.contains("already has an active observer") => {
                    tokio::task::yield_now().await;
                }
                Err(error) => break Err(anyhow::Error::msg(error)),
            }
        }
    })
    .await
    .context("cancelled execution leaked its remote cell")??;

    assert_eq!(
        execute(&session, request(r#"text("still alive");"#)).await?,
        text_response("2", "still alive")
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn dropping_a_started_cell_off_runtime_terminates_its_buffered_remote_execution() -> Result<()>
{
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(/*value*/ 1);
    let started = session.execute(pending).await.map_err(anyhow::Error::msg)?;
    let abandoned_cell = started.cell_id.clone();

    timeout(TEST_TIMEOUT, async {
        loop {
            match session
                .wait(WaitRequest {
                    cell_id: abandoned_cell.clone(),
                    yield_time_ms: 1,
                })
                .await
            {
                Ok(WaitOutcome::LiveCell(RuntimeResponse::Yielded { .. })) => break Ok(()),
                Err(error) if error.contains("already has an active observer") => {
                    tokio::task::yield_now().await;
                }
                Ok(outcome) => anyhow::bail!("unexpected execution outcome: {outcome:?}"),
                Err(error) => break Err(anyhow::Error::msg(error)),
            }
        }
    })
    .await
    .context("execution never produced a buffered initial response")??;

    std::thread::spawn(move || drop(started))
        .join()
        .map_err(|_| anyhow::anyhow!("dropping a started cell outside Tokio panicked"))?;

    timeout(TEST_TIMEOUT, async {
        loop {
            match session
                .wait(WaitRequest {
                    cell_id: abandoned_cell.clone(),
                    yield_time_ms: 1,
                })
                .await
            {
                Ok(WaitOutcome::MissingCell(_))
                | Ok(WaitOutcome::LiveCell(RuntimeResponse::Terminated { .. })) => break Ok(()),
                Ok(WaitOutcome::LiveCell(RuntimeResponse::Yielded { .. })) => {
                    tokio::task::yield_now().await;
                }
                Ok(outcome) => anyhow::bail!("unexpected abandoned-cell outcome: {outcome:?}"),
                Err(error) if error.contains("already has an active observer") => {
                    tokio::task::yield_now().await;
                }
                Err(error) => break Err(anyhow::Error::msg(error)),
            }
        }
    })
    .await
    .context("dropping a started cell did not terminate its buffered remote execution")??;

    assert!(
        delegate
            .closed_cells
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&abandoned_cell)
    );

    assert_eq!(
        execute(&session, request(r#"text("still alive");"#)).await?,
        text_response("2", "still alive")
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn dropping_an_initial_response_terminates_its_pending_remote_execution() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(/*value*/ 60_000);
    let started = session.execute(pending).await.map_err(anyhow::Error::msg)?;
    let abandoned_cell = started.cell_id.clone();
    let initial_response = tokio::spawn(started.initial_response());
    tokio::task::yield_now().await;
    initial_response.abort();
    let _ = initial_response.await;

    timeout(TEST_TIMEOUT, async {
        loop {
            if delegate
                .closed_cells
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&abandoned_cell)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("dropping an initial response did not terminate its pending remote execution")?;

    assert_eq!(
        execute(&session, request(r#"text("still alive");"#)).await?,
        text_response("2", "still alive")
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn concurrent_wait_rejects_without_displacing_the_active_observer() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let session = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(/*value*/ 1);
    let started = session.execute(pending).await.map_err(anyhow::Error::msg)?;
    let running_cell = started.cell_id.clone();
    assert_eq!(
        started
            .initial_response()
            .await
            .map_err(anyhow::Error::msg)?,
        RuntimeResponse::Yielded {
            cell_id: running_cell.clone(),
            content_items: Vec::new(),
        }
    );

    let first_wait = start_active_wait(
        Arc::clone(&session),
        WaitRequest {
            cell_id: running_cell.clone(),
            yield_time_ms: 100,
        },
    )
    .await?;

    assert_eq!(
        timeout(
            Duration::from_secs(/*secs*/ 1),
            session.wait(WaitRequest {
                cell_id: running_cell.clone(),
                yield_time_ms: 60_000,
            }),
        )
        .await
        .context("concurrent wait did not reject immediately")?
        .unwrap_err(),
        format!("exec cell {running_cell} already has an active observer")
    );

    assert_eq!(
        timeout(TEST_TIMEOUT, first_wait)
            .await
            .context("active wait was displaced by the rejected observer")??
            .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: running_cell.clone(),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        session
            .terminate(running_cell.clone())
            .await
            .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: running_cell,
            content_items: Vec::new(),
        })
    );

    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn dropping_a_wait_retires_its_observer_before_the_next_wait() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let session = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(/*value*/ 1);
    let started = session.execute(pending).await.map_err(anyhow::Error::msg)?;
    let running_cell = started.cell_id.clone();
    assert_eq!(
        started
            .initial_response()
            .await
            .map_err(anyhow::Error::msg)?,
        RuntimeResponse::Yielded {
            cell_id: running_cell.clone(),
            content_items: Vec::new(),
        }
    );

    let first_wait = start_active_wait(
        Arc::clone(&session),
        WaitRequest {
            cell_id: running_cell.clone(),
            yield_time_ms: 60_000,
        },
    )
    .await?;
    first_wait.abort();
    let _ = first_wait.await;

    assert_eq!(
        timeout(
            TEST_TIMEOUT,
            session.wait(WaitRequest {
                cell_id: running_cell.clone(),
                yield_time_ms: 1,
            }),
        )
        .await
        .context("replacement wait did not observe cancellation retirement")?
        .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: running_cell.clone(),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        session
            .terminate(running_cell.clone())
            .await
            .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: running_cell,
            content_items: Vec::new(),
        })
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn dropping_a_session_off_runtime_retires_its_active_cells() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(/*value*/ 1);
    assert_eq!(
        execute(&session, pending).await?,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );

    std::thread::spawn(move || drop(session))
        .join()
        .map_err(|_| anyhow::anyhow!("dropping a session outside Tokio panicked"))?;

    timeout(TEST_TIMEOUT, async {
        loop {
            if delegate
                .closed_cells
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&cell_id("1"))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("dropping a session outside Tokio did not retire its active cell")?;

    Ok(())
}

#[tokio::test]
async fn sessions_enforce_independent_yield_limits() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let limited = provider
        .create_session_with_limits(
            Arc::new(NoopCodeModeSessionDelegate),
            CodeModeSessionCellExecutionLimits {
                max_yield_time_ms: Some(/*value*/ 1),
                max_heap_size_bytes: Some(/*value*/ 16 * 1024 * 1024),
            },
        )
        .await
        .map_err(anyhow::Error::msg)?;
    let other = provider
        .create_session_with_limits(
            Arc::new(NoopCodeModeSessionDelegate),
            CodeModeSessionCellExecutionLimits {
                max_yield_time_ms: Some(/*value*/ 1_000),
                max_heap_size_bytes: None,
            },
        )
        .await
        .map_err(anyhow::Error::msg)?;

    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(/*value*/ 60_000);
    assert_eq!(
        execute(&limited, pending).await?,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
    assert_eq!(
        timeout(
            TEST_TIMEOUT,
            limited.wait(WaitRequest {
                cell_id: cell_id("1"),
                yield_time_ms: 60_000,
            }),
        )
        .await
        .context("session yield limit did not bound an explicit wait")?
        .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        execute(
            &other,
            request(r#"await new Promise(resolve => setTimeout(resolve, 25)); text("isolated");"#),
        )
        .await?,
        text_response("1", "isolated")
    );
    assert_eq!(
        limited
            .terminate(cell_id("1"))
            .await
            .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        execute(&limited, request(r#"text("recovered");"#)).await?,
        text_response("2", "recovered")
    );
    limited.shutdown().await.map_err(anyhow::Error::msg)?;
    other.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn dropping_a_grpc_lease_retires_its_server_session() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let mut client = CodeModeHostClient::connect(host.endpoint)
        .await
        .context("connect raw gRPC client")?;
    let mut lease = client
        .open_session(grpc::OpenSessionRequest {
            cell_execution_limits: None,
        })
        .await
        .context("open raw gRPC session")?
        .into_inner();
    let first = lease
        .message()
        .await
        .context("read raw gRPC session opening")?
        .context("raw gRPC session ended before its opening event")?;
    let Some(grpc::session_event::Event::Opened(opened)) = first.event else {
        anyhow::bail!("raw gRPC session did not start with an opening event");
    };
    drop(lease);

    timeout(TEST_TIMEOUT, async {
        loop {
            match client
                .subscribe_to_tool_calls(grpc::SubscribeToToolCallsRequest {
                    session_id: opened.session_id.clone(),
                    tool_names: Vec::new(),
                })
                .await
            {
                Ok(response) => drop(response),
                Err(status) if status.code() == Code::NotFound => return Ok::<_, anyhow::Error>(()),
                Err(status) => {
                    anyhow::bail!("unexpected session status after lease drop: {status}")
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("dropping the gRPC lease did not retire its server session")??;
    Ok(())
}
