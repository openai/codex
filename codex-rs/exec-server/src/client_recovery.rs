use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout_at;

use super::ClientGeneration;
use super::ConnectionState;
use super::ExecServerClient;
use super::ExecServerError;
use super::Inner;
use super::disconnected_message;
use super::fail_all_in_flight_work;
use super::is_transport_closed_error;
use super::record_disconnected;
use crate::client_api::ExecServerClientConnectOptions;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::rpc::RpcClient;

const SESSION_RECOVERY_TIMEOUT: Duration = Duration::from_secs(25);
const SESSION_RECOVERY_RETRY_INTERVAL: Duration = Duration::from_millis(50);

impl Inner {
    pub(super) async fn connected_generation(
        &self,
    ) -> Result<Arc<ClientGeneration>, ExecServerError> {
        let mut state_rx = self.connection_state_tx.subscribe();
        loop {
            if let Some(error) = self.disconnected_error() {
                return Err(error);
            }

            match state_rx.borrow().clone() {
                ConnectionState::Connected(generation_id) => {
                    let generation = self.client.load_full();
                    if generation.id == generation_id && !generation.client.is_disconnected() {
                        return Ok(generation);
                    }
                }
                ConnectionState::Recovering => {}
                ConnectionState::Failed(message) => {
                    return Err(ExecServerError::Disconnected(message));
                }
            }

            state_rx.changed().await.map_err(|_| {
                ExecServerError::Disconnected(disconnected_message(/*reason*/ None))
            })?;
        }
    }

    pub(super) fn handle_generation_disconnect(
        self: &Arc<Self>,
        generation_id: u64,
        message: String,
    ) {
        let generation = self.client.load_full();
        if generation.id != generation_id {
            return;
        }

        if self.remote_connect_args.is_none() {
            let message = record_disconnected(self, message);
            self.connection_state_tx
                .send_replace(ConnectionState::Failed(message.clone()));
            let inner = Arc::clone(self);
            tokio::spawn(async move {
                fail_all_in_flight_work(&inner, message).await;
            });
            return;
        }

        let should_recover = matches!(
            self.connection_state_tx.borrow().clone(),
            ConnectionState::Connected(current_generation_id)
                if current_generation_id == generation_id
        );
        if !should_recover {
            return;
        }

        self.connection_state_tx
            .send_replace(ConnectionState::Recovering);
        let inner = Arc::clone(self);
        tokio::spawn(async move {
            inner.fail_all_http_body_streams(message.clone()).await;
            inner.recover_remote_session(generation_id, message).await;
        });
    }

    pub(super) async fn wait_for_generation_recovery(
        &self,
        failed_generation_id: u64,
    ) -> Result<(), ExecServerError> {
        let mut state_rx = self.connection_state_tx.subscribe();
        loop {
            match state_rx.borrow().clone() {
                ConnectionState::Connected(generation_id)
                    if generation_id != failed_generation_id =>
                {
                    return Ok(());
                }
                ConnectionState::Failed(message) => {
                    return Err(ExecServerError::Disconnected(message));
                }
                ConnectionState::Connected(_) | ConnectionState::Recovering => {}
            }
            state_rx.changed().await.map_err(|_| {
                ExecServerError::Disconnected(disconnected_message(/*reason*/ None))
            })?;
        }
    }

    async fn recover_remote_session(
        self: Arc<Self>,
        failed_generation_id: u64,
        disconnect_message: String,
    ) {
        let _recovery_guard = self.recovery_lock.lock().await;
        if self.disconnected.get().is_some() {
            return;
        }

        let current = self.client.load_full();
        if current.id != failed_generation_id
            && matches!(
                self.connection_state_tx.borrow().clone(),
                ConnectionState::Connected(current_generation_id)
                    if current_generation_id == current.id
            )
        {
            return;
        }

        let Some(session_id) = self
            .session_id
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        else {
            self.fail_session_recovery(
                disconnect_message,
                "the disconnected client had no initialized session id".to_string(),
            )
            .await;
            return;
        };

        let deadline = Instant::now() + SESSION_RECOVERY_TIMEOUT;
        let last_error = loop {
            match timeout_at(deadline, self.resume_once(&session_id)).await {
                Ok(Ok(generation_id)) => {
                    self.connection_state_tx
                        .send_replace(ConnectionState::Connected(generation_id));
                    return;
                }
                Ok(Err(error)) => {
                    let retry = is_retryable_recovery_error(&error);
                    if !retry {
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

        self.fail_session_recovery(disconnect_message, last_error)
            .await;
    }

    async fn resume_once(self: &Arc<Self>, session_id: &str) -> Result<u64, ExecServerError> {
        let mut connect_args = self
            .remote_connect_args
            .clone()
            .ok_or_else(|| ExecServerError::Protocol("missing reconnect arguments".to_string()))?;
        connect_args.resume_session_id = Some(session_id.to_string());
        let connection = ExecServerClient::open_websocket_connection(&connect_args).await?;
        let (rpc_client, events_rx) = RpcClient::new(connection);
        let generation_id = self.next_generation_id.fetch_add(1, Ordering::SeqCst);
        let generation = Arc::new(ClientGeneration {
            id: generation_id,
            client: rpc_client,
        });
        let stable_client = ExecServerClient {
            inner: Arc::clone(self),
        };
        stable_client.spawn_generation_reader(generation_id, events_rx);
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

        stable_client.recover_process_sessions(&generation).await?;
        if generation.client.is_disconnected() {
            return Err(ExecServerError::Closed);
        }
        self.client.store(generation);
        Ok(generation_id)
    }

    async fn fail_session_recovery(self: &Arc<Self>, disconnect_message: String, error: String) {
        let message =
            format!("{disconnect_message}; failed to resume exec-server session: {error}");
        let message = record_disconnected(self, message);
        self.connection_state_tx
            .send_replace(ConnectionState::Failed(message.clone()));
        fail_all_in_flight_work(self, message).await;
    }
}

impl ExecServerClient {
    async fn recover_process_sessions(
        &self,
        generation: &ClientGeneration,
    ) -> Result<(), ExecServerError> {
        let sessions = self.inner.sessions.load_full();
        for (process_id, session) in sessions.iter() {
            let response: ReadResponse = generation
                .client
                .call(
                    EXEC_READ_METHOD,
                    &ReadParams {
                        process_id: process_id.clone(),
                        after_seq: Some(session.last_published_seq()),
                        max_bytes: None,
                        wait_ms: Some(0),
                    },
                )
                .await?;
            let published_closed = session.recover_events(response).map_err(|error| {
                ExecServerError::Protocol(format!(
                    "failed to recover process {process_id}: {error}"
                ))
            })?;
            if published_closed {
                self.inner.remove_session(process_id).await;
            }
        }
        Ok(())
    }
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
