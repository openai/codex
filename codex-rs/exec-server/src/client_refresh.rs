//! Explicit connection refresh after a planned executor replacement.
//!
//! Ordinary recovery tries to resume the same executor session after a transient
//! disconnect. Replacement needs a fresh session, without waiting for old recovery
//! to give up. The caller supplies the ordering: register the replacement first,
//! then refresh. Executor identity stays inside this connection layer.
//!
//! Flow: fresh registry lookup -> reuse or retire session -> connect if needed ->
//! live status probe. The lazy client and its `Environment` remain the same objects;
//! only the underlying `ExecServerClient` may change. The public caller contract is on
//! `Environment::refresh_connection`.
//!
//! Two races determine the synchronization here. A client installed during the
//! lookup makes that lookup stale, so refresh checks again. A connection attempt
//! cancelled by refresh must never install later. Cancellation and installation
//! synchronize on the `current_client` lock; acquire `reconnect` first when both are needed.
//! `refresh_lock` serializes only explicit refreshes; ordinary connection and recovery
//! work can continue concurrently. Retired sessions cannot publish environment state.

use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use futures::future::BoxFuture;
use tokio::sync::OnceCell;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use super::ConnectionResult;
use super::ConnectionStatus;
use super::ExecServerClient;
use super::ExecServerError;
use super::Inner;
use super::LazyRemoteExecServerClient;
use super::fail_all_in_flight_work;
use crate::EnvironmentConnectionState;
use crate::NoiseChannelPublicKey;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_api::ExecServerTransportParams;
use crate::client_api::NoiseRendezvousConnectBundle;
use crate::client_api::NoiseRendezvousConnectProvider;
use crate::client_transport::ExecServerReconnectStrategy;

/// Shared startup/reconnect result plus cancellation for work superseded by refresh.
/// The optional transport carries a refresh lookup's bundle into the normal connector.
#[derive(Default)]
pub(super) struct ConnectionAttempt {
    pub(super) result: OnceCell<ConnectionResult>,
    pub(super) cancelled: CancellationToken,
    pub(super) transport: Option<ExecServerTransportParams>,
}

// Use the compared bundle intact for the first connection: address, key and authorization
// belong together. Later lookups, including authorization refresh, use the real provider.
struct PrefetchedConnectProvider {
    bundle: StdMutex<Option<NoiseRendezvousConnectBundle>>,
    provider: Arc<dyn NoiseRendezvousConnectProvider>,
}

impl NoiseRendezvousConnectProvider for PrefetchedConnectProvider {
    fn connect_bundle(
        &self,
        harness_public_key: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        Box::pin(async move {
            let bundle = self
                .bundle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            match bundle {
                Some(bundle) => Ok(bundle),
                None => self.provider.connect_bundle(harness_public_key).await,
            }
        })
    }
}

