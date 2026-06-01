mod client_tracker;
mod protocol;
mod segment;
mod server;
mod websocket;

use crate::transport::remote_control::server::SharedRemoteControlServer;
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
    server: SharedRemoteControlServer,
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
        if !*self.enabled_tx.borrow() {
            return Err(remote_control_pairing_disabled_error());
        }
        self.server.start_pairing(params).await
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

pub(super) fn remote_control_pairing_disabled_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "remote control pairing requires remote control to be enabled",
    )
}

pub(super) fn remote_control_pairing_unavailable_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "remote control pairing is unavailable until enrollment completes",
    )
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
    let server = SharedRemoteControlServer::new(
        remote_control_url.clone(),
        installation_id.clone(),
        server_name.clone(),
        state_db,
        auth_manager.clone(),
    );
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
    let server_for_task = server.clone();
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
            auth_manager,
            RemoteControlChannels {
                transport_event_tx,
                status_publisher,
            },
            server_for_task,
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
            server,
        },
    ))
}

#[cfg(test)]
mod segment_tests;
#[cfg(test)]
mod tests;
