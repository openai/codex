mod auth;
mod client_tracker;
mod clients;
mod enroll;
mod protocol;
mod segment;
mod websocket;

use self::auth::load_remote_control_auth;
use self::auth::recover_remote_control_auth;
use self::enroll::RemoteControlEnrollment;
use self::enroll::enroll_remote_control_server;
use self::enroll::load_persisted_remote_control_enrollment;
use self::enroll::refresh_remote_control_server;
use self::enroll::update_persisted_remote_control_enrollment;
use crate::transport::remote_control::websocket::RemoteControlChannels;
use crate::transport::remote_control::websocket::RemoteControlStatusPublisher;
use crate::transport::remote_control::websocket::RemoteControlWebsocket;

pub use self::protocol::ClientId;
use self::protocol::ServerEvent;
use self::protocol::StreamId;
use self::protocol::normalize_remote_control_url;
use super::CHANNEL_CAPACITY;
use super::InitializeClientMetadata;
use super::TransportEvent;
use super::next_connection_id;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
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
use std::ops::Deref;
use std::ops::DerefMut;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;
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
    state_db: Option<Arc<StateRuntime>>,
    remote_control_url: String,
    current_enrollment: CurrentRemoteControlEnrollment,
    pairing_persistence_key: RemoteControlPairingPersistenceKey,
    pairing_persistence_key_required: bool,
    auth_manager: Arc<AuthManager>,
}

// Pairing and websocket connect share one selected server so they cannot enroll or clear
// different persisted rows while either path is awaiting backend I/O.
type CurrentRemoteControlEnrollment = Arc<RemoteControlEnrollmentState>;
type RemoteControlPairingPersistenceKey = watch::Sender<Option<String>>;

struct RemoteControlEnrollmentState {
    enrollment: StdMutex<Option<RemoteControlEnrollment>>,
    lock: Semaphore,
}

impl RemoteControlEnrollmentState {
    fn new(enrollment: Option<RemoteControlEnrollment>) -> Self {
        Self {
            enrollment: StdMutex::new(enrollment),
            lock: Semaphore::new(1),
        }
    }

    async fn lock(&self) -> RemoteControlEnrollmentLease<'_> {
        let permit = match self.lock.acquire().await {
            Ok(permit) => permit,
            Err(_) => unreachable!("remote control enrollment lock should stay open"),
        };
        RemoteControlEnrollmentLease {
            state: self,
            enrollment: self.snapshot(),
            _permit: permit,
        }
    }

    fn snapshot(&self) -> Option<RemoteControlEnrollment> {
        self.enrollment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

struct RemoteControlEnrollmentLease<'a> {
    state: &'a RemoteControlEnrollmentState,
    enrollment: Option<RemoteControlEnrollment>,
    _permit: SemaphorePermit<'a>,
}

impl Deref for RemoteControlEnrollmentLease<'_> {
    type Target = Option<RemoteControlEnrollment>;

    fn deref(&self) -> &Self::Target {
        &self.enrollment
    }
}

impl DerefMut for RemoteControlEnrollmentLease<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.enrollment
    }
}

