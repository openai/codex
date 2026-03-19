use crate::outgoing_message::ClientRequestResult;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::ThreadScopedOutgoingMessageSender;
use crate::server_request_error::is_turn_transition_server_request_error;
use crate::thread_state::ThreadStateManager;
use async_trait::async_trait;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ModelCompactParams;
use codex_app_server_protocol::ModelCompactResponse;
use codex_app_server_protocol::ModelRequestFailedNotification;
use codex_app_server_protocol::ModelRequestParams;
use codex_app_server_protocol::ModelRequestResponse;
use codex_app_server_protocol::ModelStreamEventNotification;
use codex_app_server_protocol::ModelStreamMetadataNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_core::delegated_model_transport::DelegatedModelCompactRequest;
use codex_core::delegated_model_transport::DelegatedModelEvent;
use codex_core::delegated_model_transport::DelegatedModelRequest;
use codex_core::delegated_model_transport::DelegatedModelTransport;
use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DelegatedRequestKey {
    thread_id: String,
    turn_id: String,
    request_id: String,
}

impl DelegatedRequestKey {
    fn from_request(request: &DelegatedModelRequest) -> Self {
        Self {
            thread_id: request.thread_id.to_string(),
            turn_id: request.turn_id.clone(),
            request_id: request.request_id.clone(),
        }
    }

    fn from_metadata(notification: &ModelStreamMetadataNotification) -> Self {
        Self {
            thread_id: notification.thread_id.clone(),
            turn_id: notification.turn_id.clone(),
            request_id: notification.request_id.clone(),
        }
    }

    fn from_stream_event(notification: &ModelStreamEventNotification) -> Self {
        Self {
            thread_id: notification.thread_id.clone(),
            turn_id: notification.turn_id.clone(),
            request_id: notification.request_id.clone(),
        }
    }

    fn from_failed(notification: &ModelRequestFailedNotification) -> Self {
        Self {
            thread_id: notification.thread_id.clone(),
            turn_id: notification.turn_id.clone(),
            request_id: notification.request_id.clone(),
        }
    }
}

#[derive(Clone)]
struct InflightDelegatedRequest {
    connection_id: ConnectionId,
    sender: mpsc::Sender<DelegatedModelEvent>,
}

#[derive(Clone)]
pub(crate) struct AppServerDelegatedModelTransport {
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
    inflight: Arc<Mutex<HashMap<DelegatedRequestKey, InflightDelegatedRequest>>>,
}

