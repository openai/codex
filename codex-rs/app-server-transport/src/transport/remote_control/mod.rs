mod client_tracker;
mod enroll;
mod pairing;
mod protocol;
mod segment;
mod websocket;

use self::pairing::RemoteControlPairingClient;
use crate::transport::remote_control::websocket::RemoteControlChannels;
use crate::transport::remote_control::websocket::RemoteControlStatusPublisher;
use crate::transport::remote_control::websocket::RemoteControlWebsocket;

pub use self::protocol::ClientId;
use self::protocol::ServerEvent;
use self::protocol::StreamId;
use self::protocol::normalize_remote_control_url;
use super::CHANNEL_CAPACITY;
use super::TransportEvent;
use super::next_connection_id;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_app_server_protocol::RemoteControlStatusChangedNotification;
use codex_login::AuthManager;
use codex_state::StateRuntime;
use futures::FutureExt;
use gethostname::gethostname;
use std::error::Error;
use std::fmt;
use std::io;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

pub struct RemoteControlStartConfig {
    pub remote_control_url: String,
    pub installation_id: String,
}

pub(super) struct QueuedServerEnvelope {
    pub(super) event: ServerEvent,
    pub(super) client_id: ClientId,
    pub(super) stream_id: StreamId,
    pub(super) write_complete_tx: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
pub struct RemoteControlHandle {
    enabled_tx: Arc<watch::Sender<bool>>,
    status_tx: Arc<watch::Sender<RemoteControlStatusChangedNotification>>,
    state_db_available: bool,
    pairing: PairingClientState,
    pairing_request_cancellation_revision: Arc<AtomicU64>,
    pairing_refresh_tx: Arc<watch::Sender<u64>>,
    auth_change_rx: Arc<StdMutex<watch::Receiver<u64>>>,
}

// `/server/pair` runs outside the websocket task, so keep the auth and disable
// revisions that make its eventual response safe to return to app-server.
struct PairingRequestContext {
    auth_change_revision: u64,
    cancellation_revision: u64,
    pairing_client: RemoteControlPairingClient,
}

#[derive(Clone)]
pub(super) struct PairingClientState {
    client: Arc<StdMutex<Option<RemoteControlPairingClient>>>,
    generation: Arc<AtomicU64>,
}

impl PairingClientState {
    fn new() -> Self {
        Self {
            client: Arc::new(StdMutex::new(None)),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteControlUnavailable;

impl fmt::Display for RemoteControlUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "remote control cannot be enabled because sqlite state db is unavailable"
        )
    }
}

impl Error for RemoteControlUnavailable {}

impl RemoteControlHandle {
    pub fn enable(
        &self,
    ) -> Result<RemoteControlStatusChangedNotification, RemoteControlUnavailable> {
        if !self.state_db_available {
            warn!("remote control cannot be enabled because sqlite state db is unavailable");
            return Err(RemoteControlUnavailable);
        }

        let enabled_changed = self.enabled_tx.send_if_modified(|state| {
            let changed = !*state;
            *state = true;
            changed
        });

        let status = self.status();
        info!(
            enabled_changed,
            current_status = ?status.status,
            environment_id = ?status.environment_id,
            installation_id = %status.installation_id,
            server_name = %status.server_name,
            "remote control enable requested"
        );
        if matches!(
            status.status,
            RemoteControlConnectionStatus::Connected | RemoteControlConnectionStatus::Connecting
        ) {
            return Ok(status);
        }

        Ok(self.publish_status(RemoteControlConnectionStatus::Connecting))
    }

    pub fn disable(&self) -> RemoteControlStatusChangedNotification {
        let enabled_changed = self.enabled_tx.send_if_modified(|state| {
            let changed = *state;
            *state = false;
            changed
        });
        clear_pairing_client(&self.pairing);
        self.cancel_pending_pairing_requests();

        let status = self.status();
        info!(
            enabled_changed,
            current_status = ?status.status,
            environment_id = ?status.environment_id,
            installation_id = %status.installation_id,
            server_name = %status.server_name,
            "remote control disable requested"
        );
        self.publish_status(RemoteControlConnectionStatus::Disabled)
    }

