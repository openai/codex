mod client_tracker;
mod enroll;
mod protocol;
mod segment;
mod websocket;

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
use codex_app_server_protocol::RemoteControlStatusChangedNotification;
use codex_login::AuthManager;
use codex_state::StateRuntime;
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

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
}

impl RemoteControlHandle {
    pub fn set_enabled(&self, enabled: bool) {
        let requested_enabled = enabled;
        let enabled = enabled && self.state_db_available;
        if requested_enabled && !self.state_db_available {
            warn!("remote control cannot be enabled because sqlite state db is unavailable");
        }
        self.enabled_tx.send_if_modified(|state| {
            let changed = *state != enabled;
            *state = enabled;
            changed
        });
    }

    pub fn status_receiver(&self) -> watch::Receiver<RemoteControlStatusChangedNotification> {
        self.status_tx.subscribe()
    }
}

pub async fn start_remote_control(
    remote_control_url: String,
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
        Some(normalize_remote_control_url(&remote_control_url)?)
    } else {
        None
    };

    let (enabled_tx, enabled_rx) = watch::channel(initial_enabled);
    let initial_status = RemoteControlStatusChangedNotification {
        status: if initial_enabled {
            RemoteControlConnectionStatus::Connecting
        } else {
            RemoteControlConnectionStatus::Disabled
        },
        environment_id: None,
    };
    let (status_tx, _status_rx) = watch::channel(initial_status);
    let status_publisher = RemoteControlStatusPublisher::new(status_tx.clone());
    let join_handle = tokio::spawn(async move {
        RemoteControlWebsocket::new(
            remote_control_url,
            remote_control_target,
            state_db,
            auth_manager,
            RemoteControlChannels {
                transport_event_tx,
                status_publisher,
            },
            shutdown_token,
            enabled_rx,
        )
        .run(app_server_client_name_rx)
        .await;
    });

    Ok((
        join_handle,
        RemoteControlHandle {
            enabled_tx: Arc::new(enabled_tx),
            status_tx: Arc::new(status_tx),
            state_db_available,
        },
    ))
}

#[cfg(test)]
mod segment_tests;
#[cfg(test)]
mod tests;