impl AppServerDelegatedModelTransport {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        thread_state_manager: ThreadStateManager,
    ) -> Self {
        Self {
            outgoing,
            thread_state_manager,
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) async fn handle_client_notification(
        &self,
        connection_id: ConnectionId,
        notification: ClientNotification,
    ) -> Result<()> {
        match notification {
            ClientNotification::Initialized => Ok(()),
            ClientNotification::ModelStreamMetadata(notification) => {
                self.deliver_metadata(connection_id, notification).await
            }
            ClientNotification::ModelStreamEvent(notification) => {
                self.deliver_stream_event(connection_id, notification).await
            }
            ClientNotification::ModelRequestFailed(notification) => {
                self.deliver_request_failed(connection_id, notification)
                    .await
            }
        }
    }

    async fn deliver_metadata(
        &self,
        connection_id: ConnectionId,
        notification: ModelStreamMetadataNotification,
    ) -> Result<()> {
        let key = DelegatedRequestKey::from_metadata(&notification);
        self.deliver_event(
            connection_id,
            key,
            DelegatedModelEvent::StreamMetadata(notification.metadata),
            /*final_event*/ false,
        )
        .await
    }

    async fn deliver_stream_event(
        &self,
        connection_id: ConnectionId,
        notification: ModelStreamEventNotification,
    ) -> Result<()> {
        let final_event = stream_event_is_final(&notification.event);
        let key = DelegatedRequestKey::from_stream_event(&notification);
        self.deliver_event(
            connection_id,
            key,
            DelegatedModelEvent::StreamEvent(notification.event),
            final_event,
        )
        .await
    }

    async fn deliver_request_failed(
        &self,
        connection_id: ConnectionId,
        notification: ModelRequestFailedNotification,
    ) -> Result<()> {
        let key = DelegatedRequestKey::from_failed(&notification);
        self.deliver_event(
            connection_id,
            key,
            DelegatedModelEvent::RequestFailed(notification.error),
            /*final_event*/ true,
        )
        .await
    }

    async fn deliver_event(
        &self,
        connection_id: ConnectionId,
        key: DelegatedRequestKey,
        event: DelegatedModelEvent,
        final_event: bool,
    ) -> Result<()> {
        let inflight = {
            let inflight = self.inflight.lock().await;
            inflight.get(&key).cloned()
        }
        .ok_or_else(|| {
            CodexErr::InvalidRequest(format!(
                "received delegated model notification for unknown request: thread_id={} turn_id={} request_id={}",
                key.thread_id, key.turn_id, key.request_id
            ))
        })?;

        if inflight.connection_id != connection_id {
            return Err(CodexErr::InvalidRequest(format!(
                "received delegated model notification from unexpected connection: expected={:?} actual={:?} thread_id={} turn_id={} request_id={}",
                inflight.connection_id, connection_id, key.thread_id, key.turn_id, key.request_id
            )));
        }

        if inflight.sender.send(event).await.is_err() {
            self.inflight.lock().await.remove(&key);
            return Err(CodexErr::InvalidRequest(format!(
                "received delegated model notification for completed request: thread_id={} turn_id={} request_id={}",
                key.thread_id, key.turn_id, key.request_id
            )));
        }
        if final_event {
            self.inflight.lock().await.remove(&key);
        }
        Ok(())
    }

    async fn select_handler_connection(&self, thread_id: ThreadId) -> Result<ConnectionId> {
        let mut connection_ids = self
            .thread_state_manager
            .subscribed_connection_ids_with_experimental_api(thread_id)
            .await;
        connection_ids.sort_by_key(|connection_id| connection_id.0);
        match connection_ids.as_slice() {
            [] => Err(CodexErr::InvalidRequest(format!(
                "cannot delegate model request for thread {thread_id}: no subscribed experimental-api client is available to handle model/request"
            ))),
            [connection_id] => Ok(*connection_id),
            _ => Err(CodexErr::InvalidRequest(format!(
                "cannot delegate model request for thread {thread_id}: multiple subscribed experimental-api clients are attached and PR 1 requires exactly one model/request handler"
            ))),
        }
    }

    async fn handle_model_request_response(
        &self,
        request: &DelegatedModelRequest,
        response: std::result::Result<ClientRequestResult, tokio::sync::oneshot::error::RecvError>,
    ) -> Result<()> {
        match response {
            Ok(Ok(value)) => {
                let response = serde_json::from_value::<ModelRequestResponse>(value)
                    .map_err(CodexErr::Json)?;
                if response.accepted {
                    Ok(())
                } else {
                    let reason = response.rejection_reason.unwrap_or_else(|| {
                        "delegated model request was rejected by the client".to_string()
                    });
                    Err(CodexErr::InvalidRequest(reason))
                }
            }
            Ok(Err(err)) if is_turn_transition_server_request_error(&err) => {
                Err(CodexErr::InvalidRequest(format!(
                    "delegated model request was cancelled during a turn transition: turn_id={}",
                    request.turn_id
                )))
            }
            Ok(Err(err)) => Err(CodexErr::InvalidRequest(format!(
                "client rejected delegated model request: {}",
                err.message
            ))),
            Err(err) => Err(CodexErr::InvalidRequest(format!(
                "client closed delegated model request before responding: {err}"
            ))),
        }
    }

    async fn remove_inflight_request(&self, key: &DelegatedRequestKey) {
        self.inflight.lock().await.remove(key);
    }
}

fn stream_event_is_final(event: &serde_json::Value) -> bool {
    event
        .get("type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|event_type| {
            matches!(
                event_type,
                "response.completed" | "response.failed" | "response.incomplete"
            )
        })
}

impl AppServerDelegatedModelTransport {
    async fn start_model_request_impl(
        &self,
        request: DelegatedModelRequest,
    ) -> Result<mpsc::Receiver<DelegatedModelEvent>> {
        let key = DelegatedRequestKey::from_request(&request);
        let result = async {
            let connection_id = self.select_handler_connection(request.thread_id).await?;
            let (tx, rx) = mpsc::channel(1600);
            let existing = self.inflight.lock().await.insert(
                key.clone(),
                InflightDelegatedRequest {
                    connection_id,
                    sender: tx,
                },
            );
            if existing.is_some() {
                return Err(CodexErr::InvalidRequest(format!(
                    "delegated model request already exists: thread_id={} turn_id={} request_id={}",
                    key.thread_id, key.turn_id, key.request_id
                )));
            }
            let request_sender = ThreadScopedOutgoingMessageSender::new(
                Arc::clone(&self.outgoing),
                vec![connection_id],
                request.thread_id,
            );
            let (_, response_rx) = request_sender
                .send_request(ServerRequestPayload::ModelRequest(ModelRequestParams {
                    thread_id: key.thread_id.clone(),
                    turn_id: key.turn_id.clone(),
                    request_id: key.request_id.clone(),
                    request: request.request.clone(),
                }))
                .await;
            self.handle_model_request_response(&request, response_rx.await)
                .await?;
            Ok::<mpsc::Receiver<DelegatedModelEvent>, CodexErr>(rx)
        }
        .await;

        let receiver = match result {
            Ok(receiver) => receiver,
            Err(err) => {
                self.remove_inflight_request(&key).await;
                return Err(err);
            }
        };

        Ok(receiver)
    }

