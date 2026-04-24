mod client_tracker;
mod enroll;
mod protocol;
mod websocket;

use crate::transport::remote_control::websocket::RemoteControlWebsocket;

pub use self::protocol::ClientId;
use self::protocol::ServerEvent;
use self::protocol::StreamId;
use self::protocol::normalize_remote_control_url;
use super::CHANNEL_CAPACITY;
use super::TransportEvent;
use super::next_connection_id;
use codex_login::AuthManager;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(super) struct QueuedServerEnvelope {
    pub(super) event: ServerEvent,
    pub(super) client_id: ClientId,
    pub(super) stream_id: StreamId,
    pub(super) write_complete_tx: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
pub(crate) struct RemoteControlHandle {
    enabled_tx: Arc<watch::Sender<bool>>,
}

#[derive(Clone)]
pub(crate) struct RemoteControlStateRuntimeConfig {
    pub(crate) sqlite_home: PathBuf,
    pub(crate) model_provider_id: String,
}

impl RemoteControlHandle {
    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled_tx.send_if_modified(|state| {
            let changed = *state != enabled;
            *state = enabled;
            changed
        });
    }
}

pub(crate) async fn start_remote_control(
    remote_control_url: String,
    state_runtime_config: RemoteControlStateRuntimeConfig,
    auth_manager: Arc<AuthManager>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
    app_server_client_name_rx: Option<oneshot::Receiver<String>>,
    initial_enabled: bool,
) -> io::Result<(JoinHandle<()>, RemoteControlHandle)> {
    let remote_control_target = if initial_enabled {
        Some(normalize_remote_control_url(&remote_control_url)?)
    } else {
        None
    };

    let (enabled_tx, enabled_rx) = watch::channel(initial_enabled);
    let join_handle = tokio::spawn(async move {
        RemoteControlWebsocket::new(
            remote_control_url,
            remote_control_target,
            state_runtime_config,
            auth_manager,
            transport_event_tx,
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
        },
    ))
}

#[cfg(test)]
mod tests;
