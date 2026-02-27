use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerRequestPayload;
use codex_protocol::ThreadId;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::warn;

use crate::error_code::INTERNAL_ERROR_CODE;
use crate::server_request_error::TURN_TRANSITION_PENDING_REQUEST_ERROR_REASON;

#[cfg(test)]
use codex_protocol::account::PlanType;

pub(crate) type ClientRequestResult = std::result::Result<Result, JSONRPCErrorError>;

/// Stable identifier for a transport connection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ConnectionId(pub(crate) u64);

/// Stable identifier for a client request scoped to a transport connection.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ConnectionRequestId {
    pub(crate) connection_id: ConnectionId,
    pub(crate) request_id: RequestId,
}

#[derive(Debug, Clone)]
pub(crate) enum OutgoingEnvelope {
    ToConnection {
        connection_id: ConnectionId,
        message: OutgoingMessage,
    },
    Broadcast {
        message: OutgoingMessage,
    },
}

/// Sends messages to the client and manages request callbacks.
pub(crate) struct OutgoingMessageSender {
    next_server_request_id: AtomicI64,
    sender: mpsc::Sender<OutgoingEnvelope>,
    request_id_to_callback: Mutex<HashMap<RequestId, PendingCallbackEntry>>,
}

#[derive(Clone)]
pub(crate) struct ThreadScopedOutgoingMessageSender {
    outgoing: Arc<OutgoingMessageSender>,
    connection_ids: Arc<Vec<ConnectionId>>,
    thread_id: ThreadId,
}

struct PendingCallbackEntry {
    callback: oneshot::Sender<ClientRequestResult>,
    thread_id: Option<ThreadId>,
    request: ServerRequest,
}

impl ThreadScopedOutgoingMessageSender {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        connection_ids: Vec<ConnectionId>,
        thread_id: ThreadId,
    ) -> Self {
        Self {
            outgoing,
            connection_ids: Arc::new(connection_ids),
            thread_id,
        }
    }

    pub(crate) async fn send_request(
        &self,
        payload: ServerRequestPayload,
    ) -> (RequestId, oneshot::Receiver<ClientRequestResult>) {
        self.outgoing
            .send_request_to_connections(
                Some(self.connection_ids.as_slice()),
                payload,
                Some(self.thread_id),
            )
            .await
    }

    pub(crate) async fn send_server_notification(&self, notification: ServerNotification) {
        if self.connection_ids.is_empty() {
            return;
        }
        self.outgoing
            .send_server_notification_to_connections(self.connection_ids.as_slice(), notification)
            .await;
    }

    pub(crate) async fn abort_pending_server_requests(&self) {
        self.outgoing
            .cancel_requests_for_thread(
                self.thread_id,
                Some(JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: "client request resolved because the turn state was changed"
                        .to_string(),
                    data: Some(serde_json::json!({ "reason": TURN_TRANSITION_PENDING_REQUEST_ERROR_REASON })),
                }),
            )
            .await
    }

    pub(crate) async fn send_response<T: Serialize>(
        &self,
        request_id: ConnectionRequestId,
        response: T,
    ) {
        self.outgoing.send_response(request_id, response).await;
    }

    pub(crate) async fn send_error(
        &self,
        request_id: ConnectionRequestId,
        error: JSONRPCErrorError,
    ) {
        self.outgoing.send_error(request_id, error).await;
    }
}

