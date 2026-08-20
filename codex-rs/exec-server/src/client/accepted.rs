use std::sync::Arc;

use axum::extract::ws::WebSocket;
use futures::lock::Mutex;
use tokio::sync::OnceCell;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::watch;

use super::ConnectionStatus;
use super::ExecServerClient;
use super::Inner;
use super::LazyRemoteExecServerClient;
use crate::EnvironmentConnectionState;
use crate::ExecServerClientConnectOptions;
use crate::ExecServerError;
use crate::client_transport::ExecServerReconnectStrategy;
use crate::client_transport::ReconnectAttempt;
use crate::connection::JsonRpcConnection;
use codex_http_client::HttpClientFactory;

struct AcceptedReplacement {
    connection: JsonRpcConnection,
    permit: OwnedSemaphorePermit,
}

struct AcceptedConnectionSourceInner {
    replacements_tx: mpsc::UnboundedSender<AcceptedReplacement>,
    replacements_rx: Mutex<mpsc::UnboundedReceiver<AcceptedReplacement>>,
    replacement_slots: Arc<Semaphore>,
}

/// Receives authenticated connections supplied by an embedding host.
///
/// The source owns serialization and cancellation cleanup for replacement
/// handoffs. The reconnect loop only asks it for the next connection.
#[derive(Clone)]
pub(crate) struct AcceptedConnectionSource {
    inner: Arc<AcceptedConnectionSourceInner>,
    options: ExecServerClientConnectOptions,
}

struct AcceptedReplacementSubmission {
    source: AcceptedConnectionSource,
    permit: OwnedSemaphorePermit,
}

impl AcceptedConnectionSource {
    fn new(options: ExecServerClientConnectOptions) -> Self {
        let (replacements_tx, replacements_rx) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(AcceptedConnectionSourceInner {
                replacements_tx,
                replacements_rx: Mutex::new(replacements_rx),
                replacement_slots: Arc::new(Semaphore::new(1)),
            }),
            options,
        }
    }

    fn begin_replacement(&self) -> Result<AcceptedReplacementSubmission, ExecServerError> {
        let permit = Arc::clone(&self.inner.replacement_slots)
            .try_acquire_owned()
            .map_err(|_| {
                ExecServerError::Protocol(
                    "an accepted exec-server replacement is already in progress".to_string(),
                )
            })?;
        Ok(AcceptedReplacementSubmission {
            source: self.clone(),
            permit,
        })
    }

    pub(crate) async fn next_connection(
        &self,
        session_id: &str,
    ) -> Result<ReconnectAttempt, ExecServerError> {
        let replacement = self
            .inner
            .replacements_rx
            .lock()
            .await
            .recv()
            .await
            .ok_or_else(|| {
                ExecServerError::Disconnected(
                    "accepted exec-server replacement channel closed".to_string(),
                )
            })?;
        let mut options = self.options.clone();
        options.resume_session_id = Some(session_id.to_string());
        Ok(ReconnectAttempt::with_attempt_permit(
            replacement.connection,
            options,
            replacement.permit,
        ))
    }
}

impl AcceptedReplacementSubmission {
    fn submit(self, connection: JsonRpcConnection) -> Result<(), ExecServerError> {
        self.source
            .inner
            .replacements_tx
            .send(AcceptedReplacement {
                connection,
                permit: self.permit,
            })
            .map_err(|_| {
                ExecServerError::Disconnected(
                    "accepted exec-server connection is no longer awaiting replacements"
                        .to_string(),
                )
            })
    }
}

impl ExecServerClient {
    /// Initializes an exec-server client over a WebSocket accepted by an Axum handler.
    ///
    /// The caller owns accepting and authenticating replacement WebSockets.
    pub(crate) async fn connect_accepted_websocket(
        websocket: WebSocket,
        options: ExecServerClientConnectOptions,
    ) -> Result<Self, ExecServerError> {
        if options.resume_session_id.is_some() {
            return Err(ExecServerError::Protocol(
                "accepted exec-server initial connection cannot resume a session".to_string(),
            ));
        }
        let connection_source = AcceptedConnectionSource::new(options.clone());
        Self::connect_with_recovery(
            JsonRpcConnection::from_axum_websocket(
                websocket,
                "accepted exec-server websocket".to_string(),
            ),
            options,
            Some(ExecServerReconnectStrategy::Accepted(connection_source)),
        )
        .await
    }

