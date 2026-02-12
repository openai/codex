use std::collections::HashMap;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_core::protocol::Event;
use codex_protocol::ThreadId;
use rmcp::model::CustomNotification;
use rmcp::model::CustomRequest;
use rmcp::model::ErrorData;
use rmcp::model::JsonRpcError;
use rmcp::model::JsonRpcMessage;
use rmcp::model::JsonRpcNotification;
use rmcp::model::JsonRpcRequest;
use rmcp::model::JsonRpcResponse;
use rmcp::model::JsonRpcVersion2_0;
use rmcp::model::RequestId;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::warn;

pub(crate) type OutgoingJsonRpcMessage = JsonRpcMessage<CustomRequest, Value, CustomNotification>;

/// Sends messages to the client and manages request callbacks.
pub(crate) struct OutgoingMessageSender {
    next_request_id: AtomicI64,
    sender: mpsc::UnboundedSender<OutgoingMessage>,
    request_id_to_callback: Mutex<HashMap<RequestId, oneshot::Sender<Value>>>,
}

impl OutgoingMessageSender {
    pub(crate) fn new(sender: mpsc::UnboundedSender<OutgoingMessage>) -> Self {
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
    ) -> oneshot::Receiver<Value> {
        let id = RequestId::Number(self.next_request_id.fetch_add(1, Ordering::Relaxed));
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
        let _ = self.sender.send(outgoing_message);
        rx_approve
    }