impl OutgoingMessageSender {
    pub(crate) fn new(sender: mpsc::Sender<OutgoingEnvelope>) -> Self {
        Self {
            next_server_request_id: AtomicI64::new(0),
            sender,
            request_id_to_callback: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn send_request(
        &self,
        request: ServerRequestPayload,
    ) -> (RequestId, oneshot::Receiver<ClientRequestResult>) {
        self.send_request_to_connections(None, request, None).await
    }

    fn next_request_id(&self) -> RequestId {
        RequestId::Integer(self.next_server_request_id.fetch_add(1, Ordering::Relaxed))
    }

    async fn send_request_to_connections(
        &self,
        connection_ids: Option<&[ConnectionId]>,
        request: ServerRequestPayload,
        thread_id: Option<ThreadId>,
    ) -> (RequestId, oneshot::Receiver<ClientRequestResult>) {
        let id = self.next_request_id();
        let outgoing_message_id = id.clone();
        let request = request.request_with_id(outgoing_message_id.clone());

        let (tx_approve, rx_approve) = oneshot::channel();
        {
            let mut request_id_to_callback = self.request_id_to_callback.lock().await;
            request_id_to_callback.insert(
                id,
                PendingCallbackEntry {
                    callback: tx_approve,
                    thread_id,
                    request: request.clone(),
                },
            );
        }

        let outgoing_message = OutgoingMessage::Request(request);
        let send_result = match connection_ids {
            None => {
                self.sender
                    .send(OutgoingEnvelope::Broadcast {
                        message: outgoing_message,
                    })
                    .await
            }
            Some(connection_ids) => {
                let mut send_error = None;
                for connection_id in connection_ids {
                    if let Err(err) = self
                        .sender
                        .send(OutgoingEnvelope::ToConnection {
                            connection_id: *connection_id,
                            message: outgoing_message.clone(),
                        })
                        .await
                    {
                        send_error = Some(err);
                        break;
                    }
                }
                match send_error {
                    Some(err) => Err(err),
                    None => Ok(()),
                }
            }
        };

        if let Err(err) = send_result {
            warn!("failed to send request {outgoing_message_id:?} to client: {err:?}");
            let mut request_id_to_callback = self.request_id_to_callback.lock().await;
            request_id_to_callback.remove(&outgoing_message_id);
        }
        (outgoing_message_id, rx_approve)
    }

    pub(crate) async fn replay_requests_to_connection_for_thread(
        &self,
        connection_id: ConnectionId,
        thread_id: ThreadId,
    ) {
        let requests = self.pending_requests_for_thread(thread_id).await;
        for request in requests {
            if let Err(err) = self
                .sender
                .send(OutgoingEnvelope::ToConnection {
                    connection_id,
                    message: OutgoingMessage::Request(request),
                })
                .await
            {
                warn!("failed to resend request to client: {err:?}");
            }
        }
    }

    pub(crate) async fn notify_client_response(&self, id: RequestId, result: Result) {
        let entry = self.take_request_callback(&id).await;

        match entry {
            Some((id, entry)) => {
                if let Err(err) = entry.callback.send(Ok(result)) {
                    warn!("could not notify callback for {id:?} due to: {err:?}");
                }
            }
            None => {
                warn!("could not find callback for {id:?}");
            }
        }
    }

    pub(crate) async fn notify_client_error(&self, id: RequestId, error: JSONRPCErrorError) {
        let entry = self.take_request_callback(&id).await;

        match entry {
            Some((id, entry)) => {
                warn!("client responded with error for {id:?}: {error:?}");
                if let Err(err) = entry.callback.send(Err(error)) {
                    warn!("could not notify callback for {id:?} due to: {err:?}");
                }
            }
            None => {
                warn!("could not find callback for {id:?}");
            }
        }
    }

    pub(crate) async fn cancel_request(&self, id: &RequestId) -> bool {
        self.take_request_callback(id).await.is_some()
    }

    async fn take_request_callback(
        &self,
        id: &RequestId,
    ) -> Option<(RequestId, PendingCallbackEntry)> {
        let mut request_id_to_callback = self.request_id_to_callback.lock().await;
        request_id_to_callback.remove_entry(id)
    }

    pub(crate) async fn pending_requests_for_thread(
        &self,
        thread_id: ThreadId,
    ) -> Vec<ServerRequest> {
        let request_id_to_callback = self.request_id_to_callback.lock().await;
        let mut requests = request_id_to_callback
            .iter()
            .filter_map(|(_, entry)| {
                (entry.thread_id == Some(thread_id)).then_some(entry.request.clone())
            })
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| left.id().cmp(right.id()));
        requests
    }

    pub(crate) async fn cancel_requests_for_thread(
        &self,
        thread_id: ThreadId,
        error: Option<JSONRPCErrorError>,
    ) {
        let entries = {
            let mut request_id_to_callback = self.request_id_to_callback.lock().await;
            let request_ids = request_id_to_callback
                .iter()
                .filter_map(|(request_id, entry)| {
                    (entry.thread_id == Some(thread_id)).then_some(request_id.clone())
                })
                .collect::<Vec<_>>();

            let mut entries = Vec::with_capacity(request_ids.len());
            for request_id in request_ids {
                if let Some(entry) = request_id_to_callback.remove(&request_id) {
                    entries.push(entry);
                }
            }
            entries
        };

        if let Some(error) = error {
            for entry in entries {
                if let Err(err) = entry.callback.send(Err(error.clone())) {
                    let request_id = entry.request.id();
                    warn!("could not notify callback for {request_id:?} due to: {err:?}",);
                }
            }
        }
    }

    pub(crate) async fn send_response<T: Serialize>(
        &self,
        request_id: ConnectionRequestId,
        response: T,
    ) {
        match serde_json::to_value(response) {
            Ok(result) => {
                let outgoing_message = OutgoingMessage::Response(OutgoingResponse {
                    id: request_id.request_id,
                    result,
                });
                if let Err(err) = self
                    .sender
                    .send(OutgoingEnvelope::ToConnection {
                        connection_id: request_id.connection_id,
                        message: outgoing_message,
                    })
                    .await
                {
                    warn!("failed to send response to client: {err:?}");
                }
            }
            Err(err) => {
                self.send_error(
                    request_id,
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
        self.send_server_notification_to_connections(&[], notification)
            .await;
    }

    pub(crate) async fn send_server_notification_to_connections(
        &self,
        connection_ids: &[ConnectionId],
        notification: ServerNotification,
    ) {
        tracing::trace!(
            targeted_connections = connection_ids.len(),
            "app-server event: {notification}"
        );
        let outgoing_message = OutgoingMessage::AppServerNotification(notification);
        if connection_ids.is_empty() {
            if let Err(err) = self
                .sender
                .send(OutgoingEnvelope::Broadcast {
                    message: outgoing_message,
                })
                .await
            {
                warn!("failed to send server notification to client: {err:?}");
            }
            return;
        }
        for connection_id in connection_ids {
            if let Err(err) = self
                .sender
                .send(OutgoingEnvelope::ToConnection {
                    connection_id: *connection_id,
                    message: outgoing_message.clone(),
                })
                .await
            {
                warn!("failed to send server notification to client: {err:?}");
            }
        }
    }

    pub(crate) async fn send_notification_to_connections(
        &self,
        connection_ids: &[ConnectionId],
        notification: OutgoingNotification,
    ) {
        let outgoing_message = OutgoingMessage::Notification(notification);
        if connection_ids.is_empty() {
            if let Err(err) = self
                .sender
                .send(OutgoingEnvelope::Broadcast {
                    message: outgoing_message,
                })
                .await
            {
                warn!("failed to send notification to client: {err:?}");
            }
            return;
        }
        for connection_id in connection_ids {
            if let Err(err) = self
                .sender
                .send(OutgoingEnvelope::ToConnection {
                    connection_id: *connection_id,
                    message: outgoing_message.clone(),
                })
                .await
            {
                warn!("failed to send notification to client: {err:?}");
            }
        }
    }

    pub(crate) async fn send_error(
        &self,
        request_id: ConnectionRequestId,
        error: JSONRPCErrorError,
    ) {
        let outgoing_message = OutgoingMessage::Error(OutgoingError {
            id: request_id.request_id,
            error,
        });
        if let Err(err) = self
            .sender
            .send(OutgoingEnvelope::ToConnection {
                connection_id: request_id.connection_id,
                message: outgoing_message,
            })
            .await
        {
            warn!("failed to send error to client: {err:?}");
        }
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
    use std::time::Duration;

    use codex_app_server_protocol::AccountLoginCompletedNotification;
    use codex_app_server_protocol::AccountRateLimitsUpdatedNotification;
    use codex_app_server_protocol::AccountUpdatedNotification;
    use codex_app_server_protocol::ApplyPatchApprovalParams;
    use codex_app_server_protocol::AuthMode;
    use codex_app_server_protocol::ConfigWarningNotification;
    use codex_app_server_protocol::DynamicToolCallParams;
    use codex_app_server_protocol::FileChangeRequestApprovalParams;
    use codex_app_server_protocol::LoginChatGptCompleteNotification;
    use codex_app_server_protocol::ModelRerouteReason;
    use codex_app_server_protocol::ModelReroutedNotification;
    use codex_app_server_protocol::RateLimitSnapshot;
    use codex_app_server_protocol::RateLimitWindow;
    use codex_app_server_protocol::ToolRequestUserInputParams;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::time::timeout;
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
                    "error": null,
                },
            }),
            serde_json::to_value(jsonrpc_notification)
                .expect("ensure the strum macros serialize the method field correctly"),
            "ensure the strum macros serialize the method field correctly"
        );
    }

