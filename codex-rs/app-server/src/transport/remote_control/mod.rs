mod approval_key;
mod client_tracker;
mod enroll;
mod protocol;
mod websocket;

use crate::transport::remote_control::approval_key::RemoteControlApprovalKey;
use crate::transport::remote_control::approval_key::RemoteControlApprovalSignature;
use crate::transport::remote_control::websocket::RemoteControlWebsocket;
use crate::transport::remote_control::websocket::RemoteControlWebsocketControls;
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
use std::sync::Mutex;
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
    approval_key: Arc<Mutex<Option<RemoteControlApprovalKey>>>,
}

impl RemoteControlHandle {
    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled_tx.send_if_modified(|state| {
            let changed = *state != enabled;
            *state = enabled;
            changed
        });
    }

    pub(crate) fn sign_approval(
        &self,
        challenge: &str,
    ) -> io::Result<RemoteControlApprovalSignature> {
        let guard = self
            .approval_key
            .lock()
            .map_err(|_| io::Error::other("remote control approval key lock poisoned"))?;
        let Some(approval_key) = guard.as_ref() else {
            return Err(io::Error::other(
                "remote control approval key is not available",
            ));
        };
        Ok(approval_key.sign(challenge))
    }
}

struct RemoteControlApprovalKeyHandle {
    approval_key: Arc<Mutex<Option<RemoteControlApprovalKey>>>,
}

impl RemoteControlApprovalKeyHandle {
    #[cfg(test)]
    fn empty() -> Self {
        Self {
            approval_key: Arc::new(Mutex::new(None)),
        }
    }

    fn set(&self, approval_key: RemoteControlApprovalKey) -> io::Result<()> {
        let mut guard = self
            .approval_key
            .lock()
            .map_err(|_| io::Error::other("remote control approval key lock poisoned"))?;
        *guard = Some(approval_key);
        Ok(())
    }
}

pub(crate) async fn start_remote_control(
    remote_control_url: String,
    state_db: Option<Arc<StateRuntime>>,
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
    let approval_key = Arc::new(Mutex::new(None));
    let approval_key_handle = RemoteControlApprovalKeyHandle {
        approval_key: approval_key.clone(),
    };
    let join_handle = tokio::spawn(async move {
        RemoteControlWebsocket::new(
            remote_control_url,
            remote_control_target,
            state_db,
            auth_manager,
            transport_event_tx,
            shutdown_token,
            RemoteControlWebsocketControls {
                enabled_rx,
                approval_key_handle,
            },
        )
        .run(app_server_client_name_rx)
        .await;
    });

    Ok((
        join_handle,
        RemoteControlHandle {
            enabled_tx: Arc::new(enabled_tx),
            approval_key,
        },
    ))
}

#[cfg(test)]
mod tests;