    async fn run_model_compact_request_impl(
        &self,
        request: DelegatedModelCompactRequest,
    ) -> Result<Vec<codex_protocol::models::ResponseItem>> {
        let connection_id = self.select_handler_connection(request.thread_id).await?;
        let request_sender = ThreadScopedOutgoingMessageSender::new(
            Arc::clone(&self.outgoing),
            vec![connection_id],
            request.thread_id,
        );
        let (_, response_rx) = request_sender
            .send_request(ServerRequestPayload::ModelCompact(ModelCompactParams {
                thread_id: request.thread_id.to_string(),
                turn_id: request.turn_id.clone(),
                request_id: request.request_id.clone(),
                request: request.request.clone(),
            }))
            .await;

        match response_rx.await {
            Ok(Ok(value)) => serde_json::from_value::<ModelCompactResponse>(value)
                .map(|response| response.output)
                .map_err(CodexErr::Json),
            Ok(Err(err)) if is_turn_transition_server_request_error(&err) => {
                Err(CodexErr::InvalidRequest(format!(
                    "delegated model compact request was cancelled during a turn transition: turn_id={}",
                    request.turn_id
                )))
            }
            Ok(Err(err)) => Err(CodexErr::InvalidRequest(format!(
                "client rejected delegated model compact request: {}",
                err.message
            ))),
            Err(err) => Err(CodexErr::InvalidRequest(format!(
                "client closed delegated model compact request before responding: {err}"
            ))),
        }
    }
}

#[async_trait]
impl DelegatedModelTransport for AppServerDelegatedModelTransport {
    async fn start_model_request(
        &self,
        request: DelegatedModelRequest,
    ) -> Result<mpsc::Receiver<DelegatedModelEvent>> {
        self.start_model_request_impl(request).await
    }

    async fn run_model_compact_request(
        &self,
        request: DelegatedModelCompactRequest,
    ) -> Result<Vec<codex_protocol::models::ResponseItem>> {
        self.run_model_compact_request_impl(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;
    use crate::outgoing_message::OutgoingMessageSender;
    use codex_app_server_protocol::ModelCompactEnvelope;
    use codex_app_server_protocol::ModelRequestEnvelope;
    use codex_app_server_protocol::ServerRequest;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::time::Duration;
    use tokio::time::timeout;

    fn test_request(thread_id: ThreadId) -> DelegatedModelRequest {
        DelegatedModelRequest {
            thread_id,
            turn_id: "turn-1".to_string(),
            request_id: "req-1".to_string(),
            request: ModelRequestEnvelope {
                model: "gpt-test".to_string(),
                instructions: "delegate this".to_string(),
                input: Vec::new(),
                tools: Vec::new(),
                tool_choice: "auto".to_string(),
                parallel_tool_calls: false,
                reasoning: None,
                store: false,
                stream: true,
                include: Vec::new(),
                service_tier: None,
                prompt_cache_key: None,
                text: None,
                request_headers: Some(HashMap::from([(
                    "x-codex-turn-state".to_string(),
                    "sticky-route".to_string(),
                )])),
            },
        }
    }

    fn test_compact_request(thread_id: ThreadId) -> DelegatedModelCompactRequest {
        DelegatedModelCompactRequest {
            thread_id,
            turn_id: "turn-1".to_string(),
            request_id: "compact-1".to_string(),
            request: ModelCompactEnvelope {
                model: "gpt-test".to_string(),
                input: Vec::new(),
                instructions: "compact this".to_string(),
                tools: Vec::new(),
                parallel_tool_calls: false,
                reasoning: None,
                text: None,
                request_headers: Some(HashMap::from([(
                    "session_id".to_string(),
                    "thread-123".to_string(),
                )])),
            },
        }
    }

    #[tokio::test]
    async fn delegated_request_round_trips_ack_and_notifications() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
        let thread_state_manager = ThreadStateManager::new();
        let transport = AppServerDelegatedModelTransport::new(
            Arc::clone(&outgoing),
            thread_state_manager.clone(),
        );
        let thread_id = ThreadId::new();
        let connection_id = ConnectionId(7);
        thread_state_manager
            .connection_initialized(connection_id, /*experimental_api_enabled*/ true)
            .await;
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, connection_id)
                .await
        );

