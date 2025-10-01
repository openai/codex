use std::collections::HashMap;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerRequestPayload;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::warn;

use crate::error_code::INTERNAL_ERROR_CODE;

/// Sends messages to the client and manages request callbacks.
pub(crate) struct OutgoingMessageSender {
    next_request_id: AtomicI64,
    sender: mpsc::UnboundedSender<OutgoingMessage>,
    request_id_to_callback: Mutex<HashMap<RequestId, oneshot::Sender<Result>>>,
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
        request: ServerRequestPayload,
    ) -> oneshot::Receiver<Result> {
        let id = RequestId::Integer(self.next_request_id.fetch_add(1, Ordering::Relaxed));
        let outgoing_message_id = id.clone();
        let (tx_approve, rx_approve) = oneshot::channel();
        {
            let mut request_id_to_callback = self.request_id_to_callback.lock().await;
            request_id_to_callback.insert(id, tx_approve);
        }

        let outgoing_message =
            OutgoingMessage::Request(request.request_with_id(outgoing_message_id));
        let _ = self.sender.send(outgoing_message);
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

    pub(crate) async fn send_response<T: Serialize>(&self, id: RequestId, response: T) {
        match serde_json::to_value(response) {
            Ok(result) => {
                let outgoing_message = OutgoingMessage::Response(OutgoingResponse { id, result });
                let _ = self.sender.send(outgoing_message);
            }
            Err(err) => {
                self.send_error(
                    id,
                    JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!("failed to serialize response: {err}"),
                        data: None,
                    },
                )
                .await;
            }
        }
    }

    pub(crate) async fn send_server_notification(&self, notification: ServerNotification) {
        let _ = self
            .sender
            .send(OutgoingMessage::AppServerNotification(notification));
    }

    /// All notifications should be migrated to [`ServerNotification`] and
    /// [`OutgoingMessage::Notification`] should be removed.
    pub(crate) async fn send_notification(&self, notification: OutgoingNotification) {
        let outgoing_message = OutgoingMessage::Notification(notification);
        let _ = self.sender.send(outgoing_message);
    }

    pub(crate) async fn send_error(&self, id: RequestId, error: JSONRPCErrorError) {
        let outgoing_message = OutgoingMessage::Error(OutgoingError { id, error });
        let _ = self.sender.send(outgoing_message);
    }
}

/// Outgoing message from the server to the client.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum OutgoingMessage {
    Request(ServerRequest),
    Notification(OutgoingNotification),
    /// AppServerNotification is specific to the case where this is run as an
    /// "app server" as opposed to an MCP server.
    AppServerNotification(ServerNotification),
    Response(OutgoingResponse),
    Error(OutgoingError),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct OutgoingNotification {
    pub method: String,
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

#[cfg(test)]
mod tests {
    use codex_app_server_protocol::LoginChatGptCompleteNotification;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn verify_server_notification_serialization() {
        let notification =
            ServerNotification::LoginChatGptComplete(LoginChatGptCompleteNotification {
                login_id: Uuid::nil(),
                success: true,
                error: None,
            });

        let jsonrpc_notification = OutgoingMessage::AppServerNotification(notification);
        assert_eq!(
            json!({
                "method": "loginChatGptComplete",
                "params": {
                    "loginId": Uuid::nil(),
                    "success": true,
                },
            }),
            serde_json::to_value(jsonrpc_notification)
                .expect("ensure the strum macros serialize the method field correctly"),
            "ensure the strum macros serialize the method field correctly"
        );
    }
}
