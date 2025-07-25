use std::collections::HashMap;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCError;
use mcp_types::JSONRPCErrorError;
use mcp_types::JSONRPCMessage;
use mcp_types::JSONRPCNotification;
use mcp_types::JSONRPCRequest;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use mcp_types::Result;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::warn;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EventMethod {
    ExecApprovalRequest,
    Error,
    ApplyPatchApprovalRequest,
    TaskComplete,
    SessionConfigured,
    AgentMessageDelta,
    AgentReasoningDelta,
    AgentMessage,
    TaskStarted,
    TokenCount,
    AgentReasoning,
    McpToolCallBegin,
    McpToolCallEnd,
    ExecCommandBegin,
    ExecCommandEnd,
    BackgroundEvent,
    PatchApplyBegin,
    PatchApplyEnd,
    GetHistoryEntryResponse,
    ShutdownComplete,
}

impl EventMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventMethod::ExecApprovalRequest => "ExecApprovalRequest",
            EventMethod::Error => "Error",
            EventMethod::ApplyPatchApprovalRequest => "ApplyPatchApprovalRequest",
            EventMethod::TaskComplete => "TaskComplete",
            EventMethod::SessionConfigured => "SessionConfigured",
            EventMethod::AgentMessageDelta => "AgentMessageDelta",
            EventMethod::AgentReasoningDelta => "AgentReasoningDelta",
            EventMethod::AgentMessage => "AgentMessage",
            EventMethod::TaskStarted => "TaskStarted",
            EventMethod::TokenCount => "TokenCount",
            EventMethod::AgentReasoning => "AgentReasoning",
            EventMethod::McpToolCallBegin => "McpToolCallBegin",
            EventMethod::McpToolCallEnd => "McpToolCallEnd",
            EventMethod::ExecCommandBegin => "ExecCommandBegin",
            EventMethod::ExecCommandEnd => "ExecCommandEnd",
            EventMethod::BackgroundEvent => "BackgroundEvent",
            EventMethod::PatchApplyBegin => "PatchApplyBegin",
            EventMethod::PatchApplyEnd => "PatchApplyEnd",
            EventMethod::GetHistoryEntryResponse => "GetHistoryEntryResponse",
            EventMethod::ShutdownComplete => "ShutdownComplete",
        }
    }
}

impl From<&EventMsg> for EventMethod {
    fn from(msg: &EventMsg) -> Self {
        match msg {
            EventMsg::ExecApprovalRequest(_) => EventMethod::ExecApprovalRequest,
            EventMsg::Error(_) => EventMethod::Error,
            EventMsg::ApplyPatchApprovalRequest(_) => EventMethod::ApplyPatchApprovalRequest,
            EventMsg::TaskComplete(_) => EventMethod::TaskComplete,
            EventMsg::SessionConfigured(_) => EventMethod::SessionConfigured,
            EventMsg::AgentMessageDelta(_) => EventMethod::AgentMessageDelta,
            EventMsg::AgentReasoningDelta(_) => EventMethod::AgentReasoningDelta,
            EventMsg::AgentMessage(_) => EventMethod::AgentMessage,
            EventMsg::TaskStarted => EventMethod::TaskStarted,
            EventMsg::TokenCount(_) => EventMethod::TokenCount,
            EventMsg::AgentReasoning(_) => EventMethod::AgentReasoning,
            EventMsg::McpToolCallBegin(_) => EventMethod::McpToolCallBegin,
            EventMsg::McpToolCallEnd(_) => EventMethod::McpToolCallEnd,
            EventMsg::ExecCommandBegin(_) => EventMethod::ExecCommandBegin,
            EventMsg::ExecCommandEnd(_) => EventMethod::ExecCommandEnd,
            EventMsg::BackgroundEvent(_) => EventMethod::BackgroundEvent,
            EventMsg::PatchApplyBegin(_) => EventMethod::PatchApplyBegin,
            EventMsg::PatchApplyEnd(_) => EventMethod::PatchApplyEnd,
            EventMsg::GetHistoryEntryResponse(_) => EventMethod::GetHistoryEntryResponse,
            EventMsg::ShutdownComplete => EventMethod::ShutdownComplete,
        }
    }
}

