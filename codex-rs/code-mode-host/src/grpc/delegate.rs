use std::sync::Arc;
use std::sync::Weak;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_code_mode_protocol::grpc as proto;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::session::GrpcSession;

pub(super) struct GrpcDelegate {
    session: Weak<GrpcSession>,
}

impl GrpcDelegate {
    pub(super) fn new(session: Weak<GrpcSession>) -> Self {
        Self { session }
    }
}

impl CodeModeSessionDelegate for GrpcDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            let session = self
                .session
                .upgrade()
                .ok_or_else(|| "code-mode session is closed".to_string())?;
            let _permit = session.delegate_permit()?;
            let execution_id = session
                .execution_id(invocation.cell_id.as_str(), &cancellation)
                .await?;
            let input_json = invocation
                .input
                .as_ref()
                .map(serde_json::to_vec)
                .transpose()
                .map_err(|error| format!("failed to encode code-mode tool input: {error}"))?;
            let invocation_id = Uuid::new_v4();
            let (response, receiver) = oneshot::channel();
            session
                .dispatch_tool(
                    invocation,
                    execution_id,
                    invocation_id,
                    input_json,
                    response,
                    &cancellation,
                )
                .await?;
            let mut pending = PendingCallback::tool(Arc::clone(&session), invocation_id);
            let result = tokio::select! {
                biased;
                result = receiver => result
                    .map_err(|_| "code-mode client closed before returning tool output".to_string())?,
                _ = cancellation.cancelled() => {
                    Err("code mode delegate request cancelled".to_string())
                }
                _ = session.closed.cancelled() => {
                    Err("code-mode session closed before returning tool output".to_string())
                }
            };
            if result.is_ok() {
                pending.disarm();
            }
            result
        })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            let session = self
                .session
                .upgrade()
                .ok_or_else(|| "code-mode session is closed".to_string())?;
            let _permit = session.delegate_permit()?;
            let execution_id = session
                .execution_id(cell_id.as_str(), &cancellation)
                .await?;
            let notification_id = Uuid::new_v4();
            let (response, receiver) = oneshot::channel();
            session
                .begin_notification(
                    notification_id,
                    proto::Notification {
                        notification_id: notification_id.to_string(),
                        execution_id,
                        cell_id: cell_id.to_string(),
                        call_id,
                        text,
                    },
                    response,
                    &cancellation,
                )
                .await?;
            let mut pending = PendingCallback::notification(Arc::clone(&session), notification_id);
            tokio::select! {
                biased;
                result = receiver => {
                    result.map_err(|_| {
                        "code-mode client closed before acknowledging notification".to_string()
                    })?;
                    pending.disarm();
                    Ok(())
                }
                _ = cancellation.cancelled() => {
                    Err("code mode notification cancelled".to_string())
                }
                _ = session.closed.cancelled() => {
                    Err("code-mode session closed before acknowledging notification".to_string())
                }
            }
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        if let Some(session) = self.session.upgrade() {
            session.close_cell(cell_id.as_str());
        }
    }
}

enum CallbackKind {
    Tool,
    Notification,
}

struct PendingCallback {
    session: Arc<GrpcSession>,
    id: Option<Uuid>,
    kind: CallbackKind,
}

impl PendingCallback {
    fn tool(session: Arc<GrpcSession>, id: Uuid) -> Self {
        Self {
            session,
            id: Some(id),
            kind: CallbackKind::Tool,
        }
    }

    fn notification(session: Arc<GrpcSession>, id: Uuid) -> Self {
        Self {
            session,
            id: Some(id),
            kind: CallbackKind::Notification,
        }
    }

    fn disarm(&mut self) {
        self.id = None;
    }
}

impl Drop for PendingCallback {
    fn drop(&mut self) {
        let Some(id) = self.id.take() else {
            return;
        };
        match self.kind {
            CallbackKind::Tool => self.session.cancel_invocation(id),
            CallbackKind::Notification => self.session.cancel_notification(id),
        }
    }
}