    /// Supplies an authenticated replacement WebSocket for this accepted client.
    ///
    /// Retires the old transport before resuming the saved session. Returns
    /// after handoff; recovery continues asynchronously.
    pub(crate) async fn replace_accepted_websocket(
        &self,
        websocket: WebSocket,
    ) -> Result<(), ExecServerError> {
        self.inner
            .accept_replacement_connection(JsonRpcConnection::from_axum_websocket(
                websocket,
                "accepted exec-server replacement websocket".to_string(),
            ))
            .await
    }
}

impl Inner {
    /// Hands a replacement connection from the host to the existing accepted client.
    ///
    /// This method coordinates the handoff in this order:
    ///
    /// 1. Verify that this client uses the accepted connection source.
    /// 2. Reserve the source so concurrent handoffs are rejected.
    /// 3. If the old RPC transport is still connected, move the client into
    ///    recovery and close that transport before attaching the same session to
    ///    the replacement.
    /// 4. Queue the raw connection for the recovery task. That task creates the
    ///    new RPC client, runs the initialize/resume handshake with the saved
    ///    session ID, and recovers the existing processes.
    async fn accept_replacement_connection(
        self: &Arc<Self>,
        connection: JsonRpcConnection,
    ) -> Result<(), ExecServerError> {
        if self.session_id.get().is_none() {
            return Err(ExecServerError::Protocol(
                "accepted exec-server connection is missing its session ID".to_string(),
            ));
        }

        let Some(ExecServerReconnectStrategy::Accepted(connection_source)) =
            &self.reconnect_strategy
        else {
            return Err(ExecServerError::Protocol(
                "only an accepted exec-server connection can be replaced directly".to_string(),
            ));
        };
        let (current_rpc_client, replacement_submission) = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current_rpc_client = match &connection.status {
                ConnectionStatus::Failed(message) => {
                    return Err(ExecServerError::Disconnected(message.clone()));
                }
                ConnectionStatus::Connected(rpc_client) => Some(Arc::clone(rpc_client)),
                ConnectionStatus::Recovering => None,
            };
            let replacement_submission = connection_source.begin_replacement()?;
            (current_rpc_client, replacement_submission)
        };
        if let Some(current_rpc_client) = current_rpc_client {
            self.request_recovery(
                Arc::clone(&current_rpc_client),
                "exec-server connection replaced".to_string(),
            );
            current_rpc_client.close_transport().await;
        }
        // Synchronize the enqueue with terminal recovery. Recovery can time out
        // while the handoff is waiting for the old transport to close; in that
        // case the host must not receive success for a socket that no task will
        // consume. Holding the connection lock through the synchronous send
        // makes either the failure or the enqueue win the race unambiguously.
        let connection_state = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let ConnectionStatus::Failed(message) = &connection_state.status {
            return Err(ExecServerError::Disconnected(message.clone()));
        }
        replacement_submission.submit(connection)
    }
}

#[cfg(test)]
#[path = "accepted_tests.rs"]
mod tests;

impl LazyRemoteExecServerClient {
    pub(crate) fn from_connected(
        client: ExecServerClient,
        http_client_factory: HttpClientFactory,
    ) -> Self {
        let environment_connection_state_tx =
            watch::channel(EnvironmentConnectionState::Connected).0;
        client.attach_environment_connection_state(environment_connection_state_tx.clone());
        Self {
            transport_params: None,
            http_client_factory,
            recovery_policy: super::RecoveryPolicy::Wait,
            startup: std::sync::Arc::new(OnceCell::new_with(Some(Ok(client)))),
            current_client: std::sync::Arc::new(std::sync::Mutex::new(None)),
            reconnect: std::sync::Arc::new(std::sync::Mutex::new(None)),
            environment_connection_state_tx,
        }
    }

    pub(crate) async fn replace_accepted_websocket(
        &self,
        websocket: WebSocket,
    ) -> Result<(), ExecServerError> {
        self.get()
            .await?
            .replace_accepted_websocket(websocket)
            .await
    }
}