    pub fn cancel_pending_pairing_requests(&self) {
        self.pairing_request_cancellation_revision
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn status(&self) -> RemoteControlStatusChangedNotification {
        self.status_tx.borrow().clone()
    }

    pub fn status_receiver(&self) -> watch::Receiver<RemoteControlStatusChangedNotification> {
        self.status_tx.subscribe()
    }

    pub async fn start_pairing(
        &self,
        params: RemoteControlPairingStartParams,
    ) -> io::Result<RemoteControlPairingStartResponse> {
        let pairing_request = self.pairing_request_context()?;
        let pairing_response = pairing_request
            .pairing_client
            .start(protocol::StartRemoteControlPairingRequest {
                manual_code: params.manual_code,
            })
            .await;
        self.refresh_pairing_auth_after_rejection(
            &pairing_request.pairing_client,
            &pairing_response,
        );
        self.validate_pairing_response(&pairing_request, &pairing_response)?;
        pairing_response
    }

    fn pairing_request_context(&self) -> io::Result<PairingRequestContext> {
        if !*self.enabled_tx.borrow() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote control pairing requires remote control to be enabled",
            ));
        }
        if self.status().status != RemoteControlConnectionStatus::Connected {
            return Err(Self::pairing_unavailable_error());
        }

        let auth_change_revision = self.auth_change_revision();
        let pairing_client = {
            let mut pairing_client = self
                .pairing
                .client
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current_pairing_client = pairing_client
                .as_ref()
                .filter(|pairing_client| {
                    pairing_client.matches_auth_change_revision(auth_change_revision)
                })
                .cloned();
            if current_pairing_client.is_none() {
                *pairing_client = None;
            }
            current_pairing_client
        }
        .ok_or_else(Self::pairing_unavailable_error)?;

        Ok(PairingRequestContext {
            auth_change_revision,
            cancellation_revision: self
                .pairing_request_cancellation_revision
                .load(Ordering::Relaxed),
            pairing_client,
        })
    }

    fn refresh_pairing_auth_after_rejection(
        &self,
        pairing_client: &RemoteControlPairingClient,
        pairing_response: &io::Result<RemoteControlPairingStartResponse>,
    ) {
        if pairing_response.as_ref().is_err_and(|err| {
            matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            )
        }) && clear_pairing_client_if_current(&self.pairing, pairing_client)
        {
            self.request_pairing_auth_refresh();
        }
    }

    fn validate_pairing_response(
        &self,
        pairing_request: &PairingRequestContext,
        pairing_response: &io::Result<RemoteControlPairingStartResponse>,
    ) -> io::Result<()> {
        if self.auth_change_revision() != pairing_request.auth_change_revision {
            return Err(Self::pairing_unavailable_error());
        }
        if pairing_response.is_ok()
            && self
                .pairing_request_cancellation_revision
                .load(Ordering::Relaxed)
                != pairing_request.cancellation_revision
        {
            return Err(Self::pairing_unavailable_error());
        }
        Ok(())
    }

    fn auth_change_revision(&self) -> u64 {
        *self
            .auth_change_rx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .borrow()
    }

    fn pairing_unavailable_error() -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "remote control pairing is unavailable until enrollment completes",
        )
    }

    fn request_pairing_auth_refresh(&self) {
        self.pairing_refresh_tx
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }

    fn publish_status(
        &self,
        connection_status: RemoteControlConnectionStatus,
    ) -> RemoteControlStatusChangedNotification {
        let mut status_change = None;
        self.status_tx.send_if_modified(|status| {
            let next_status =
                remote_control_status_with_connection_status(status, connection_status);
            if *status == next_status {
                return false;
            }

            status_change = Some((status.clone(), next_status.clone()));
            *status = next_status;
            true
        });
        if let Some((previous_status, next_status)) = status_change {
            info!(
                previous_status = ?previous_status.status,
                next_status = ?next_status.status,
                previous_environment_id = ?previous_status.environment_id,
                next_environment_id = ?next_status.environment_id,
                installation_id = %next_status.installation_id,
                server_name = %next_status.server_name,
                "remote control handle status changed"
            );
        }
        self.status()
    }
}

fn remote_control_status_with_connection_status(
    status: &RemoteControlStatusChangedNotification,
    connection_status: RemoteControlConnectionStatus,
) -> RemoteControlStatusChangedNotification {
    RemoteControlStatusChangedNotification {
        status: connection_status,
        server_name: status.server_name.clone(),
        installation_id: status.installation_id.clone(),
        environment_id: if connection_status == RemoteControlConnectionStatus::Disabled {
            None
        } else {
            status.environment_id.clone()
        },
    }
}

fn clear_pairing_client(pairing: &PairingClientState) {
    *pairing
        .client
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    pairing.generation.fetch_add(1, Ordering::Relaxed);
}

fn clear_pairing_client_if_current(
    pairing: &PairingClientState,
    expected_pairing_client: &RemoteControlPairingClient,
) -> bool {
    let mut pairing_client = pairing
        .client
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if pairing_client
        .as_ref()
        .is_none_or(|pairing_client| !pairing_client.matches_pairing_auth(expected_pairing_client))
    {
        return false;
    }
    *pairing_client = None;
    pairing.generation.fetch_add(1, Ordering::Relaxed);
    true
}