impl Drop for RemoteControlEnrollmentLease<'_> {
    fn drop(&mut self) {
        *self
            .state
            .enrollment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = self.enrollment.take();
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
        app_server_client_name: Option<&str>,
    ) -> io::Result<RemoteControlPairingStartResponse> {
        let mut auth = load_remote_control_auth(&self.auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        let status = self.status();
        let installation_id = status.installation_id;
        let app_server_client_name = self.pairing_persistence_key(app_server_client_name)?;
        let app_server_client_name = app_server_client_name.as_deref();
        let mut current_enrollment = self.current_enrollment.lock().await;
        let mut enrollment = self
            .load_or_enroll_pairing_server(
                &mut current_enrollment,
                &mut auth,
                &installation_id,
                &status.server_name,
                app_server_client_name,
            )
            .await?;
        if enrollment.should_refresh_server_token() {
            refresh_pairing_enrollment(
                &mut current_enrollment,
                self.state_db.as_deref(),
                app_server_client_name,
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
        let pairing_response = match enrollment.start_pairing(pairing_request()).await {
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                clear_pairing_server_token(&mut current_enrollment, &mut enrollment)?;
                refresh_pairing_enrollment(
                    &mut current_enrollment,
                    self.state_db.as_deref(),
                    app_server_client_name,
                    &self.auth_manager,
                    &mut auth,
                    &installation_id,
                    &mut enrollment,
                )
                .await?;
                enrollment.start_pairing(pairing_request()).await
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                clear_pairing_enrollment(
                    &mut current_enrollment,
                    self.state_db.as_deref(),
                    app_server_client_name,
                    &enrollment,
                )
                .await;
                enrollment = self
                    .load_or_enroll_pairing_server(
                        &mut current_enrollment,
                        &mut auth,
                        &installation_id,
                        &status.server_name,
                        app_server_client_name,
                    )
                    .await?;
                enrollment.start_pairing(pairing_request()).await
            }
            pairing_response => pairing_response,
        };
        if let Err(err) = &pairing_response {
            match err.kind() {
                io::ErrorKind::NotFound => {
                    clear_pairing_enrollment(
                        &mut current_enrollment,
                        self.state_db.as_deref(),
                        app_server_client_name,
                        &enrollment,
                    )
                    .await;
                    return Err(pairing_unavailable_error());
                }
                io::ErrorKind::PermissionDenied => {
                    clear_pairing_server_token(&mut current_enrollment, &mut enrollment)?;
                    return Err(pairing_unavailable_error());
                }
                _ => {}
            }
        }
        let current_auth = load_remote_control_auth(&self.auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        if current_auth.account_id != auth.account_id {
            return Err(pairing_unavailable_error());
        }
        pairing_response
    }

    async fn load_or_enroll_pairing_server(
        &self,
        current_enrollment: &mut Option<RemoteControlEnrollment>,
        auth: &mut auth::RemoteControlConnectionAuth,
        installation_id: &str,
        server_name: &str,
        app_server_client_name: Option<&str>,
    ) -> io::Result<RemoteControlEnrollment> {
        if let Some(enrollment) = current_enrollment
            .as_ref()
            .filter(|enrollment| enrollment.account_id == auth.account_id)
            .cloned()
        {
            return Ok(enrollment);
        }

        let remote_control_target = normalize_remote_control_url(&self.remote_control_url)?;
        let state_db = self
            .state_db
            .as_deref()
            .ok_or_else(pairing_unavailable_error)?;
        if let Some(mut enrollment) = load_persisted_remote_control_enrollment(
            Some(state_db),
            &remote_control_target,
            &auth.account_id,
            app_server_client_name,
        )
        .await?
        {
            enrollment.server_name = server_name.to_string();
            publish_current_enrollment(current_enrollment, &enrollment);
            return Ok(enrollment);
        }

        let enrollment = enroll_pairing_server(
            &self.auth_manager,
            auth,
            &remote_control_target,
            installation_id,
            server_name,
        )
        .await?;
        update_persisted_remote_control_enrollment(
            Some(state_db),
            &remote_control_target,
            &auth.account_id,
            app_server_client_name,
            Some(&enrollment),
        )
        .await?;
        publish_current_enrollment(current_enrollment, &enrollment);
        Ok(enrollment)
    }

    fn pairing_persistence_key(
        &self,
        app_server_client_name: Option<&str>,
    ) -> io::Result<Option<String>> {
        if self.pairing_persistence_key_required && self.pairing_persistence_key.borrow().is_none()
        {
            let app_server_client_name =
                app_server_client_name.ok_or_else(pairing_unavailable_error)?;
            self.pairing_persistence_key
                .send_replace(Some(app_server_client_name.to_string()));
        }
        Ok(self.pairing_persistence_key.borrow().clone())
    }

    pub async fn list_clients(
        &self,
        params: RemoteControlClientsListParams,
    ) -> io::Result<RemoteControlClientsListResponse> {
        clients::list_remote_control_clients(&self.remote_control_url, &self.auth_manager, params)
            .await
    }

    pub async fn revoke_client(
        &self,
        params: RemoteControlClientsRevokeParams,
    ) -> io::Result<RemoteControlClientsRevokeResponse> {
        clients::revoke_remote_control_client(&self.remote_control_url, &self.auth_manager, params)
            .await
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

async fn enroll_pairing_server(
    auth_manager: &Arc<AuthManager>,
    auth: &mut auth::RemoteControlConnectionAuth,
    remote_control_target: &protocol::RemoteControlTarget,
    installation_id: &str,
    server_name: &str,
) -> io::Result<RemoteControlEnrollment> {
    match enroll_remote_control_server(
        remote_control_target,
        auth,
        installation_id,
        server_name,
        /*apns_registration*/ None,
    )
    .await
    {
        Ok(enrollment) => return Ok(enrollment),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            let mut auth_recovery = auth_manager.unauthorized_recovery();
            let mut auth_change_rx = auth_manager.auth_change_receiver();
            if !recover_remote_control_auth(&mut auth_recovery, &mut auth_change_rx).await {
                return Err(err);
            }
            *auth = load_remote_control_auth(auth_manager)
                .await
                .map_err(|_| pairing_unavailable_error())?;
        }
        Err(err) => return Err(err),
    }
    enroll_remote_control_server(
        remote_control_target,
        auth,
        installation_id,
        server_name,
        /*apns_registration*/ None,
    )
    .await
}

async fn refresh_pairing_enrollment(
    current_enrollment: &mut Option<RemoteControlEnrollment>,
    state_db: Option<&StateRuntime>,
    app_server_client_name: Option<&str>,
    auth_manager: &Arc<AuthManager>,
    auth: &mut auth::RemoteControlConnectionAuth,
    installation_id: &str,
    enrollment: &mut RemoteControlEnrollment,
) -> io::Result<()> {
    if let Err(err) = refresh_remote_control_server(
        auth,
        installation_id,
        /*apns_registration*/ None,
        enrollment,
    )
    .await
    {
        if err.kind() != io::ErrorKind::PermissionDenied {
            return handle_pairing_refresh_error(
                current_enrollment,
                state_db,
                app_server_client_name,
                enrollment,
                err,
            )
            .await;
        }
        let mut auth_recovery = auth_manager.unauthorized_recovery();
        let mut auth_change_rx = auth_manager.auth_change_receiver();
        if !recover_remote_control_auth(&mut auth_recovery, &mut auth_change_rx).await {
            return Err(err);
        }
        *auth = load_remote_control_auth(auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        if auth.account_id != enrollment.account_id {
            return Err(pairing_unavailable_error());
        }
        if let Err(err) = refresh_remote_control_server(
            auth,
            installation_id,
            /*apns_registration*/ None,
            enrollment,
        )
        .await
        {
            return handle_pairing_refresh_error(
                current_enrollment,
                state_db,
                app_server_client_name,
                enrollment,
                err,
            )
            .await;
        }
    }
    if replace_current_enrollment(current_enrollment, enrollment) {
        Ok(())
    } else {
        Err(pairing_unavailable_error())
    }
}

async fn handle_pairing_refresh_error(
    current_enrollment: &mut Option<RemoteControlEnrollment>,
    state_db: Option<&StateRuntime>,
    app_server_client_name: Option<&str>,
    enrollment: &RemoteControlEnrollment,
    err: io::Error,
) -> io::Result<()> {
    if err.kind() == io::ErrorKind::NotFound {
        clear_pairing_enrollment(
            current_enrollment,
            state_db,
            app_server_client_name,
            enrollment,
        )
        .await;
        Err(pairing_unavailable_error())
    } else {
        Err(err)
    }
}

async fn clear_pairing_enrollment(
    current_enrollment: &mut Option<RemoteControlEnrollment>,
    state_db: Option<&StateRuntime>,
    app_server_client_name: Option<&str>,
    enrollment: &RemoteControlEnrollment,
) {
    if !clear_current_enrollment_if_matches(current_enrollment, enrollment) {
        return;
    }
    let Some(state_db) = state_db else {
        return;
    };
    if let Err(err) = update_persisted_remote_control_enrollment(
        Some(state_db),
        &enrollment.remote_control_target,
        &enrollment.account_id,
        app_server_client_name,
        /*enrollment*/ None,
    )
    .await
    {
        warn!("failed to clear stale pairing enrollment: {err}");
    }
}

fn clear_pairing_server_token(
    current_enrollment: &mut Option<RemoteControlEnrollment>,
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
    current_enrollment: &mut Option<RemoteControlEnrollment>,
    enrollment: &RemoteControlEnrollment,
) {
    *current_enrollment = Some(enrollment.clone());
}

fn replace_current_enrollment(
    current_enrollment: &mut Option<RemoteControlEnrollment>,
    enrollment: &RemoteControlEnrollment,
) -> bool {
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
    current_enrollment: &mut Option<RemoteControlEnrollment>,
    enrollment: &RemoteControlEnrollment,
) -> bool {
    if current_enrollment
        .as_ref()
        .is_some_and(|current| same_remote_control_enrollment(current, enrollment))
    {
        *current_enrollment = None;
        true
    } else {
        false
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
    app_server_client_metadata_rx: Option<oneshot::Receiver<InitializeClientMetadata>>,
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
    let current_enrollment = Arc::new(RemoteControlEnrollmentState::new(/*enrollment*/ None));
    let websocket_current_enrollment = current_enrollment.clone();
    let pairing_persistence_key_required = app_server_client_metadata_rx.is_some();
    let (pairing_persistence_key, _pairing_persistence_key_rx) = watch::channel(None);
    let websocket_pairing_persistence_key = pairing_persistence_key.clone();
    let handle_auth_manager = auth_manager.clone();
    let handle_state_db = state_db.clone();
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
    let handle_remote_control_url = remote_control_url.clone();
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
                pairing_persistence_key: websocket_pairing_persistence_key,
            },
            shutdown_token,
            enabled_rx,
        )
        .run(app_server_client_metadata_rx);
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
            state_db: handle_state_db,
            remote_control_url: handle_remote_control_url,
            current_enrollment,
            pairing_persistence_key,
            pairing_persistence_key_required,
            auth_manager: handle_auth_manager,
        },
    ))
}

#[cfg(test)]
mod segment_tests;
#[cfg(test)]
mod tests;
