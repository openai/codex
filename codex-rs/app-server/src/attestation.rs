use std::sync::Arc;
use std::sync::Weak;

use axum::http::HeaderValue;
use codex_app_server_protocol::AttestationGenerateParams;
use codex_app_server_protocol::AttestationGenerateResponse;
use codex_app_server_protocol::ServerRequestPayload;
use codex_core::AttestationContext;
use codex_core::AttestationProvider;
use codex_core::GenerateAttestationFuture;
use serde::Serialize;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout_at;
use tracing::debug;
use tracing::warn;

use crate::outgoing_message::OutgoingMessageSender;
use crate::thread_state::ThreadStateManager;

const ATTESTATION_GENERATE_TIMEOUT: Duration = Duration::from_millis(100);

pub(crate) fn app_server_attestation_provider(
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
) -> Arc<dyn AttestationProvider> {
    Arc::new(AppServerAttestationProvider {
        outgoing: Arc::downgrade(&outgoing),
        thread_state_manager,
    })
}

struct AppServerAttestationProvider {
    outgoing: Weak<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
}

impl std::fmt::Debug for AppServerAttestationProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppServerAttestationProvider")
            .finish()
    }
}

impl AttestationProvider for AppServerAttestationProvider {
    fn header_for_request(&self, context: AttestationContext) -> GenerateAttestationFuture<'_> {
        let Some(outgoing) = self.outgoing.upgrade() else {
            return Box::pin(async move {
                warn!(
                    thread_id = %context.thread_id,
                    stage = "provider",
                    reason = "outgoing_unavailable",
                    outcome = "omitted",
                    "app-server attestation provider is unavailable"
                );
                None
            });
        };
        let thread_state_manager = self.thread_state_manager.clone();
        Box::pin(async move {
            request_attestation_header_value_with_timeout(
                outgoing,
                thread_state_manager,
                context.thread_id,
                ATTESTATION_GENERATE_TIMEOUT,
            )
            .await
            .and_then(|value| match HeaderValue::from_bytes(value.as_bytes()) {
                Ok(value) => Some(value),
                Err(err) => {
                    warn!(
                        thread_id = %context.thread_id,
                        stage = "header_serialize",
                        reason = "invalid_header_value",
                        outcome = "omitted",
                        error = %err,
                        "failed to build app-server attestation header"
                    );
                    None
                }
            })
        })
    }
}

async fn request_attestation_header_value_with_timeout(
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
    thread_id: codex_protocol::ThreadId,
    timeout_duration: Duration,
) -> Option<String> {
    let deadline = Instant::now() + timeout_duration;
    let connection_id = match timeout_at(
        deadline,
        thread_state_manager.wait_for_attestation_capable_connection_for_thread(thread_id),
    )
    .await
    {
        Ok(Some(connection_id)) => connection_id,
        Ok(None) => {
            let subscriber_count = thread_state_manager
                .subscribed_connection_ids(thread_id)
                .await
                .len();
            debug!(
                thread_id = %thread_id,
                stage = "capability_lookup",
                reason = "no_live_capable_connection",
                outcome = "omitted",
                subscriber_count,
                "no live client supports attestation generation"
            );
            return None;
        }
        Err(_) => {
            let subscriber_count = thread_state_manager
                .subscribed_connection_ids(thread_id)
                .await
                .len();
            warn!(
                thread_id = %thread_id,
                stage = "capability_lookup",
                reason = "no_capable_thread_subscriber",
                outcome = "omitted",
                subscriber_count,
                timeout_ms = timeout_duration.as_millis(),
                "timed out waiting for an attestation-capable client to subscribe"
            );
            return None;
        }
    };

    let connection_ids = [connection_id];
    let (request_id, rx) = outgoing
        .send_request_to_connections(
            Some(&connection_ids),
            ServerRequestPayload::AttestationGenerate(AttestationGenerateParams {}),
            /*thread_id*/ None,
        )
        .await;
    debug!(
        thread_id = %thread_id,
        connection_id = connection_id.0,
        request_id = ?request_id,
        stage = "rpc_dispatch",
        reason = "capable_connection_selected",
        "dispatched attestation generation request"
    );

    let result = match timeout_at(deadline, rx).await {
        Ok(Ok(Ok(result))) => result,
        Ok(Ok(Err(err))) => {
            warn!(
                thread_id = %thread_id,
                connection_id = connection_id.0,
                request_id = ?request_id,
                stage = "rpc_response",
                reason = "request_failed",
                outcome = "error_header",
                code = err.code,
                message = %err.message,
                "attestation generation request failed"
            );
            return app_server_attestation_header_value(
                AppServerAttestationStatus::RequestFailed,
                /*token*/ None,
            );
        }
        Ok(Err(err)) => {
            warn!(
                thread_id = %thread_id,
                connection_id = connection_id.0,
                request_id = ?request_id,
                stage = "rpc_response",
                reason = "request_canceled",
                outcome = "error_header",
                error = %err,
                "attestation generation request canceled"
            );
            return app_server_attestation_header_value(
                AppServerAttestationStatus::RequestCanceled,
                /*token*/ None,
            );
        }
        Err(_) => {
            let _canceled = outgoing.cancel_request(&request_id).await;
            warn!(
                thread_id = %thread_id,
                connection_id = connection_id.0,
                request_id = ?request_id,
                stage = "rpc_response",
                reason = "timeout",
                outcome = "error_header",
                timeout_ms = timeout_duration.as_millis(),
                "attestation generation request timed out"
            );
            return app_server_attestation_header_value(
                AppServerAttestationStatus::Timeout,
                /*token*/ None,
            );
        }
    };

    match serde_json::from_value::<AttestationGenerateResponse>(result) {
        Ok(response) => {
            debug!(
                thread_id = %thread_id,
                connection_id = connection_id.0,
                request_id = ?request_id,
                stage = "rpc_response",
                reason = "ok",
                outcome = "header",
                "received attestation generation response"
            );
            app_server_attestation_header_value(
                AppServerAttestationStatus::Ok,
                Some(&response.token),
            )
        }
        Err(err) => {
            warn!(
                thread_id = %thread_id,
                connection_id = connection_id.0,
                request_id = ?request_id,
                stage = "rpc_response",
                reason = "malformed_response",
                outcome = "error_header",
                error = %err,
                "failed to deserialize attestation generation response"
            );
            app_server_attestation_header_value(
                AppServerAttestationStatus::MalformedResponse,
                /*token*/ None,
            )
        }
    }
}