    #[test]
    fn verify_account_login_completed_notification_serialization() {
        let notification =
            ServerNotification::AccountLoginCompleted(AccountLoginCompletedNotification {
                login_id: Some(Uuid::nil().to_string()),
                success: true,
                error: None,
            });

        let jsonrpc_notification = OutgoingMessage::AppServerNotification(notification);
        assert_eq!(
            json!({
                "method": "account/login/completed",
                "params": {
                    "loginId": Uuid::nil().to_string(),
                    "success": true,
                    "error": null,
                },
            }),
            serde_json::to_value(jsonrpc_notification)
                .expect("ensure the notification serializes correctly"),
            "ensure the notification serializes correctly"
        );
    }

    #[test]
    fn verify_account_rate_limits_notification_serialization() {
        let notification =
            ServerNotification::AccountRateLimitsUpdated(AccountRateLimitsUpdatedNotification {
                rate_limits: RateLimitSnapshot {
                    limit_id: Some("codex".to_string()),
                    limit_name: None,
                    primary: Some(RateLimitWindow {
                        used_percent: 25,
                        window_duration_mins: Some(15),
                        resets_at: Some(123),
                    }),
                    secondary: None,
                    credits: None,
                    plan_type: Some(PlanType::Plus),
                },
            });

        let jsonrpc_notification = OutgoingMessage::AppServerNotification(notification);
        assert_eq!(
            json!({
                "method": "account/rateLimits/updated",
                "params": {
                        "rateLimits": {
                        "limitId": "codex",
                        "limitName": null,
                        "primary": {
                            "usedPercent": 25,
                            "windowDurationMins": 15,
                            "resetsAt": 123
                        },
                        "secondary": null,
                        "credits": null,
                        "planType": "plus"
                    }
                },
            }),
            serde_json::to_value(jsonrpc_notification)
                .expect("ensure the notification serializes correctly"),
            "ensure the notification serializes correctly"
        );
    }

