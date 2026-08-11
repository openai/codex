use std::sync::Arc;
use std::sync::PoisonError;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSession;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_code_mode::NotificationFuture;
use codex_code_mode::RuntimeResponse;
use codex_code_mode::ToolInvocationFuture;
use codex_code_mode::WaitOutcome;
use codex_code_mode::WaitRequest;
use pretty_assertions::assert_eq;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[path = "support/host.rs"]
mod host;
#[path = "support/recording_delegate.rs"]
mod recording_delegate;

use host::HostHarness;
use recording_delegate::RecordingDelegate;
use recording_delegate::cell_id;

const TEST_TIMEOUT: Duration = Duration::from_secs(20);

struct BlockingNotificationDelegate {
    started: Semaphore,
    release: Semaphore,
    delivered: Semaphore,
    cancelled: Semaphore,
    closed: Semaphore,
}

impl BlockingNotificationDelegate {
    fn new() -> Self {
        Self {
            started: Semaphore::new(/*permits*/ 0),
            release: Semaphore::new(/*permits*/ 0),
            delivered: Semaphore::new(/*permits*/ 0),
            cancelled: Semaphore::new(/*permits*/ 0),
            closed: Semaphore::new(/*permits*/ 0),
        }
    }
}

impl CodeModeSessionDelegate for BlockingNotificationDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        _cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async { Err("unexpected tool invocation".to_string()) })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        cancellation: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            self.started.add_permits(/*n*/ 1);
            tokio::select! {
                _ = cancellation.cancelled() => {
                    self.cancelled.add_permits(/*n*/ 1);
                    Err("notification cancelled".to_string())
                }
                permit = self.release.acquire() => {
                    permit
                        .map_err(|_| "notification release closed".to_string())?
                        .forget();
                    self.delivered.add_permits(/*n*/ 1);
                    Ok(())
                }
            }
        })
    }

    fn cell_closed(&self, _cell_id: &CellId) {
        self.closed.add_permits(/*n*/ 1);
    }
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

#[tokio::test]
async fn completed_cells_drain_pending_notifications_before_completion() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(BlockingNotificationDelegate::new());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;

    let executing = Arc::clone(&session);
    let completion = tokio::spawn(async move {
        execute(&executing, request(r#"notify("notice"); text("done");"#)).await
    });
    timeout(TEST_TIMEOUT, delegate.started.acquire())
        .await
        .context("notification did not start")??
        .forget();
    assert!(!completion.is_finished());
    delegate.release.add_permits(/*n*/ 1);
    assert_eq!(
        timeout(TEST_TIMEOUT, completion)
            .await
            .context("completed cell did not finish after notification delivery")???,
        text_response("1", "done")
    );
    timeout(TEST_TIMEOUT, delegate.delivered.acquire())
        .await
        .context("completed cell did not deliver its pending notification")??
        .forget();
    assert!(delegate.cancelled.try_acquire().is_err());
    timeout(TEST_TIMEOUT, delegate.closed.acquire())
        .await
        .context("completed cell was not retired")??
        .forget();

    assert_eq!(
        execute(&session, request(r#"text("still alive");"#)).await?,
        text_response("2", "still alive")
    );

    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn completed_waits_drain_pending_notifications_before_returning() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(BlockingNotificationDelegate::new());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request(
        r#"await new Promise(resolve => setTimeout(resolve, 25)); notify("notice"); text("done");"#,
    );
    pending.yield_time_ms = Some(/*value*/ 1);
    let cell = session.execute(pending).await.map_err(anyhow::Error::msg)?;
    assert_eq!(
        cell.initial_response().await.map_err(anyhow::Error::msg)?,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );

    let waiting = Arc::clone(&session);
    let completion = tokio::spawn(async move {
        waiting
            .wait(WaitRequest {
                cell_id: cell_id("1"),
                yield_time_ms: 5_000,
            })
            .await
            .map_err(anyhow::Error::msg)
    });
    timeout(TEST_TIMEOUT, delegate.started.acquire())
        .await
        .context("wait notification did not start")??
        .forget();
    assert!(!completion.is_finished());
    delegate.release.add_permits(/*n*/ 1);
    assert_eq!(
        timeout(TEST_TIMEOUT, completion)
            .await
            .context("wait did not finish after notification delivery")???,
        WaitOutcome::LiveCell(text_response("1", "done"))
    );
    timeout(TEST_TIMEOUT, delegate.delivered.acquire())
        .await
        .context("wait did not deliver its pending notification")??
        .forget();
    assert!(delegate.cancelled.try_acquire().is_err());

    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn termination_cancels_pending_notifications() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(BlockingNotificationDelegate::new());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request(r#"notify("notice"); await new Promise(() => {});"#);
    pending.yield_time_ms = Some(/*value*/ 1);
    let cell = session.execute(pending).await.map_err(anyhow::Error::msg)?;

    timeout(TEST_TIMEOUT, delegate.started.acquire())
        .await
        .context("notification did not start")??
        .forget();
    assert_eq!(
        cell.initial_response().await.map_err(anyhow::Error::msg)?,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
    assert_eq!(
        session
            .terminate(cell_id("1"))
            .await
            .map_err(anyhow::Error::msg)?,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
    timeout(TEST_TIMEOUT, delegate.cancelled.acquire())
        .await
        .context("termination did not cancel notification delivery")??
        .forget();
    timeout(TEST_TIMEOUT, delegate.closed.acquire())
        .await
        .context("terminated cell was not retired")??
        .forget();

    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

#[tokio::test]
async fn oversized_notification_text_is_truncated_at_a_utf8_boundary() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .map_err(anyhow::Error::msg)?;

    assert_eq!(
        execute(
            &session,
            request(r#"notify("🦀".repeat(512)); text("done");"#),
        )
        .await?,
        text_response("1", "done")
    );
    timeout(TEST_TIMEOUT, delegate.notification_delivered.notified())
        .await
        .context("truncated notification was not delivered")?;
    assert_eq!(
        *delegate
            .notifications
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        vec![(
            "call-1".to_string(),
            cell_id("1"),
            format!("{}... [truncated]", "🦀".repeat(252)),
        )]
    );

    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}