        let request = test_request(thread_id);
        let start_task = tokio::spawn({
            let transport = transport.clone();
            let request = request.clone();
            async move { transport.start_model_request(request).await }
        });

        let request_id = match timeout(Duration::from_secs(1), outgoing_rx.recv())
            .await
            .expect("receive model request envelope")
            .expect("model request should be sent")
        {
            OutgoingEnvelope::ToConnection {
                connection_id: actual_connection_id,
                message:
                    OutgoingMessage::Request(ServerRequest::ModelRequest { request_id, params }),
            } => {
                assert_eq!(actual_connection_id, connection_id);
                assert_eq!(params.thread_id, thread_id.to_string());
                assert_eq!(params.turn_id, "turn-1");
                assert_eq!(params.request_id, "req-1");
                assert_eq!(params.request, request.request);
                request_id
            }
            other => panic!("expected targeted model/request, got {other:?}"),
        };

        outgoing
            .notify_client_response(
                request_id,
                json!({
                    "accepted": true,
                    "rejectionReason": null,
                }),
            )
            .await;

        let mut event_rx = start_task
            .await
            .expect("request task should join")
            .expect("delegated model request should be accepted");

        let metadata =
            HashMap::from([("x-codex-turn-state".to_string(), "sticky-route".to_string())]);
        transport
            .handle_client_notification(
                connection_id,
                ClientNotification::ModelStreamMetadata(ModelStreamMetadataNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    request_id: "req-1".to_string(),
                    metadata: metadata.clone(),
                }),
            )
            .await
            .expect("metadata notification should route");

        let delta_event = json!({
            "type": "response.output_text.delta",
            "delta": "hello",
        });
        transport
            .handle_client_notification(
                connection_id,
                ClientNotification::ModelStreamEvent(ModelStreamEventNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    request_id: "req-1".to_string(),
                    event: delta_event.clone(),
                }),
            )
            .await
            .expect("stream event notification should route");

        let completed_event = json!({
            "type": "response.completed",
            "response": { "id": "resp-1" },
        });
        transport
            .handle_client_notification(
                connection_id,
                ClientNotification::ModelStreamEvent(ModelStreamEventNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    request_id: "req-1".to_string(),
                    event: completed_event.clone(),
                }),
            )
            .await
            .expect("final stream event notification should route");

        assert_eq!(
            timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .expect("receive metadata"),
            Some(DelegatedModelEvent::StreamMetadata(metadata))
        );
        assert_eq!(
            timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .expect("receive delta event"),
            Some(DelegatedModelEvent::StreamEvent(delta_event))
        );
        assert_eq!(
            timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .expect("receive completed event"),
            Some(DelegatedModelEvent::StreamEvent(completed_event))
        );

        let err = transport
            .handle_client_notification(
                connection_id,
                ClientNotification::ModelRequestFailed(ModelRequestFailedNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    request_id: "req-1".to_string(),
                    error: codex_app_server_protocol::ModelRequestError {
                        error_type: "invalid_request_error".to_string(),
                        code: None,
                        param: None,
                        message: "too late".to_string(),
                    },
                }),
            )
            .await
            .expect_err("completed requests should be removed from inflight routing");
        assert!(
            err.to_string().contains("unknown request"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn delegated_request_requires_exactly_one_subscriber() {
        let (outgoing_tx, _outgoing_rx) = mpsc::channel(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
        let thread_state_manager = ThreadStateManager::new();
        let transport = AppServerDelegatedModelTransport::new(
            Arc::clone(&outgoing),
            thread_state_manager.clone(),
        );
        let thread_id = ThreadId::new();
        let connection_a = ConnectionId(7);
        let connection_b = ConnectionId(8);
        thread_state_manager
            .connection_initialized(connection_a, /*experimental_api_enabled*/ true)
            .await;
        thread_state_manager
            .connection_initialized(connection_b, /*experimental_api_enabled*/ true)
            .await;
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, connection_a)
                .await
        );
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, connection_b)
                .await
        );

        let err = transport
            .start_model_request(test_request(thread_id))
            .await
            .expect_err("multiple subscribers should be rejected in PR 1");
        assert!(
            err.to_string()
                .contains("exactly one model/request handler"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn delegated_compact_request_round_trips_output() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
        let thread_state_manager = ThreadStateManager::new();
        let transport = AppServerDelegatedModelTransport::new(
            Arc::clone(&outgoing),
            thread_state_manager.clone(),
        );
        let thread_id = ThreadId::new();
        let connection_id = ConnectionId(7);
        thread_state_manager
            .connection_initialized(connection_id, /*experimental_api_enabled*/ true)
            .await;
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, connection_id)
                .await
        );

        let request = test_compact_request(thread_id);
        let compact_task = tokio::spawn({
            let transport = transport.clone();
            let request = request.clone();
            async move { transport.run_model_compact_request(request).await }
        });

        let request_id = match timeout(Duration::from_secs(1), outgoing_rx.recv())
            .await
            .expect("receive model compact envelope")
            .expect("model compact request should be sent")
        {
            OutgoingEnvelope::ToConnection {
                connection_id: actual_connection_id,
                message:
                    OutgoingMessage::Request(ServerRequest::ModelCompact { request_id, params }),
            } => {
                assert_eq!(actual_connection_id, connection_id);
                assert_eq!(params.thread_id, thread_id.to_string());
                assert_eq!(params.turn_id, "turn-1");
                assert_eq!(params.request_id, "compact-1");
                assert_eq!(params.request, request.request);
                request_id
            }
            other => panic!("expected targeted model/compact, got {other:?}"),
        };

        let expected_output = vec![ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "compacted".to_string(),
            }],
            end_turn: None,
            phase: None,
        }];
        outgoing
            .notify_client_response(
                request_id,
                json!({
                    "output": expected_output.clone(),
                }),
            )
            .await;

        assert_eq!(
            compact_task
                .await
                .expect("compact task should join")
                .expect("delegated compact should succeed"),
            expected_output
        );
    }

    #[tokio::test]
    async fn delegated_request_requires_experimental_api_subscriber() {
        let (outgoing_tx, _outgoing_rx) = mpsc::channel(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
        let thread_state_manager = ThreadStateManager::new();
        let transport = AppServerDelegatedModelTransport::new(
            Arc::clone(&outgoing),
            thread_state_manager.clone(),
        );
        let thread_id = ThreadId::new();
        let connection_id = ConnectionId(7);
        thread_state_manager
            .connection_initialized(connection_id, /*experimental_api_enabled*/ false)
            .await;
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, connection_id)
                .await
        );

        let err = transport
            .start_model_request(test_request(thread_id))
            .await
            .expect_err("non-experimental subscribers should be rejected");
        assert!(
            err.to_string().contains("experimental-api client"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn delegated_request_rejects_notifications_from_unexpected_connection() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
        let thread_state_manager = ThreadStateManager::new();
        let transport = AppServerDelegatedModelTransport::new(
            Arc::clone(&outgoing),
            thread_state_manager.clone(),
        );
        let thread_id = ThreadId::new();
        let handler_connection = ConnectionId(7);
        let other_connection = ConnectionId(8);
        thread_state_manager
            .connection_initialized(handler_connection, /*experimental_api_enabled*/ true)
            .await;
        thread_state_manager
            .connection_initialized(other_connection, /*experimental_api_enabled*/ false)
            .await;
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, handler_connection)
                .await
        );
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, other_connection)
                .await
        );

        let request = test_request(thread_id);
        let start_task = tokio::spawn({
            let transport = transport.clone();
            let request = request.clone();
            async move { transport.start_model_request(request).await }
        });

        let request_id = match timeout(Duration::from_secs(1), outgoing_rx.recv())
            .await
            .expect("receive model request envelope")
            .expect("model request should be sent")
        {
            OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::Request(ServerRequest::ModelRequest { request_id, .. }),
            } => {
                assert_eq!(connection_id, handler_connection);
                request_id
            }
            other => panic!("expected targeted model/request, got {other:?}"),
        };

        outgoing
            .notify_client_response(
                request_id,
                json!({
                    "accepted": true,
                    "rejectionReason": null,
                }),
            )
            .await;

        let event_rx = start_task
            .await
            .expect("request task should join")
            .expect("delegated model request should be accepted");
        drop(event_rx);

        let err = transport
            .handle_client_notification(
                other_connection,
                ClientNotification::ModelStreamEvent(ModelStreamEventNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    request_id: "req-1".to_string(),
                    event: json!({
                        "type": "response.completed",
                        "response": { "id": "resp-1" },
                    }),
                }),
            )
            .await
            .expect_err("mismatched connection should be rejected");
        assert!(
            err.to_string().contains("unexpected connection"),
            "unexpected error: {err}"
        );
    }
}
