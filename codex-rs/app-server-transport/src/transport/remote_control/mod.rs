mod client_tracker;
mod enroll;
mod protocol;
mod segment;
mod websocket;

use self::enroll::RemoteControlEnrollment;
use self::enroll::refresh_remote_control_server;
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
    current_enrollment: CurrentRemoteControlEnrollment,
    auth_manager: Arc<AuthManager>,
}

type CurrentRemoteControlEnrollment = Arc<StdMutex<Option<RemoteControlEnrollment>>>;

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
        clear_current_enrollment(&self.current_enrollment);

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
            return Err(Self::pairing_disabled_error());
        }
        let mut auth = websocket::load_remote_control_auth(&self.auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        let mut enrollment = {
            let current_enrollment = self
                .current_enrollment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            current_enrollment
                .as_ref()
                .filter(|enrollment| enrollment.account_id == auth.account_id)
                .cloned()
        }
        .ok_or_else(pairing_unavailable_error)?;
        let installation_id = self.status().installation_id;
        if enrollment.should_refresh_server_token() {
            refresh_pairing_enrollment(
                &self.current_enrollment,
                &self.auth_manager,
                &mut auth,
                &installation_id,
                &mut enrollment,
            )
            .await?;
        }
        let pairing_request = || protocol::StartRemoteControlPairingRequest {
            manual_code: params.manual_code,
        };
        let pairing_response = match enrollment.start_pairing(&auth, pairing_request()).await {
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                clear_pairing_server_token(&self.current_enrollment, &mut enrollment)?;
                refresh_pairing_enrollment(
                    &self.current_enrollment,
                    &self.auth_manager,
                    &mut auth,
                    &installation_id,
                    &mut enrollment,
                )
                .await?;
                enrollment.start_pairing(&auth, pairing_request()).await
            }
            pairing_response => pairing_response,
        };
        if let Err(err) = &pairing_response {
            match err.kind() {
                io::ErrorKind::NotFound => {
                    clear_current_enrollment_if_matches(&self.current_enrollment, &enrollment);
                    return Err(pairing_unavailable_error());
                }
                io::ErrorKind::PermissionDenied => {
                    clear_pairing_server_token(&self.current_enrollment, &mut enrollment)?;
                    return Err(pairing_unavailable_error());
                }
                _ => {}
            }
        }
        if !*self.enabled_tx.borrow() {
            return Err(Self::pairing_disabled_error());
        }
        let current_auth = websocket::load_remote_control_auth(&self.auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        if current_auth.account_id != auth.account_id {
            return Err(pairing_unavailable_error());
        }
        pairing_response
    }

    fn pairing_disabled_error() -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "remote control pairing requires remote control to be enabled",
        )
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

async fn refresh_pairing_enrollment(
    current_enrollment: &CurrentRemoteControlEnrollment,
    auth_manager: &Arc<AuthManager>,
    auth: &mut enroll::RemoteControlConnectionAuth,
    installation_id: &str,
    enrollment: &mut RemoteControlEnrollment,
) -> io::Result<()> {
    if let Err(err) = refresh_remote_control_server(auth, installation_id, enrollment).await {
        if err.kind() != io::ErrorKind::PermissionDenied {
            return handle_pairing_refresh_error(current_enrollment, enrollment, err);
        }
        let mut auth_recovery = auth_manager.unauthorized_recovery();
        let mut auth_change_rx = auth_manager.auth_change_receiver();
        if !websocket::recover_remote_control_auth(&mut auth_recovery, &mut auth_change_rx).await {
            return Err(err);
        }
        *auth = websocket::load_remote_control_auth(auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        if auth.account_id != enrollment.account_id {
            return Err(pairing_unavailable_error());
        }
        if let Err(err) = refresh_remote_control_server(auth, installation_id, enrollment).await {
            return handle_pairing_refresh_error(current_enrollment, enrollment, err);
        }
    }
    if replace_current_enrollment(current_enrollment, enrollment) {
        Ok(())
    } else {
        Err(pairing_unavailable_error())
    }
}

fn handle_pairing_refresh_error(
    current_enrollment: &CurrentRemoteControlEnrollment,
    enrollment: &RemoteControlEnrollment,
    err: io::Error,
) -> io::Result<()> {
    if err.kind() == io::ErrorKind::NotFound {
        clear_current_enrollment_if_matches(current_enrollment, enrollment);
        Err(pairing_unavailable_error())
    } else {
        Err(err)
    }
}

fn clear_pairing_server_token(
    current_enrollment: &CurrentRemoteControlEnrollment,
    enrollment: &mut RemoteControlEnrollment,
) -> io::Result<()> {
    enrollment.clear_server_token();
    if replace_current_enrollment(current_enrollment, enrollment) {
        Ok(())
    } else {
        Err(pairing_unavailable_error())
    }
}

fn pairing_unavailable_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "remote control pairing is unavailable until enrollment completes",
    )
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

fn publish_current_enrollment(
    current_enrollment: &CurrentRemoteControlEnrollment,
    enrollment: &RemoteControlEnrollment,
) {
    *current_enrollment
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(enrollment.clone());
}

fn clear_current_enrollment(current_enrollment: &CurrentRemoteControlEnrollment) {
    *current_enrollment
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

fn replace_current_enrollment(
    current_enrollment: &CurrentRemoteControlEnrollment,
    enrollment: &RemoteControlEnrollment,
) -> bool {
    let mut current_enrollment = current_enrollment
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !current_enrollment
        .as_ref()
        .is_some_and(|current| same_remote_control_enrollment(current, enrollment))
    {
        return false;
    }
    *current_enrollment = Some(enrollment.clone());
    true
}

fn clear_current_enrollment_if_matches(
    current_enrollment: &CurrentRemoteControlEnrollment,
    enrollment: &RemoteControlEnrollment,
) {
    let mut current_enrollment = current_enrollment
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if current_enrollment
        .as_ref()
        .is_some_and(|current| same_remote_control_enrollment(current, enrollment))
    {
        *current_enrollment = None;
    }
}

fn same_remote_control_enrollment(
    left: &RemoteControlEnrollment,
    right: &RemoteControlEnrollment,
) -> bool {
    // A refresh rotates only the bearer. Pairing remains current while the same persisted server
    // record is still selected for the current account.
    left.account_id == right.account_id
        && left.server_id == right.server_id
        && left.environment_id == right.environment_id
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
    let current_enrollment = Arc::new(StdMutex::new(None));
    let websocket_current_enrollment = current_enrollment.clone();
    let handle_auth_manager = auth_manager.clone();
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
                current_enrollment: websocket_current_enrollment,
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
            current_enrollment,
            auth_manager: handle_auth_manager,
        },
    ))
}

#[cfg(test)]
mod segment_tests;
#[cfg(test)]
mod tests;
