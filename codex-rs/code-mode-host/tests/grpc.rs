use std::sync::Arc;
use std::sync::PoisonError;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSession;
use codex_code_mode::CodeModeSessionCellExecutionLimits;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_code_mode::NoopCodeModeSessionDelegate;
use codex_code_mode::NotificationFuture;
use codex_code_mode::RuntimeResponse;
use codex_code_mode::ToolDefinition;
use codex_code_mode::ToolInvocationFuture;
use codex_code_mode::WaitOutcome;
use codex_code_mode::WaitRequest;
#[cfg(unix)]
use codex_code_mode_host::GrpcCodeModeHost;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::grpc::code_mode_host_client::CodeModeHostClient;
#[cfg(unix)]
use codex_code_mode_protocol::grpc::code_mode_host_server::CodeModeHostServer;
use codex_protocol::ToolName;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use serde_json::json;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::Semaphore;
use tokio::time::timeout;
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::Code;
#[cfg(unix)]
use tonic::transport::Server;

#[path = "support/host.rs"]
mod host;
#[path = "support/large_tool_delegate.rs"]
mod large_tool_delegate;
#[path = "support/recording_delegate.rs"]
mod recording_delegate;

use host::HostHarness;
use large_tool_delegate::LargeToolResultDelegate;
use recording_delegate::RecordingDelegate;
use recording_delegate::cell_id;

const TEST_TIMEOUT: Duration = Duration::from_secs(20);

struct PanickingDelegate;

struct SelfCancellingToolDelegate;

impl CodeModeSessionDelegate for PanickingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        _cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        panic!("synchronous tool delegate panic")
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation: CancellationToken,
    ) -> NotificationFuture<'a> {
        panic!("synchronous notification delegate panic")
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

impl CodeModeSessionDelegate for SelfCancellingToolDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        cancellation.cancel();
        Box::pin(async { Err("tool delegate cancelled itself".to_string()) })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

fn request(source: &str) -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
        yield_time_ms: Some(/*value*/ 5_000),
        max_output_tokens: Some(/*value*/ 1_000),
    }
}

fn tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        tool_name: ToolName::plain(name),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
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
async fn grpc_endpoints_reject_credentials_without_disclosing_them() {
    for endpoint in [
        "http://alice:secret@host.example",
        "https://alice:secret@host.example",
        "https://alice@host.example",
        "https://:secret@host.example",
    ] {
        let provider = GrpcCodeModeSessionProvider::new(endpoint);
        let error = provider
            .create_session(Arc::new(NoopCodeModeSessionDelegate))
            .await
            .err()
            .expect("gRPC credentials should be rejected");

        assert!(error.contains("must not include credentials"));
        assert!(!error.contains("alice"));
        assert!(!error.contains("secret"));
    }
}

