use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout_at;

use super::ClientGeneration;
use super::ExecServerClient;
use super::ExecServerError;
use super::Inner;
use super::TransportState;
use super::fail_all_in_flight_work;
use super::is_transport_closed_error;
use crate::client_api::ExecServerClientConnectOptions;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::rpc::RpcClient;

const SESSION_RECOVERY_TIMEOUT: Duration = Duration::from_secs(25);
const SESSION_RECOVERY_RETRY_INTERVAL: Duration = Duration::from_millis(50);

impl Inner {
    pub(super) fn handle_generation_disconnect(
        self: &Arc<Self>,
        generation: Arc<ClientGeneration>,
        message: String,
    ) {
        if !generation.mark_terminal() {
            return;
        }

        generation.client.close_transport();
        let inner = Arc::clone(self);
        tokio::spawn(async move {
            generation.client.close().await;
            if inner.remote_connect_args.is_some() {
                inner.recover_remote_session(generation, message).await;
            } else {
                inner.fail_generation(generation.id, message).await;
            }
        });
    }

    async fn fail_generation(&self, generation_id: u64, message: String) {
        let mut state = self.transport.write().await;
        if !is_current_generation(&state, generation_id) {
            return;
        }

        *state = TransportState::Failed(message.clone());
        fail_all_in_flight_work(self, message).await;
    }

    async fn recover_remote_session(
        self: Arc<Self>,
        failed_generation: Arc<ClientGeneration>,
        disconnect_message: String,
    ) {
        let mut state = self.transport.write().await;
        if !is_current_generation(&state, failed_generation.id) {
            return;
        }

        // The write lease starts after all operations using the failed
        // generation have finished. It also prevents new process and HTTP
        // routes from being registered until promotion completes.
        self.fail_all_http_body_streams(disconnect_message.clone())
            .await;

        let session_id = self.session_id.clone();
        let deadline = Instant::now() + SESSION_RECOVERY_TIMEOUT;
        let last_error = loop {
            match timeout_at(deadline, self.resume_once(&session_id)).await {
                Ok(Ok(generation)) if !generation.is_terminal() => {
                    *state = TransportState::Connected(generation);
                    return;
                }
                Ok(Ok(generation)) => {
                    generation.client.close().await;
                }
                Ok(Err(error)) => {
                    if !is_retryable_recovery_error(&error) {
                        break error.to_string();
                    }
                }
                Err(_) => {
                    break format!("recovery timed out after {SESSION_RECOVERY_TIMEOUT:?}");
                }
            }

            let now = Instant::now();
            if now >= deadline {
                break format!("recovery timed out after {SESSION_RECOVERY_TIMEOUT:?}");
            }
            sleep(SESSION_RECOVERY_RETRY_INTERVAL.min(deadline - now)).await;
        };

        let message =
            format!("{disconnect_message}; failed to resume exec-server session: {last_error}");
        *state = TransportState::Failed(message.clone());
        fail_all_in_flight_work(&self, message).await;
    }

    async fn resume_once(
        self: &Arc<Self>,
        session_id: &str,
    ) -> Result<Arc<ClientGeneration>, ExecServerError> {
        let mut connect_args = self
            .remote_connect_args
            .clone()
            .ok_or_else(|| ExecServerError::Protocol("missing reconnect arguments".to_string()))?;
        connect_args.resume_session_id = Some(session_id.to_string());
        let connection = ExecServerClient::open_websocket_connection(&connect_args).await?;
        let (rpc_client, events_rx) = RpcClient::new(connection);
        let generation = Arc::new(ClientGeneration::new(
            self.next_generation_id.fetch_add(1, Ordering::SeqCst),
            rpc_client,
        ));
        let stable_client = ExecServerClient {
            inner: Arc::clone(self),
        };
        stable_client.spawn_generation_reader(Arc::clone(&generation), events_rx);

        let result = async {
            let response = ExecServerClient::initialize_generation(
                &generation,
                ExecServerClientConnectOptions {
                    client_name: connect_args.client_name,
                    initialize_timeout: connect_args.initialize_timeout,
                    resume_session_id: connect_args.resume_session_id,
                },
            )
            .await?;
            if response.session_id != session_id {
                return Err(ExecServerError::Protocol(format!(
                    "exec-server resumed session {session_id} as unexpected session {}",
                    response.session_id
                )));
            }

            stable_client.recover_process_sessions(&generation).await
        }
        .await;

        let error = match result {
            Ok(()) if !generation.is_terminal() => return Ok(generation),
            Ok(()) => ExecServerError::Closed,
            Err(error) => error,
        };
        generation.mark_terminal();
        generation.client.close().await;
        Err(error)
    }
}

impl ExecServerClient {
    async fn recover_process_sessions(
        &self,
        generation: &ClientGeneration,
    ) -> Result<(), ExecServerError> {
        let sessions = self
            .inner
            .sessions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for (process_id, session) in sessions.iter() {
            let Some(current_session) = self.inner.get_session(process_id) else {
                continue;
            };
            if !Arc::ptr_eq(session, &current_session) {
                continue;
            }

            let response = generation
                .client
                .call::<_, ReadResponse>(
                    EXEC_READ_METHOD,
                    &ReadParams {
                        process_id: process_id.clone(),
                        after_seq: Some(session.last_published_seq()),
                        max_bytes: None,
                        wait_ms: Some(0),
                    },
                )
                .await
                .map_err(ExecServerError::from);
            let recovered = match response {
                Ok(response) => session.recover_events(response),
                Err(error) if is_transport_closed_error(&error) => return Err(error),
                Err(error) => Err(error),
            };
            match recovered {
                Ok(published_closed) => {
                    if published_closed {
                        self.inner.remove_session(process_id);
                    }
                }
                Err(error) => {
                    let message = format!("failed to recover process {process_id}: {error}");
                    self.inner.remove_session(process_id);
                    session.set_failure(message).await;
                }
            }
        }
        Ok(())
    }
}

fn is_current_generation(state: &TransportState, generation_id: u64) -> bool {
    matches!(
        state,
        TransportState::Connected(generation) if generation.id == generation_id
    )
}

fn is_retryable_recovery_error(error: &ExecServerError) -> bool {
    is_transport_closed_error(error)
        || matches!(
            error,
            ExecServerError::WebSocketConnectTimeout { .. }
                | ExecServerError::WebSocketConnect { .. }
                | ExecServerError::InitializeTimedOut { .. }
        )
        || matches!(
            error,
            ExecServerError::Server { message, .. }
                if message.contains("is already attached to another connection")
        )
}