    #[test]
    fn verify_account_updated_notification_serialization() {
        let notification = ServerNotification::AccountUpdated(AccountUpdatedNotification {
            auth_mode: Some(AuthMode::ApiKey),
        });

        let jsonrpc_notification = OutgoingMessage::AppServerNotification(notification);
        assert_eq!(
            json!({
                "method": "account/updated",
                "params": {
                    "authMode": "apikey"
                },
            }),
            serde_json::to_value(jsonrpc_notification)
                .expect("ensure the notification serializes correctly"),
            "ensure the notification serializes correctly"
        );
    }

    #[test]
    fn verify_config_warning_notification_serialization() {
        let notification = ServerNotification::ConfigWarning(ConfigWarningNotification {
            summary: "Config error: using defaults".to_string(),
            details: Some("error loading config: bad config".to_string()),
            path: None,
            range: None,
        });

        let jsonrpc_notification = OutgoingMessage::AppServerNotification(notification);
        assert_eq!(
            json!( {
                "method": "configWarning",
                "params": {
                    "summary": "Config error: using defaults",
                    "details": "error loading config: bad config",
                },
            }),
            serde_json::to_value(jsonrpc_notification)
                .expect("ensure the notification serializes correctly"),
            "ensure the notification serializes correctly"
        );
    }

