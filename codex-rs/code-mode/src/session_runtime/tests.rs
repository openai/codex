use std::sync::Arc;
use std::time::Duration;

use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::*;

struct BlockingCloseDelegate {
    close_started: Notify,
    close_release: Notify,
}

impl BlockingCloseDelegate {
    fn release_close(&self) {
        self.close_release.notify_one();
    }
}

impl SessionRuntimeDelegate for BlockingCloseDelegate {
    async fn invoke_tool(
        &self,
        _invocation: NestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        Ok(JsonValue::Null)
    }

    async fn notify(
        &self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn cell_closed(&self, _cell_id: &CellId) -> Result<(), String> {
        self.close_started.notify_one();
        self.close_release.notified().await;
        Ok(())
    }
}

fn execute_request() -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: r#"text("done");"#.to_string(),
    }
}

#[tokio::test]
async fn closing_cells_reject_requests_while_delegate_cleanup_runs() {
    let delegate = Arc::new(BlockingCloseDelegate {
        close_started: Notify::new(),
        close_release: Notify::new(),
    });
    let runtime = Arc::new(SessionRuntime::new(Arc::clone(&delegate)));
    let close_started = delegate.close_started.notified();
    let started = runtime
        .execute(
            execute_request(),
            ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        )
        .await
        .unwrap();
    let cell_id = started.cell_id.clone();

    assert_eq!(
        started.initial_event().await.unwrap(),
        CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        }
    );
    close_started.await;

    assert_eq!(
        runtime
            .observe(&cell_id, ObserveMode::PendingFrontier)
            .await,
        Err(Error::MissingCell(cell_id.clone()))
    );
    assert_eq!(
        runtime.terminate(&cell_id).await,
        Err(Error::MissingCell(cell_id.clone()))
    );

    let shutdown_runtime = Arc::clone(&runtime);
    let mut shutdown = tokio::spawn(async move { shutdown_runtime.shutdown().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut shutdown)
            .await
            .is_err()
    );

    delegate.release_close();

    assert_eq!(shutdown.await.unwrap(), Ok(()));
}