impl LazyRemoteExecServerClient {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "serialize explicit refreshes, not ordinary connection or recovery attempts"
    )]
    pub(crate) async fn refresh_connection(&self) -> Result<(), ExecServerError> {
        let _refresh = self.refresh_lock.lock().await;
        let (previous, attempt) = loop {
            let observed = self.cached_client();
            let observed_registration = observed
                .as_ref()
                .and_then(ExecServerClient::executor_registration_id);
            let mut transport = self.transport_params.clone().ok_or_else(|| {
                ExecServerError::Protocol(
                    "connection refresh requires a Noise registry".to_string(),
                )
            })?;
            let target = match &mut transport {
                ExecServerTransportParams::Deferred(deferred) => &mut deferred.transport,
                transport => transport,
            };
            let ExecServerTransportParams::NoiseRendezvous { provider, identity } = target else {
                return Err(ExecServerError::Protocol(
                    "connection refresh requires a Noise registry".to_string(),
                ));
            };
            // This lookup is independent of the old session and its recovery deadline.
            let bundle = timeout(
                DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
                provider.connect_bundle(identity.public_key()),
            )
            .await
            .map_err(|_| {
                ExecServerError::EnvironmentRegistryRequest(
                    codex_http_client::RouteAwareRequestError::Timeout,
                )
            })??;
            let executor_public_key = bundle.executor_public_key.clone();
            let executor_registration_id = bundle.executor_registration_id.clone();
            *provider = Arc::new(PrefetchedConnectProvider {
                bundle: StdMutex::new(Some(bundle)),
                provider: Arc::clone(provider),
            });

            let mut reconnect = self
                .reconnect
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current = self
                .current_client
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // A new client makes the lookup stale. Recovery can also renew the
            // registration on the same client, so compare it under the connection lock.
            if !match (&observed, &*current) {
                (Some(observed), Some(current)) => Arc::ptr_eq(&observed.inner, &current.inner),
                (None, None) => true,
                _ => false,
            } {
                continue;
            }
            if let Some(client) = current.as_ref() {
                let connection = client
                    .inner
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if connection.executor_registration_id != observed_registration {
                    continue;
                }
                // Preserve same-executor recovery; the final probe fails fast if still recovering.
                if !matches!(connection.status, ConnectionStatus::Failed(_))
                    && connection.executor_registration_id.as_ref()
                        == Some(&executor_registration_id)
                    && matches!(
                        client.inner.reconnect_strategy.as_ref(),
                        Some(ExecServerReconnectStrategy::NoiseRendezvous {
                            executor_public_key: key, ..
                        }) if key == &executor_public_key
                    )
                {
                    break (current.clone(), None);
                }
                // Claim retirement before recovery can install another registration.
                // Transport shutdown and pending-work cleanup happen outside these locks.
                client.inner.retired.cancel();
            }
            // Cancellation and connection installation use the same lock. A late
            // handshake cannot install a client after its attempt has been superseded.
            self.startup.cancelled.cancel();
            if let Some(attempt) = reconnect.as_ref() {
                attempt.cancelled.cancel();
            }
            self.environment_connection_state_tx
                .send_replace(EnvironmentConnectionState::Disconnected);
            let attempt = Arc::new(ConnectionAttempt {
                transport: Some(transport),
                ..Default::default()
            });
            *reconnect = Some(Arc::clone(&attempt));
            break (current.clone(), Some(attempt));
        };
        let client = match attempt {
            Some(attempt) => {
                if let Some(previous) = previous {
                    previous.inner.retire().await;
                }
                let result = attempt
                    .result
                    .get_or_init(|| self.connect_once(&attempt))
                    .await
                    .clone();
                let mut reconnect = self
                    .reconnect
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if reconnect
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &attempt))
                {
                    *reconnect = None;
                }
                result.map_err(ExecServerError::ConnectionAttempt)?
            }
            None => previous.ok_or_else(|| {
                ExecServerError::Protocol("current executor session is missing".to_string())
            })?,
        };
        // Metadata may be cached; readiness requires a live, non-recovering probe.
        client.environment_status().await.map(drop)
    }

    #[tracing::instrument(name = "codex.exec_server.remote.connect", skip_all)]
    pub(super) fn connect_once<'a>(
        &'a self,
        attempt: &'a ConnectionAttempt,
    ) -> BoxFuture<'a, ConnectionResult> {
        // Keep the transport future out of every caller's async layout, including
        // the CLI entry point, which otherwise exceeds rustc's query-depth limit.
        Box::pin(async move {
            let transport = attempt
                .transport
                .as_ref()
                .or(self.transport_params.as_ref())
                .ok_or_else(|| {
                    Arc::new(ExecServerError::Protocol(
                        "missing transport params for lazy exec-server connection".to_string(),
                    ))
                })?;
            let client = tokio::select! {
                biased;
                _ = attempt.cancelled.cancelled() => return Err(Arc::new(ExecServerError::Disconnected("connection attempt was superseded".to_string()))),
                result = ExecServerClient::connect_for_transport(transport.clone(), self.http_client_factory.clone()) => result.map_err(Arc::new)?,
            };
            // Cancellation can race with a completed handshake. Recheck before attaching
            // state or installing the client, under the same lock used by refresh.
            {
                let mut current = self
                    .current_client
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !attempt.cancelled.is_cancelled() {
                    client.attach_environment_connection_state(
                        self.environment_connection_state_tx.clone(),
                    );
                    *current = Some(client.clone());
                    return Ok(client);
                }
            }
            client.inner.retire().await;
            Err(Arc::new(ExecServerError::Disconnected(
                "connection attempt was superseded".to_string(),
            )))
        })
    }
}

impl Inner {
    async fn retire(self: &Arc<Self>) {
        let message = "exec-server executor was replaced".to_string();
        let rpc_client = {
            let mut connection = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Detach before a later transport completion can publish stale state.
            connection.environment_connection_state_tx =
                watch::channel(EnvironmentConnectionState::Disconnected).0;
            let rpc_client = match &connection.status {
                ConnectionStatus::Connected(client) => Some(Arc::clone(client)),
                ConnectionStatus::Recovering | ConnectionStatus::Failed(_) => None,
            };
            self.retired.cancel();
            connection.set_status(ConnectionStatus::Failed(message.clone()));
            rpc_client
        };
        self.connection_changed.send_replace(());
        // Drain pending RPCs before stream cleanup, which may wait for other work.
        if let Some(rpc_client) = rpc_client {
            rpc_client.close_transport().await;
        }
        fail_all_in_flight_work(self, message).await;
    }
}

#[cfg(test)]
#[path = "client_refresh_tests.rs"]
mod tests;
