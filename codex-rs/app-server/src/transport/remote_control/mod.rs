mod client_tracker;
mod enroll;
mod protocol;
mod websocket;

use crate::transport::remote_control::websocket::RemoteControlWebsocket;
use crate::transport::remote_control::websocket::load_remote_control_auth;

pub use self::protocol::ClientId;
use self::protocol::ServerEvent;
use self::protocol::StreamId;
use self::protocol::normalize_remote_control_url;
use super::CHANNEL_CAPACITY;
use super::TransportEvent;
use super::next_connection_id;
use codex_login::AuthManager;
use codex_state::StateRuntime;
use std::io;
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

impl RemoteControlHandle {
    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled_tx.send_if_modified(|state| {
            let changed = *state != enabled;
            *state = enabled;
            changed
        });
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RemoteControlAuthStartup {
    AllowRecoverable,
    RequireReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RemoteControlStartup {
    Disabled,
    Enabled { auth: RemoteControlAuthStartup },
}

pub(crate) async fn start_remote_control(
    remote_control_url: String,
    state_db: Option<Arc<StateRuntime>>,
    auth_manager: Arc<AuthManager>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
    app_server_client_name_rx: Option<oneshot::Receiver<String>>,
    startup: RemoteControlStartup,
) -> io::Result<(JoinHandle<()>, RemoteControlHandle)> {
    let remote_control_target = match startup {
        RemoteControlStartup::Enabled { .. } => {
            Some(normalize_remote_control_url(&remote_control_url)?)
        }
        RemoteControlStartup::Disabled => None,
    };
    if let RemoteControlStartup::Enabled { auth } = startup {
        match auth {
            RemoteControlAuthStartup::AllowRecoverable => {
                validate_remote_control_auth(&auth_manager).await?;
            }
            RemoteControlAuthStartup::RequireReady => {
                validate_remote_control_auth_ready(&auth_manager).await?;
            }
        }
    }

    let initial_enabled = matches!(startup, RemoteControlStartup::Enabled { .. });
    let (enabled_tx, enabled_rx) = watch::channel(initial_enabled);
    let join_handle = tokio::spawn(async move {
        RemoteControlWebsocket::new(
            remote_control_url,
            remote_control_target,
            state_db,
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

pub(crate) async fn validate_remote_control_auth(
    auth_manager: &Arc<AuthManager>,
) -> io::Result<()> {
    match load_remote_control_auth(auth_manager).await {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(err) => Err(err),
    }
}

pub(crate) async fn validate_remote_control_auth_ready(
    auth_manager: &Arc<AuthManager>,
) -> io::Result<()> {
    match load_remote_control_auth(auth_manager).await {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            err.to_string(),
        )),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests;