impl Serialize for EventMethod {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

pub(crate) struct OutgoingMessageSender {
    next_request_id: AtomicI64,
    sender: mpsc::Sender<OutgoingMessage>,
    request_id_to_callback: Mutex<HashMap<RequestId, oneshot::Sender<Result>>>,
}

impl OutgoingMessageSender {
    pub(crate) fn new(sender: mpsc::Sender<OutgoingMessage>) -> Self {
        Self {
            next_request_id: AtomicI64::new(0),
            sender,
            request_id_to_callback: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> oneshot::Receiver<Result> {
        let id = RequestId::Integer(self.next_request_id.fetch_add(1, Ordering::Relaxed));
        let outgoing_message_id = id.clone();
        let (tx_approve, rx_approve) = oneshot::channel();
        {
            let mut request_id_to_callback = self.request_id_to_callback.lock().await;
            request_id_to_callback.insert(id, tx_approve);
        }

        let outgoing_message = OutgoingMessage::Request(OutgoingRequest {
            id: outgoing_message_id,
            method: method.to_string(),
            params,
        });
        let _ = self.sender.send(outgoing_message).await;
        rx_approve
    }

    pub(crate) async fn notify_client_response(&self, id: RequestId, result: Result) {
        let entry = {
            let mut request_id_to_callback = self.request_id_to_callback.lock().await;
            request_id_to_callback.remove_entry(&id)
        };

        match entry {
            Some((id, sender)) => {
                if let Err(err) = sender.send(result) {
                    warn!("could not notify callback for {id:?} due to: {err:?}");
                }
            }
            None => {
                warn!("could not find callback for {id:?}");
            }
        }
    }

    pub(crate) async fn send_response(&self, id: RequestId, result: Result) {
        let outgoing_message = OutgoingMessage::Response(OutgoingResponse { id, result });
        let _ = self.sender.send(outgoing_message).await;
    }

    pub(crate) async fn send_event_as_notification(&self, event: &Event) {
        let params = match serde_json::to_value(event) {
            Ok(v) => Some(v),
            Err(err) => {
                tracing::error!("failed to serialize event: {err}");
                None
            }
        };
        let method = EventMethod::from(&event.msg);
        let outgoing_message =
            OutgoingMessage::Notification(OutgoingNotification { method, params });
        let _ = self.sender.send(outgoing_message).await;
    }

    pub(crate) async fn send_error(&self, id: RequestId, error: JSONRPCErrorError) {
        let outgoing_message = OutgoingMessage::Error(OutgoingError { id, error });
        let _ = self.sender.send(outgoing_message).await;
    }
}

/// Outgoing message from the server to the client.
pub(crate) enum OutgoingMessage {
    Request(OutgoingRequest),
    Notification(OutgoingNotification),
    Response(OutgoingResponse),
    Error(OutgoingError),
}

impl From<OutgoingMessage> for JSONRPCMessage {
    fn from(val: OutgoingMessage) -> Self {
        use OutgoingMessage::*;
        match val {
            Request(OutgoingRequest { id, method, params }) => {
                JSONRPCMessage::Request(JSONRPCRequest {
                    jsonrpc: JSONRPC_VERSION.into(),
                    id,
                    method,
                    params,
                })
            }
            Notification(OutgoingNotification { method, params }) => {
                JSONRPCMessage::Notification(JSONRPCNotification {
                    jsonrpc: JSONRPC_VERSION.into(),
                    method: method.as_str().to_string(),
                    params,
                })
            }
            Response(OutgoingResponse { id, result }) => {
                JSONRPCMessage::Response(JSONRPCResponse {
                    jsonrpc: JSONRPC_VERSION.into(),
                    id,
                    result,
                })
            }
            Error(OutgoingError { id, error }) => JSONRPCMessage::Error(JSONRPCError {
                jsonrpc: JSONRPC_VERSION.into(),
                id,
                error,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct OutgoingRequest {
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct OutgoingNotification {
    pub method: EventMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct OutgoingResponse {
    pub id: RequestId,
    pub result: Result,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct OutgoingError {
    pub error: JSONRPCErrorError,
    pub id: RequestId,
}
