mod client_tracker;
mod enroll;
mod protocol;
mod websocket;

use crate::transport::remote_control::websocket::RemoteControlWebsocket;
use crate::transport::remote_control::websocket::load_remote_control_auth;

pub use self::protocol::ClientId;
use self::protocol::ServerEvent;
use self::protocol::normalize_remote_control_url;
use super::CHANNEL_CAPACITY;
use super::TransportEvent;
use super::next_connection_id;
use codex_core::AuthManager;
use codex_state::StateRuntime;
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(super) struct QueuedServerEnvelope {
    pub(super) event: ServerEvent,
    pub(super) client_id: ClientId,
    pub(super) write_complete_tx: Option<oneshot::Sender<()>>,
}

pub(crate) async fn start_remote_control(
    remote_control_url: String,
    state_db: Option<Arc<StateRuntime>>,
    auth_manager: Arc<AuthManager>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
) -> io::Result<JoinHandle<()>> {
    let remote_control_target = normalize_remote_control_url(&remote_control_url)?;
    validate_remote_control_auth(&auth_manager).await?;

    Ok(tokio::spawn(async move {
        RemoteControlWebsocket::new(
            remote_control_target,
            state_db,
            auth_manager,
            transport_event_tx,
            shutdown_token,
        )
        .run()
        .await;
    }))
}

pub(crate) async fn validate_remote_control_auth(
    auth_manager: &Arc<AuthManager>,
) -> io::Result<()> {
    load_remote_control_auth(auth_manager).await.map(|_| ())
}

#[cfg(test)]
mod tests;