    pub(crate) async fn notify_client_response(&self, id: RequestId, result: Value) {
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

    pub(crate) async fn send_response<T: Serialize>(&self, id: RequestId, response: T) {
        let result = match serde_json::to_value(response) {
            Ok(result) => result,
            Err(err) => {
                self.send_error(
                    id,
                    ErrorData::internal_error(format!("failed to serialize response: {err}"), None),
                )
                .await;
                return;
            }
        };

        let outgoing_message = OutgoingMessage::Response(OutgoingResponse { id, result });
        let _ = self.sender.send(outgoing_message);
    }

    /// This is used with the MCP server, but not the more general JSON-RPC app
    /// server. Prefer [`OutgoingMessageSender::send_server_notification`] where
    /// possible.
    pub(crate) async fn send_event_as_notification(
        &self,
        event: &Event,
        meta: Option<OutgoingNotificationMeta>,
    ) {
        #[expect(clippy::expect_used)]
        let event_json = serde_json::to_value(event).expect("Event must serialize");

        let params = if let Ok(params) = serde_json::to_value(OutgoingNotificationParams {
            meta,
            event: event_json.clone(),
        }) {
            params
        } else {
            warn!("Failed to serialize event as OutgoingNotificationParams");
            event_json
        };

        self.send_notification(OutgoingNotification {
            method: "codex/event".to_string(),
            params: Some(params.clone()),
        })
        .await;
    }

    pub(crate) async fn send_notification(&self, notification: OutgoingNotification) {
        let outgoing_message = OutgoingMessage::Notification(notification);
        let _ = self.sender.send(outgoing_message);
    }

    pub(crate) async fn send_error(&self, id: RequestId, error: ErrorData) {
        let outgoing_message = OutgoingMessage::Error(OutgoingError { id, error });
        let _ = self.sender.send(outgoing_message);
    }
}

/// Outgoing message from the server to the client.
pub(crate) enum OutgoingMessage {
    Request(OutgoingRequest),
    Notification(OutgoingNotification),
    Response(OutgoingResponse),
    Error(OutgoingError),
}

impl From<OutgoingMessage> for OutgoingJsonRpcMessage {
    fn from(val: OutgoingMessage) -> Self {
        use OutgoingMessage::*;
        match val {
            Request(OutgoingRequest { id, method, params }) => {
                JsonRpcMessage::Request(JsonRpcRequest {
                    jsonrpc: JsonRpcVersion2_0,
                    id,
                    request: CustomRequest::new(method, params),
                })
            }
            Notification(OutgoingNotification { method, params }) => {
                JsonRpcMessage::Notification(JsonRpcNotification {
                    jsonrpc: JsonRpcVersion2_0,
                    notification: CustomNotification::new(method, params),
                })
            }
            Response(OutgoingResponse { id, result }) => {
                JsonRpcMessage::Response(JsonRpcResponse {
                    jsonrpc: JsonRpcVersion2_0,
                    id,
                    result,
                })
            }
            Error(OutgoingError { id, error }) => JsonRpcMessage::Error(JsonRpcError {
                jsonrpc: JsonRpcVersion2_0,
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
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct OutgoingNotificationParams {
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<OutgoingNotificationMeta>,

    #[serde(flatten)]
    pub event: serde_json::Value,
}

// Additional mcp-specific data to be added to a [`codex_core::protocol::Event`] as notification.params._meta
// MCP Spec: https://modelcontextprotocol.io/specification/2025-06-18/basic#meta
// Typescript Schema: https://github.com/modelcontextprotocol/modelcontextprotocol/blob/0695a497eb50a804fc0e88c18a93a21a675d6b3e/schema/2025-06-18/schema.ts
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OutgoingNotificationMeta {
    pub request_id: Option<RequestId>,

    /// Because multiple threads may be multiplexed over a single MCP connection,
    /// include the `threadId` in the notification meta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<ThreadId>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct OutgoingResponse {
    pub id: RequestId,
    pub result: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct OutgoingError {
    pub error: ErrorData,
    pub id: RequestId,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::Result;
    use codex_core::protocol::AskForApproval;
    use codex_core::protocol::EventMsg;
    use codex_core::protocol::SandboxPolicy;
    use codex_core::protocol::SessionConfiguredEvent;
    use codex_protocol::ThreadId;
    use codex_protocol::openai_models::ReasoningEffort;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn outgoing_request_serializes_as_jsonrpc_request() {
        let msg: OutgoingJsonRpcMessage = OutgoingMessage::Request(OutgoingRequest {
            id: RequestId::Number(1),
            method: "elicitation/create".to_string(),
            params: Some(json!({ "k": "v" })),
        })
        .into();

        let value = serde_json::to_value(msg).expect("message should serialize");
        let obj = value.as_object().expect("json object");

        assert_eq!(obj.get("jsonrpc"), Some(&json!("2.0")));
        assert_eq!(obj.get("id"), Some(&json!(1)));
        assert_eq!(obj.get("method"), Some(&json!("elicitation/create")));
        assert_eq!(obj.get("params"), Some(&json!({ "k": "v" })));
        assert!(
            obj.get("request").is_none(),
            "rmcp request must flatten to JSON-RPC method/params"
        );
    }

    #[test]
    fn outgoing_notification_serializes_as_jsonrpc_notification() {
        let msg: OutgoingJsonRpcMessage = OutgoingMessage::Notification(OutgoingNotification {
            method: "notifications/initialized".to_string(),
            params: None,
        })
        .into();

        let value = serde_json::to_value(msg).expect("message should serialize");
        let obj = value.as_object().expect("json object");

        assert_eq!(obj.get("jsonrpc"), Some(&json!("2.0")));
        assert_eq!(obj.get("method"), Some(&json!("notifications/initialized")));
        assert_eq!(obj.get("params"), Some(&serde_json::Value::Null));
        assert!(
            obj.get("notification").is_none(),
            "rmcp notification must flatten to JSON-RPC method/params"
        );
    }

    #[tokio::test]
    async fn test_send_event_as_notification() -> Result<()> {
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<OutgoingMessage>();
        let outgoing_message_sender = OutgoingMessageSender::new(outgoing_tx);

        let thread_id = ThreadId::new();
        let rollout_file = NamedTempFile::new()?;
        let event = Event {
            id: "1".to_string(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: thread_id,
                forked_from_id: None,
                thread_name: None,
                model: "gpt-4o".to_string(),
                model_provider_id: "test-provider".to_string(),
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                cwd: PathBuf::from("/home/user/project"),
                reasoning_effort: Some(ReasoningEffort::default()),
                history_log_id: 1,
                history_entry_count: 1000,
                initial_messages: None,
                network_proxy: None,
                rollout_path: Some(rollout_file.path().to_path_buf()),
            }),
        };

        outgoing_message_sender
            .send_event_as_notification(&event, None)
            .await;

        let result = outgoing_rx.recv().await.unwrap();
        let OutgoingMessage::Notification(OutgoingNotification { method, params }) = result else {
            panic!("expected Notification for first message");
        };
        assert_eq!(method, "codex/event");

        let Ok(expected_params) = serde_json::to_value(&event) else {
            panic!("Event must serialize");
        };
        assert_eq!(params, Some(expected_params));
        Ok(())
    }

    #[tokio::test]
    async fn test_send_event_as_notification_with_meta() -> Result<()> {
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<OutgoingMessage>();
        let outgoing_message_sender = OutgoingMessageSender::new(outgoing_tx);

        let conversation_id = ThreadId::new();
        let rollout_file = NamedTempFile::new()?;
        let session_configured_event = SessionConfiguredEvent {
            session_id: conversation_id,
            forked_from_id: None,
            thread_name: None,
            model: "gpt-4o".to_string(),
            model_provider_id: "test-provider".to_string(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            cwd: PathBuf::from("/home/user/project"),
            reasoning_effort: Some(ReasoningEffort::default()),
            history_log_id: 1,
            history_entry_count: 1000,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(rollout_file.path().to_path_buf()),
        };
        let event = Event {
            id: "1".to_string(),
            msg: EventMsg::SessionConfigured(session_configured_event.clone()),
        };
        let meta = OutgoingNotificationMeta {
            request_id: Some(RequestId::String("123".into())),
            thread_id: None,
        };

        outgoing_message_sender
            .send_event_as_notification(&event, Some(meta))
            .await;

        let result = outgoing_rx.recv().await.unwrap();
        let OutgoingMessage::Notification(OutgoingNotification { method, params }) = result else {
            panic!("expected Notification for first message");
        };
        assert_eq!(method, "codex/event");
        let expected_params = json!({
            "_meta": {
                "requestId": "123",
            },
            "id": "1",
            "msg": {
                "type": "session_configured",
                "session_id": session_configured_event.session_id,
                "model": "gpt-4o",
                "model_provider_id": "test-provider",
                "approval_policy": "never",
                "sandbox_policy": {
                    "type": "read-only"
                },
                "cwd": "/home/user/project",
                "reasoning_effort": session_configured_event.reasoning_effort,
                "history_log_id": session_configured_event.history_log_id,
                "history_entry_count": session_configured_event.history_entry_count,
                "rollout_path": rollout_file.path().to_path_buf(),
            }
        });
        assert_eq!(params.unwrap(), expected_params);
        Ok(())
    }

    #[tokio::test]
    async fn test_send_event_as_notification_with_meta_and_thread_id() -> Result<()> {
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<OutgoingMessage>();
        let outgoing_message_sender = OutgoingMessageSender::new(outgoing_tx);

        let thread_id = ThreadId::new();
        let rollout_file = NamedTempFile::new()?;
        let session_configured_event = SessionConfiguredEvent {
            session_id: thread_id,
            forked_from_id: None,
            thread_name: None,
            model: "gpt-4o".to_string(),
            model_provider_id: "test-provider".to_string(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            cwd: PathBuf::from("/home/user/project"),
            reasoning_effort: Some(ReasoningEffort::default()),
            history_log_id: 1,
            history_entry_count: 1000,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(rollout_file.path().to_path_buf()),
        };
        let event = Event {
            id: "1".to_string(),
            msg: EventMsg::SessionConfigured(session_configured_event.clone()),
        };
        let meta = OutgoingNotificationMeta {
            request_id: Some(RequestId::String("123".into())),
            thread_id: Some(thread_id),
        };

        outgoing_message_sender
            .send_event_as_notification(&event, Some(meta))
            .await;

        let result = outgoing_rx.recv().await.unwrap();
        let OutgoingMessage::Notification(OutgoingNotification { method, params }) = result else {
            panic!("expected Notification for first message");
        };
        assert_eq!(method, "codex/event");
        let expected_params = json!({
            "_meta": {
                "requestId": "123",
                "threadId": thread_id.to_string(),
            },
            "id": "1",
            "msg": {
                "type": "session_configured",
                "session_id": session_configured_event.session_id,
                "model": "gpt-4o",
                "model_provider_id": "test-provider",
                "approval_policy": "never",
                "sandbox_policy": {
                    "type": "read-only"
                },
                "cwd": "/home/user/project",
                "reasoning_effort": session_configured_event.reasoning_effort,
                "history_log_id": session_configured_event.history_log_id,
                "history_entry_count": session_configured_event.history_entry_count,
                "rollout_path": rollout_file.path().to_path_buf(),
            }
        });
        assert_eq!(params.unwrap(), expected_params);
        Ok(())
    }
}
