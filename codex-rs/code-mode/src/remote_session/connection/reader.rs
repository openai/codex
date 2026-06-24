use std::sync::Arc;
use std::sync::Weak;

use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::DelegateRequest;
use codex_code_mode_protocol::host::DelegateResponse;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::WireResult;
use tokio::process::ChildStdout;
use tokio_util::sync::CancellationToken;

use super::ConnectionState;

pub(super) async fn drive_reader(
    mut reader: FramedReader<ChildStdout>,
    state: Weak<ConnectionState>,
    cancellation: CancellationToken,
) {
    loop {
        let message = tokio::select! {
            _ = cancellation.cancelled() => return,
            result = reader.read::<HostToClient>() => result,
        };
        match message {
            Ok(Some(message)) => {
                let Some(state) = state.upgrade() else {
                    return;
                };
                handle_host_message(state, message).await;
            }
            Ok(None) => {
                if let Some(state) = state.upgrade() {
                    state
                        .fail("code-mode host closed its stdout".to_string())
                        .await;
                }
                return;
            }
            Err(err) => {
                if let Some(state) = state.upgrade() {
                    state
                        .fail(format!("failed to read code-mode host message: {err}"))
                        .await;
                }
                return;
            }
        }
    }
}

async fn handle_host_message(state: Arc<ConnectionState>, message: HostToClient) {
    match message {
        HostToClient::Response { id, result } => {
            if let Some(sender) = state.pending.lock().await.remove(&id) {
                let _ = sender.send(result.into_result());
            }
        }
        HostToClient::InitialResponse { id, result } => {
            if let Some(sender) = state.initial_responses.lock().await.remove(&id) {
                let _ = sender.send(result.into_result().map(Into::into));
            }
        }
        HostToClient::DelegateRequest {
            id,
            session_id,
            request,
        } => {
            let delegate = state.delegates.lock().await.get(&session_id).cloned();
            let Some(delegate) = delegate else {
                let _ = state
                    .send(ClientToHost::DelegateResponse {
                        id,
                        result: WireResult::Err {
                            message: format!("unknown code-mode session {session_id}"),
                        },
                    })
                    .await;
                return;
            };
            let cancellation = state.cancellation.child_token();
            state
                .delegate_cancellations
                .lock()
                .await
                .insert(id, cancellation.clone());
            tokio::spawn(async move {
                let result = match request {
                    DelegateRequest::InvokeTool { invocation } => delegate
                        .invoke_tool(invocation.into(), cancellation)
                        .await
                        .map(|result| DelegateResponse::ToolResult { result }),
                    DelegateRequest::Notify {
                        call_id,
                        cell_id,
                        text,
                    } => delegate
                        .notify(call_id, cell_id.into(), text, cancellation)
                        .await
                        .map(|()| DelegateResponse::NotificationDelivered),
                };
                state.delegate_cancellations.lock().await.remove(&id);
                let _ = state
                    .send(ClientToHost::DelegateResponse {
                        id,
                        result: WireResult::from_result(result),
                    })
                    .await;
            });
        }
        HostToClient::CancelDelegateRequest { id } => {
            if let Some(cancellation) = state.delegate_cancellations.lock().await.remove(&id) {
                cancellation.cancel();
            }
        }
        HostToClient::CellClosed {
            session_id,
            cell_id,
        } => {
            if let Some(delegate) = state.delegates.lock().await.get(&session_id).cloned() {
                let cell_id = cell_id.into();
                delegate.cell_closed(&cell_id);
            }
        }
        HostToClient::HostHello(_) | HostToClient::HandshakeRejected { .. } => {
            state
                .fail("code-mode host sent a second handshake response".to_string())
                .await;
        }
    }
}

#[cfg(test)]
#[path = "reader_tests.rs"]
mod tests;
