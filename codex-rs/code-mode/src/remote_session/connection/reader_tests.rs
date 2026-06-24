use std::sync::Mutex as StdMutex;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_code_mode_protocol::host::DelegateRequestId;
use codex_code_mode_protocol::host::SessionId;
use pretty_assertions::assert_eq;
use tokio::sync::oneshot;

use super::*;

struct CancellationRecordingDelegate {
    cancellation_tx: StdMutex<Option<oneshot::Sender<()>>>,
}

impl CodeModeSessionDelegate for CancellationRecordingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async { Err("unexpected tool invocation".to_string()) })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        let cancellation_tx = self
            .cancellation_tx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        Box::pin(async move {
            cancellation_token.cancelled().await;
            if let Some(cancellation_tx) = cancellation_tx {
                let _ = cancellation_tx.send(());
            }
            Ok(())
        })
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

#[tokio::test]
async fn delegate_started_after_connection_cancellation_is_cancelled() {
    let (outgoing_tx, mut outgoing_rx) = tokio::sync::mpsc::channel(/*max_capacity*/ 1);
    let connection_cancellation = CancellationToken::new();
    connection_cancellation.cancel();
    let state = Arc::new(ConnectionState::new(outgoing_tx, connection_cancellation));
    let session_id = SessionId::new("session-1").expect("session ID");
    let delegate_request_id = DelegateRequestId::new(7);
    let (cancellation_tx, cancellation_rx) = oneshot::channel();
    state.delegates.lock().await.insert(
        session_id.clone(),
        Arc::new(CancellationRecordingDelegate {
            cancellation_tx: StdMutex::new(Some(cancellation_tx)),
        }),
    );

    handle_host_message(
        Arc::clone(&state),
        HostToClient::DelegateRequest {
            id: delegate_request_id,
            session_id,
            request: DelegateRequest::Notify {
                call_id: "call-1".to_string(),
                cell_id: CellId::new("cell-1".to_string()).into(),
                text: "hello".to_string(),
            },
        },
    )
    .await;

    tokio::time::timeout(Duration::from_secs(1), cancellation_rx)
        .await
        .expect("delegate cancellation timeout")
        .expect("delegate cancellation signal");
    assert_eq!(
        outgoing_rx.recv().await,
        Some(ClientToHost::DelegateResponse {
            id: delegate_request_id,
            result: WireResult::Ok {
                value: DelegateResponse::NotificationDelivered,
            },
        })
    );
    assert!(state.delegate_cancellations.lock().await.is_empty());
}