pub async fn start_remote_control(
    config: RemoteControlStartConfig,
    state_db: Option<Arc<StateRuntime>>,
    auth_manager: Arc<AuthManager>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
    app_server_client_name_rx: Option<oneshot::Receiver<String>>,
    initial_enabled: bool,
) -> io::Result<(JoinHandle<()>, RemoteControlHandle)> {
    let state_db_available = state_db.is_some();
    let requested_initial_enabled = initial_enabled;
    let initial_enabled = initial_enabled && state_db_available;
    if requested_initial_enabled && !state_db_available {
        warn!("remote control disabled because sqlite state db is unavailable");
    }
    let remote_control_target = if initial_enabled {
        Some(normalize_remote_control_url(&config.remote_control_url)?)
    } else {
        None
    };

    let (enabled_tx, enabled_rx) = watch::channel(initial_enabled);
    let pairing = PairingClientState::new();
    let websocket_pairing = pairing.clone();
    let pairing_request_cancellation_revision = Arc::new(AtomicU64::new(0));
    let (pairing_refresh_tx, pairing_refresh_rx) = watch::channel(0u64);
    let websocket_pairing_refresh_tx = pairing_refresh_tx.clone();
    let auth_change_rx = Arc::new(StdMutex::new(auth_manager.auth_change_receiver()));
    let server_name = gethostname().to_string_lossy().trim().to_string();
    let remote_control_url = config.remote_control_url;
    let installation_id = config.installation_id;
    let initial_status = RemoteControlStatusChangedNotification {
        status: if initial_enabled {
            RemoteControlConnectionStatus::Connecting
        } else {
            RemoteControlConnectionStatus::Disabled
        },
        server_name: server_name.clone(),
        installation_id: installation_id.clone(),
        environment_id: None,
    };
    let (status_tx, _status_rx) = watch::channel(initial_status);
    let status_publisher = RemoteControlStatusPublisher::new(status_tx.clone());
    info!(
        remote_control_url = %remote_control_url,
        installation_id = %installation_id,
        server_name = %server_name,
        state_db_available,
        initial_enabled,
        "starting app-server remote control websocket task"
    );
    let remote_control_url_for_log = remote_control_url.clone();
    let installation_id_for_log = installation_id.clone();
    let server_name_for_log = server_name.clone();
    let shutdown_token_for_log = shutdown_token.clone();
    let join_handle = tokio::spawn(async move {
        info!(
            remote_control_url = %remote_control_url_for_log,
            installation_id = %installation_id_for_log,
            server_name = %server_name_for_log,
            initial_enabled,
            "app-server remote control websocket task started"
        );
        let websocket_task = RemoteControlWebsocket::new(
            websocket::RemoteControlWebsocketConfig {
                remote_control_url,
                installation_id,
                remote_control_target,
                server_name,
            },
            state_db,
            auth_manager,
            RemoteControlChannels {
                transport_event_tx,
                status_publisher,
                pairing: websocket_pairing,
                pairing_refresh_tx: websocket_pairing_refresh_tx,
                pairing_refresh_rx,
            },
            shutdown_token,
            enabled_rx,
        )
        .run(app_server_client_name_rx);
        match AssertUnwindSafe(websocket_task).catch_unwind().await {
            Ok(()) => {
                let shutdown_requested = shutdown_token_for_log.is_cancelled();
                if shutdown_requested {
                    info!(
                        remote_control_url = %remote_control_url_for_log,
                        installation_id = %installation_id_for_log,
                        server_name = %server_name_for_log,
                        shutdown_requested,
                        "app-server remote control websocket task exited"
                    );
                } else {
                    warn!(
                        remote_control_url = %remote_control_url_for_log,
                        installation_id = %installation_id_for_log,
                        server_name = %server_name_for_log,
                        shutdown_requested,
                        "app-server remote control websocket task exited without shutdown"
                    );
                }
            }
            Err(panic) => {
                error!(
                    remote_control_url = %remote_control_url_for_log,
                    installation_id = %installation_id_for_log,
                    server_name = %server_name_for_log,
                    "app-server remote control websocket task panicked"
                );
                std::panic::resume_unwind(panic);
            }
        }
    });

    Ok((
        join_handle,
        RemoteControlHandle {
            enabled_tx: Arc::new(enabled_tx),
            status_tx: Arc::new(status_tx),
            state_db_available,
            pairing,
            pairing_request_cancellation_revision,
            pairing_refresh_tx: Arc::new(pairing_refresh_tx),
            auth_change_rx,
        },
    ))
}

#[cfg(test)]
mod pairing_tests;
#[cfg(test)]
mod segment_tests;
#[cfg(test)]
mod tests;