#[tokio::test]
async fn tcp_session_persists_values_and_forwards_tools_notifications_and_closure() -> Result<()> {
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

    let mut callback = request(
        r#"const result = await tools.echo({ value: String(load("key")) }); notify("notice"); text(result.value);"#,
    );
    callback.tool_call_id = "call-2".to_string();
    callback.enabled_tools = vec![tool("echo")];
    assert_eq!(
        execute(&session, callback).await?,
        text_response("2", "output")
    );
    timeout(TEST_TIMEOUT, delegate.notification_delivered.notified())
        .await
        .context("notification was not delivered")?;
    assert_eq!(
        *delegate
            .invocations
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        vec![CodeModeNestedToolCall {
            cell_id: cell_id("2"),
            runtime_tool_call_id: "tool-1".to_string(),
            tool_name: ToolName::plain("echo"),
            tool_kind: CodeModeToolKind::Function,
            input: Some(json!({ "value": "persisted" })),
        }]
    );
    assert_eq!(
        *delegate
            .notifications
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        vec![("call-2".to_string(), cell_id("2"), "notice".to_string())]
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
async fn synchronous_delegate_panics_do_not_orphan_callbacks_or_close_the_session() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let session = provider
        .create_session(Arc::new(PanickingDelegate))
        .await
        .map_err(anyhow::Error::msg)?;

    let mut tool_panic = request(
        r#"try { await tools.echo({}); text("unexpected"); } catch (_) { text("tool recovered"); }"#,
    );
    tool_panic.enabled_tools = vec![tool("echo")];
    assert_eq!(
        execute(&session, tool_panic).await?,
        text_response("1", "tool recovered")
    );

    assert_eq!(
        execute(
            &session,
            request(r#"notify("panic"); text("notification recovered");"#),
        )
        .await?,
        text_response("2", "notification recovered")
    );
    assert_eq!(
        execute(&session, request(r#"text("still alive");"#)).await?,
        text_response("3", "still alive")
    );

    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn tool_delegate_self_cancellation_returns_an_error_without_hanging() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let session = provider
        .create_session(Arc::new(SelfCancellingToolDelegate))
        .await
        .map_err(anyhow::Error::msg)?;

    let mut callback =
        request(r#"try { await tools.echo({}); } catch (_) { text("tool recovered"); }"#);
    callback.enabled_tools = vec![tool("echo")];
    assert_eq!(
        execute(&session, callback).await?,
        text_response("1", "tool recovered")
    );
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
async fn large_unary_tool_completion_does_not_block_an_independent_session() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(LargeToolResultDelegate {
        started: Semaphore::new(/*permits*/ 0),
        release: Semaphore::new(/*permits*/ 0),
    });
    let slow_session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let fast_session = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .map_err(anyhow::Error::msg)?;
    let mut slow_request = request(
        r#"const result = await tools.large({ value: "request" }); text(String(result.value.length));"#,
    );
    slow_request.enabled_tools = vec![tool("large")];
    slow_request.yield_time_ms = Some(/*value*/ 20_000);
    let slow_cell = slow_session
        .execute(slow_request)
        .await
        .map_err(anyhow::Error::msg)?;
    timeout(TEST_TIMEOUT, delegate.started.acquire())
        .await
        .context("large tool callback did not start")??
        .forget();

    assert_eq!(
        execute(&fast_session, request(r#"text("fast-before");"#)).await?,
        text_response("1", "fast-before")
    );

    delegate.release.add_permits(/*n*/ 1);
    let slow_response = timeout(TEST_TIMEOUT, slow_cell.initial_response());
    let fast_response = execute(&fast_session, request(r#"text("fast-during");"#));
    let (slow_response, fast_response) = tokio::join!(slow_response, fast_response);

    assert_eq!(fast_response?, text_response("2", "fast-during"));
    assert_eq!(
        slow_response
            .context("large unary tool response did not complete")?
            .map_err(anyhow::Error::msg)?,
        text_response("1", "8388608")
    );
    slow_session.shutdown().await.map_err(anyhow::Error::msg)?;
    fast_session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn single_subscription_processes_slow_and_fast_tools_concurrently() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(LargeToolResultDelegate {
        started: Semaphore::new(/*permits*/ 0),
        release: Semaphore::new(/*permits*/ 0),
    });
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;

    let mut slow =
        request(r#"const result = await tools.large({}); text(String(result.value.length));"#);
    slow.enabled_tools = vec![tool("large")];
    slow.yield_time_ms = Some(/*value*/ 20_000);
    let slow_cell = session.execute(slow).await.map_err(anyhow::Error::msg)?;
    timeout(TEST_TIMEOUT, delegate.started.acquire())
        .await
        .context("slow tool did not start")??
        .forget();

    let mut fast = request(r#"const result = await tools.fast({}); text(result.value);"#);
    fast.enabled_tools = vec![tool("fast")];
    assert_eq!(
        execute(&session, fast).await?,
        text_response("2", "isolated")
    );

    delegate.release.add_permits(/*n*/ 1);
    assert_eq!(
        timeout(TEST_TIMEOUT, slow_cell.initial_response())
            .await
            .context("slow tool did not finish")?
            .map_err(anyhow::Error::msg)?,
        text_response("1", "8388608")
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
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
        execute(&limited, request("await new Promise(() => {});")).await?,
        RuntimeResponse::Yielded {
            cell_id: cell_id("2"),
            content_items: Vec::new(),
        }
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

#[tokio::test]
async fn cached_session_recovers_after_a_remote_host_restarts() -> Result<()> {
    let mut original = HostHarness::start("grpc://127.0.0.1:0").await?;
    let listen_url = original
        .endpoint
        .replacen("http://", "grpc://", /*count*/ 1);
    let provider = GrpcCodeModeSessionProvider::new(original.endpoint.clone());
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;

    let mut pending = request("await tools.echo({generation: 1}); await new Promise(() => {});");
    pending.enabled_tools = vec![tool("echo")];
    pending.yield_time_ms = Some(/*value*/ 1);
    let started = session.execute(pending).await.map_err(anyhow::Error::msg)?;
    let old_cell_id = started.cell_id.clone();
    assert_eq!(old_cell_id, cell_id("1"));
    assert!(matches!(
        started.initial_response().await,
        Ok(RuntimeResponse::Yielded { .. })
    ));

    let interrupted_wait = start_active_wait(
        Arc::clone(&session),
        WaitRequest {
            cell_id: old_cell_id.clone(),
            yield_time_ms: 60_000,
        },
    )
    .await?;
    timeout(TEST_TIMEOUT, async {
        while delegate
            .invocations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("original host did not dispatch its tool callback")?;
    original
        ._child
        .kill()
        .await
        .context("stop the original gRPC host")?;
    assert!(
        timeout(TEST_TIMEOUT, interrupted_wait)
            .await
            .context("host loss did not interrupt the pending wait")?
            .context("interrupted wait task panicked")?
            .is_err()
    );
    timeout(TEST_TIMEOUT, async {
        loop {
            if delegate
                .closed_cells
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&old_cell_id)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("host loss did not retire the original generation's cell")?;

    let _replacement = HostHarness::start(&listen_url).await?;
    let mut callback = request(
        r#"const result = await tools.echo({generation: 2}); notify("reconnected"); text(result.value);"#,
    );
    callback.tool_call_id = "reconnected-call".to_string();
    callback.enabled_tools = vec![tool("echo")];
    let (callback_response, concurrent_response) = tokio::join!(
        execute(&session, callback),
        execute(&session, request(r#"text("concurrent")"#)),
    );
    let callback_response = callback_response?;
    let RuntimeResponse::Result {
        cell_id: callback_cell_id,
        ..
    } = &callback_response
    else {
        anyhow::bail!("reconnected tool call did not complete");
    };
    let callback_cell_id = callback_cell_id.clone();
    assert_eq!(
        callback_response,
        text_response(callback_cell_id.as_str(), "output")
    );
    let concurrent_response = concurrent_response?;
    let RuntimeResponse::Result {
        cell_id: concurrent_cell_id,
        ..
    } = &concurrent_response
    else {
        anyhow::bail!("concurrent reconnected cell did not complete");
    };
    let concurrent_cell_id = concurrent_cell_id.clone();
    assert_eq!(
        concurrent_response,
        text_response(concurrent_cell_id.as_str(), "concurrent")
    );
    let mut replacement_cell_ids = [callback_cell_id.as_str(), concurrent_cell_id.as_str()];
    replacement_cell_ids.sort_unstable();
    assert_eq!(replacement_cell_ids, ["g2:1", "g2:2"]);
    assert_eq!(
        delegate
            .invocations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|invocation| (invocation.cell_id.clone(), invocation.input.clone()))
            .collect::<Vec<_>>(),
        vec![
            (old_cell_id.clone(), Some(json!({ "generation": 1 }))),
            (callback_cell_id.clone(), Some(json!({ "generation": 2 }))),
        ]
    );
    assert_eq!(
        *delegate
            .notifications
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        vec![(
            "reconnected-call".to_string(),
            callback_cell_id,
            "reconnected".to_string(),
        )]
    );

    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(/*value*/ 1);
    let started = session.execute(pending).await.map_err(anyhow::Error::msg)?;
    let replacement_cell_id = started.cell_id.clone();
    assert_eq!(replacement_cell_id, cell_id("g2:3"));
    assert_eq!(
        started.initial_response().await,
        Ok(RuntimeResponse::Yielded {
            cell_id: replacement_cell_id.clone(),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        session
            .wait(WaitRequest {
                cell_id: replacement_cell_id.clone(),
                yield_time_ms: 1,
            })
            .await
            .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: replacement_cell_id.clone(),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        session
            .terminate(replacement_cell_id.clone())
            .await
            .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: replacement_cell_id,
            content_items: Vec::new(),
        })
    );

    let stale_wait = session
        .wait(WaitRequest {
            cell_id: old_cell_id.clone(),
            yield_time_ms: 1,
        })
        .await
        .unwrap_err();
    assert!(stale_wait.contains("stale code-mode host generation"));
    let stale_termination = session.terminate(old_cell_id).await.unwrap_err();
    assert!(stale_termination.contains("stale code-mode host generation"));
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn unix_socket_endpoints_execute_code_mode_cells() -> Result<()> {
    let directory = tempfile::tempdir().context("create Unix socket directory")?;
    let socket_path = directory.path().join("grpc.sock");
    let listener = UnixListener::bind(&socket_path).context("bind code-mode Unix socket")?;
    let server = tokio::spawn(
        Server::builder()
            .add_service(CodeModeHostServer::new(GrpcCodeModeHost::new()))
            .serve_with_incoming(UnixListenerStream::new(listener)),
    );

    for endpoint in [
        format!("unix://{}", socket_path.display()),
        format!("unix:{}", socket_path.display()),
    ] {
        let session = GrpcCodeModeSessionProvider::new(endpoint)
            .create_session(Arc::new(NoopCodeModeSessionDelegate))
            .await
            .map_err(anyhow::Error::msg)?;
        assert_eq!(
            execute(&session, request(r#"text("unix socket")"#)).await?,
            text_response("1", "unix socket")
        );
        session.shutdown().await.map_err(anyhow::Error::msg)?;
    }

    server.abort();
    Ok(())
}