#[derive(Clone, Copy)]
enum AppServerAttestationStatus {
    Ok,
    Timeout,
    RequestFailed,
    RequestCanceled,
    MalformedResponse,
}

impl AppServerAttestationStatus {
    const fn code(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Timeout => 1,
            Self::RequestFailed => 2,
            Self::RequestCanceled => 3,
            Self::MalformedResponse => 4,
        }
    }
}

#[derive(Serialize)]
struct AppServerAttestationEnvelope<'a> {
    v: u8,
    s: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    t: Option<&'a str>,
}

fn app_server_attestation_header_value(
    status: AppServerAttestationStatus,
    token: Option<&str>,
) -> Option<String> {
    serde_json::to_string(&AppServerAttestationEnvelope {
        v: 1,
        s: status.code(),
        t: token,
    })
    .map_err(|err| {
        warn!(
            stage = "header_serialize",
            reason = "json_serialize_failed",
            outcome = "omitted",
            error = %err,
            "failed to serialize app-server attestation envelope"
        )
    })
    .ok()
}

#[cfg(test)]
mod tests {
    use super::AppServerAttestationStatus;
    use super::app_server_attestation_header_value;
    use super::app_server_attestation_provider;
    use super::request_attestation_header_value_with_timeout;
    use crate::outgoing_message::ConnectionId;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;
    use crate::outgoing_message::OutgoingMessageSender;
    use crate::thread_state::ConnectionCapabilities;
    use crate::thread_state::ThreadStateManager;
    use codex_app_server_protocol::ServerRequest;
    use codex_core::AttestationContext;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn app_server_attestation_header_value_wraps_opaque_client_payloads() {
        assert_eq!(
            app_server_attestation_header_value(
                AppServerAttestationStatus::Ok,
                Some("v1.opaque-client-payload"),
            ),
            Some(r#"{"v":1,"s":0,"t":"v1.opaque-client-payload"}"#.to_string())
        );
    }

    #[test]
    fn app_server_attestation_header_value_reports_app_server_failures() {
        assert_eq!(
            app_server_attestation_header_value(
                AppServerAttestationStatus::Timeout,
                /*token*/ None,
            ),
            Some(r#"{"v":1,"s":1}"#.to_string())
        );
        assert_eq!(
            app_server_attestation_header_value(
                AppServerAttestationStatus::RequestFailed,
                /*token*/ None,
            ),
            Some(r#"{"v":1,"s":2}"#.to_string())
        );
        assert_eq!(
            app_server_attestation_header_value(
                AppServerAttestationStatus::RequestCanceled,
                /*token*/ None,
            ),
            Some(r#"{"v":1,"s":3}"#.to_string())
        );
        assert_eq!(
            app_server_attestation_header_value(
                AppServerAttestationStatus::MalformedResponse,
                /*token*/ None
            ),
            Some(r#"{"v":1,"s":4}"#.to_string())
        );
    }

    #[tokio::test]
    async fn unavailable_provider_is_omitted() {
        let (outgoing_tx, _outgoing_rx) = mpsc::channel(1);
        let provider = app_server_attestation_provider(
            Arc::new(OutgoingMessageSender::new(
                outgoing_tx,
                codex_analytics::AnalyticsEventsClient::disabled(),
            )),
            ThreadStateManager::new(),
        );

        let header = provider
            .header_for_request(AttestationContext {
                thread_id: ThreadId::new(),
            })
            .await;

        assert_eq!(header, None);
    }

    #[tokio::test]
    async fn attestation_without_thread_subscriber_is_omitted() {
        let (outgoing_tx, _outgoing_rx) = mpsc::channel(1);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            codex_analytics::AnalyticsEventsClient::disabled(),
        ));
        let thread_state_manager = ThreadStateManager::new();
        let thread_id = ThreadId::new();

        let header = request_attestation_header_value_with_timeout(
            outgoing,
            thread_state_manager,
            thread_id,
            Duration::from_millis(1),
        )
        .await;

        assert_eq!(header, None);
    }

    #[tokio::test]
    async fn attestation_waits_for_capable_thread_subscriber() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(1);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            codex_analytics::AnalyticsEventsClient::disabled(),
        ));
        let thread_state_manager = ThreadStateManager::new();
        let thread_id = ThreadId::new();
        let connection_id = ConnectionId(7);
        thread_state_manager
            .connection_initialized(
                connection_id,
                ConnectionCapabilities {
                    request_attestation: true,
                },
            )
            .await;

        let request_header = request_attestation_header_value_with_timeout(
            Arc::clone(&outgoing),
            thread_state_manager.clone(),
            thread_id,
            Duration::from_secs(1),
        );
        let respond = attach_capable_connection_and_respond(
            &thread_state_manager,
            thread_id,
            connection_id,
            outgoing.as_ref(),
            &mut outgoing_rx,
        );

        let (header, ()) = tokio::join!(request_header, respond);
        assert_eq!(
            header,
            Some(r#"{"v":1,"s":0,"t":"v1.integration-test"}"#.to_string())
        );
    }

    #[tokio::test]
    async fn attestation_waits_past_unsupported_subscriber_for_capable_subscriber() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(1);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            codex_analytics::AnalyticsEventsClient::disabled(),
        ));
        let thread_state_manager = ThreadStateManager::new();
        let thread_id = ThreadId::new();
        let unsupported_connection_id = ConnectionId(7);
        let capable_connection_id = ConnectionId(8);
        thread_state_manager
            .connection_initialized(unsupported_connection_id, ConnectionCapabilities::default())
            .await;
        thread_state_manager
            .connection_initialized(
                capable_connection_id,
                ConnectionCapabilities {
                    request_attestation: true,
                },
            )
            .await;
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, unsupported_connection_id)
                .await
        );

        let request_header = request_attestation_header_value_with_timeout(
            Arc::clone(&outgoing),
            thread_state_manager.clone(),
            thread_id,
            Duration::from_secs(1),
        );
        let respond = attach_capable_connection_and_respond(
            &thread_state_manager,
            thread_id,
            capable_connection_id,
            outgoing.as_ref(),
            &mut outgoing_rx,
        );

        let (header, ()) = tokio::join!(request_header, respond);
        assert_eq!(
            header,
            Some(r#"{"v":1,"s":0,"t":"v1.integration-test"}"#.to_string())
        );
    }

    #[tokio::test]
    async fn attestation_with_unsupported_thread_subscriber_is_omitted() {
        let (outgoing_tx, _outgoing_rx) = mpsc::channel(1);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            codex_analytics::AnalyticsEventsClient::disabled(),
        ));
        let thread_state_manager = ThreadStateManager::new();
        let thread_id = ThreadId::new();
        let connection_id = ConnectionId(7);
        thread_state_manager
            .connection_initialized(connection_id, ConnectionCapabilities::default())
            .await;
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, connection_id)
                .await
        );

        let header = request_attestation_header_value_with_timeout(
            outgoing,
            thread_state_manager,
            thread_id,
            Duration::from_secs(1),
        )
        .await;

        assert_eq!(header, None);
    }

    async fn attach_capable_connection_and_respond(
        thread_state_manager: &ThreadStateManager,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        outgoing: &OutgoingMessageSender,
        outgoing_rx: &mut mpsc::Receiver<OutgoingEnvelope>,
    ) {
        tokio::task::yield_now().await;
        assert!(
            thread_state_manager
                .try_add_connection_to_thread(thread_id, connection_id)
                .await
        );
        let envelope = timeout(Duration::from_secs(1), outgoing_rx.recv())
            .await
            .expect("timed out waiting for attestation request")
            .expect("outgoing channel closed before attestation request");
        let OutgoingEnvelope::ToConnection {
            connection_id: target_connection_id,
            message: OutgoingMessage::Request(ServerRequest::AttestationGenerate { request_id, .. }),
            ..
        } = envelope
        else {
            panic!("expected targeted attestation request");
        };
        assert_eq!(target_connection_id, connection_id);
        outgoing
            .notify_client_response(
                request_id,
                serde_json::json!({ "token": "v1.integration-test" }),
            )
            .await;
    }
}
