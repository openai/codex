use std::sync::Arc;

use codex_code_mode::SessionRuntime;
use codex_code_mode::session_runtime as runtime;
use codex_code_mode_protocol::wire::CallbackRequest;
use codex_code_mode_protocol::wire::CallbackResponse;
use codex_code_mode_protocol::wire::HostMessage;
use codex_code_mode_protocol::wire::SessionId;
use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

use crate::convert;
use crate::peer::HostPeer;

#[derive(Clone)]
pub(super) struct HostSession {
    runtime: Arc<SessionRuntime<RemoteDelegate>>,
}

impl HostSession {
    pub(super) fn new(session_id: SessionId, peer: Arc<HostPeer>) -> Self {
        let delegate = Arc::new(RemoteDelegate { session_id, peer });
        Self {
            runtime: Arc::new(SessionRuntime::new(delegate)),
        }
    }

    pub(super) async fn create_cell(
        &self,
        request: runtime::CreateCellRequest,
    ) -> Result<runtime::CellId, runtime::Error> {
        self.runtime.create_cell(request).await
    }

    pub(super) async fn observe(
        &self,
        cell_id: &runtime::CellId,
        mode: runtime::ObserveMode,
    ) -> Result<runtime::CellEvent, runtime::Error> {
        self.runtime.observe(cell_id, mode).await
    }

    pub(super) async fn terminate(
        &self,
        cell_id: &runtime::CellId,
    ) -> Result<runtime::CellEvent, runtime::Error> {
        self.runtime.terminate(cell_id).await
    }

    pub(super) async fn shutdown(&self) -> Result<(), runtime::Error> {
        self.runtime.shutdown().await
    }
}

struct RemoteDelegate {
    session_id: SessionId,
    peer: Arc<HostPeer>,
}

impl runtime::SessionRuntimeDelegate for RemoteDelegate {
    async fn invoke_tool(
        &self,
        invocation: runtime::NestedToolCall,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        match self
            .peer
            .call(
                self.session_id,
                CallbackRequest::InvokeTool {
                    invocation: convert::nested_tool_call(invocation),
                },
                cancellation_token,
            )
            .await?
        {
            CallbackResponse::ToolResult { result } => Ok(result),
            CallbackResponse::ToolError { error_text } => Err(error_text),
            CallbackResponse::NotificationDelivered
            | CallbackResponse::NotificationError { .. } => {
                Err("code-mode client returned an invalid tool response".to_string())
            }
        }
    }

    async fn notify(
        &self,
        call_id: String,
        cell_id: runtime::CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        match self
            .peer
            .call(
                self.session_id,
                CallbackRequest::Notify {
                    call_id,
                    cell_id: convert::wire_cell_id(&cell_id),
                    text,
                },
                cancellation_token,
            )
            .await?
        {
            CallbackResponse::NotificationDelivered => Ok(()),
            CallbackResponse::NotificationError { error_text } => Err(error_text),
            CallbackResponse::ToolResult { .. } | CallbackResponse::ToolError { .. } => {
                Err("code-mode client returned an invalid notification response".to_string())
            }
        }
    }

    fn cell_closed(&self, cell_id: &runtime::CellId) {
        self.peer.send_nowait(HostMessage::CellClosed {
            session_id: self.session_id,
            cell_id: convert::wire_cell_id(cell_id),
        });
    }
}