    #[test]
    fn verify_model_rerouted_notification_serialization() {
        let notification = ServerNotification::ModelRerouted(ModelReroutedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            from_model: "gpt-5.3-codex".to_string(),
            to_model: "gpt-5.2".to_string(),
            reason: ModelRerouteReason::HighRiskCyberActivity,
        });

        let jsonrpc_notification = OutgoingMessage::AppServerNotification(notification);
        assert_eq!(
            json!({
                "method": "model/rerouted",
                "params": {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "fromModel": "gpt-5.3-codex",
                    "toModel": "gpt-5.2",
                    "reason": "highRiskCyberActivity",
                },
            }),
            serde_json::to_value(jsonrpc_notification)
                .expect("ensure the notification serializes correctly"),
            "ensure the notification serializes correctly"
        );
    }

    #[tokio::test]
    async fn send_response_routes_to_target_connection() {
        let (tx, mut rx) = mpsc::channel::<OutgoingEnvelope>(4);
        let outgoing = OutgoingMessageSender::new(tx);
        let request_id = ConnectionRequestId {
            connection_id: ConnectionId(42),
            request_id: RequestId::Integer(7),
        };

        outgoing
            .send_response(request_id.clone(), json!({ "ok": true }))
            .await;

        let envelope = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("should receive envelope before timeout")
            .expect("channel should contain one message");

        match envelope {
            OutgoingEnvelope::ToConnection {
                connection_id,
                message,
            } => {
                assert_eq!(connection_id, ConnectionId(42));
                let OutgoingMessage::Response(response) = message else {
                    panic!("expected response message");
                };
                assert_eq!(response.id, request_id.request_id);
                assert_eq!(response.result, json!({ "ok": true }));
            }
            other => panic!("expected targeted response envelope, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn send_error_routes_to_target_connection() {
        let (tx, mut rx) = mpsc::channel::<OutgoingEnvelope>(4);
        let outgoing = OutgoingMessageSender::new(tx);
        let request_id = ConnectionRequestId {
            connection_id: ConnectionId(9),
            request_id: RequestId::Integer(3),
        };
        let error = JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: "boom".to_string(),
            data: None,
        };

        outgoing.send_error(request_id.clone(), error.clone()).await;

        let envelope = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("should receive envelope before timeout")
            .expect("channel should contain one message");

        match envelope {
            OutgoingEnvelope::ToConnection {
                connection_id,
                message,
            } => {
                assert_eq!(connection_id, ConnectionId(9));
                let OutgoingMessage::Error(outgoing_error) = message else {
                    panic!("expected error message");
                };
                assert_eq!(outgoing_error.id, RequestId::Integer(3));
                assert_eq!(outgoing_error.error, error);
            }
            other => panic!("expected targeted error envelope, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn notify_client_error_forwards_error_to_waiter() {
        let (tx, _rx) = mpsc::channel::<OutgoingEnvelope>(4);
        let outgoing = OutgoingMessageSender::new(tx);

        let (request_id, wait_for_result) = outgoing
            .send_request(ServerRequestPayload::ApplyPatchApproval(
                ApplyPatchApprovalParams {
                    conversation_id: ThreadId::new(),
                    call_id: "call-id".to_string(),
                    file_changes: HashMap::new(),
                    reason: None,
                    grant_root: None,
                },
            ))
            .await;

        let error = JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: "refresh failed".to_string(),
            data: None,
        };

        outgoing
            .notify_client_error(request_id, error.clone())
            .await;

        let result = timeout(Duration::from_secs(1), wait_for_result)
            .await
            .expect("wait should not time out")
            .expect("waiter should receive a callback");
        assert_eq!(result, Err(error));
    }

    #[tokio::test]
    async fn pending_requests_for_thread_returns_thread_requests_in_request_id_order() {
        let (tx, _rx) = mpsc::channel::<OutgoingEnvelope>(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let thread_id = ThreadId::new();
        let thread_outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing.clone(),
            vec![ConnectionId(1)],
            thread_id,
        );

        let (dynamic_tool_request_id, _dynamic_tool_waiter) = thread_outgoing
            .send_request(ServerRequestPayload::DynamicToolCall(
                DynamicToolCallParams {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    call_id: "call-0".to_string(),
                    tool: "tool".to_string(),
                    arguments: json!({}),
                },
            ))
            .await;
        let (first_request_id, _first_waiter) = thread_outgoing
            .send_request(ServerRequestPayload::ToolRequestUserInput(
                ToolRequestUserInputParams {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "call-1".to_string(),
                    questions: vec![],
                },
            ))
            .await;
        let (second_request_id, _second_waiter) = thread_outgoing
            .send_request(ServerRequestPayload::FileChangeRequestApproval(
                FileChangeRequestApprovalParams {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "call-2".to_string(),
                    reason: None,
                    grant_root: None,
                },
            ))
            .await;
        let pending_requests = outgoing.pending_requests_for_thread(thread_id).await;
        assert_eq!(
            pending_requests
                .iter()
                .map(ServerRequest::id)
                .collect::<Vec<_>>(),
            vec![
                &dynamic_tool_request_id,
                &first_request_id,
                &second_request_id
            ]
        );
    }

    #[tokio::test]
    async fn cancel_requests_for_thread_cancels_all_thread_requests() {
        let (tx, _rx) = mpsc::channel::<OutgoingEnvelope>(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let thread_id = ThreadId::new();
        let thread_outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing.clone(),
            vec![ConnectionId(1)],
            thread_id,
        );

        let (_dynamic_tool_request_id, dynamic_tool_waiter) = thread_outgoing
            .send_request(ServerRequestPayload::DynamicToolCall(
                DynamicToolCallParams {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    call_id: "call-0".to_string(),
                    tool: "tool".to_string(),
                    arguments: json!({}),
                },
            ))
            .await;
        let (_request_id, user_input_waiter) = thread_outgoing
            .send_request(ServerRequestPayload::ToolRequestUserInput(
                ToolRequestUserInputParams {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "call-1".to_string(),
                    questions: vec![],
                },
            ))
            .await;
        let error = JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: "tracked request cancelled".to_string(),
            data: None,
        };

        outgoing
            .cancel_requests_for_thread(thread_id, Some(error.clone()))
            .await;

        let dynamic_tool_result = timeout(Duration::from_secs(1), dynamic_tool_waiter)
            .await
            .expect("dynamic tool waiter should resolve")
            .expect("dynamic tool waiter should receive a callback");
        let user_input_result = timeout(Duration::from_secs(1), user_input_waiter)
            .await
            .expect("user input waiter should resolve")
            .expect("user input waiter should receive a callback");
        assert_eq!(dynamic_tool_result, Err(error.clone()));
        assert_eq!(user_input_result, Err(error));
        assert!(
            outgoing
                .pending_requests_for_thread(thread_id)
                .await
                .is_empty()
        );
    }
}
