mod auth;
mod client_tracker;
mod clients;
mod controller;
mod persistence;
pub use controller::RemoteControlHandle;
pub use controller::start_remote_control;
mod desired_state;
mod enroll;
mod host_device;
mod protocol;
mod segment;
mod server_api;
mod websocket;

use self::auth::load_remote_control_auth;
use self::auth::recover_remote_control_auth;
use self::desired_state::RemoteControlDesiredState;
use self::enroll::RemoteControlEnrollment;
use self::enroll::load_persisted_remote_control_enrollment;
use self::persistence::RemoteControlPersistence;
use self::server_api::enroll_remote_control_server;
use self::server_api::refresh_remote_control_server;
use crate::transport::remote_control::websocket::RemoteControlChannels;
use crate::transport::remote_control::websocket::RemoteControlStatusPublisher;
use crate::transport::remote_control::websocket::RemoteControlWebsocket;

pub use self::protocol::ClientId;
use self::protocol::RemoteControlPairingStatusCode;
use self::protocol::ServerEvent;
use self::protocol::StreamId;
use self::protocol::normalize_remote_control_url;
use super::CHANNEL_CAPACITY;
use super::TransportEvent;
use super::next_connection_id;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_app_server_protocol::RemoteControlPairingStatusParams;
use codex_app_server_protocol::RemoteControlPairingStatusResponse;
use codex_app_server_protocol::RemoteControlStatusChangedNotification;
use codex_login::AuthManager;
use codex_state::StateRuntime;
use gethostname::gethostname;
use std::error::Error;
use std::fmt;
use std::io;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

pub struct RemoteControlStartConfig {
    pub remote_control_url: String,
    pub installation_id: String,
    pub policy: RemoteControlPolicy,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RemoteControlPolicy {
    #[default]
    Allowed,
    DisabledByRequirements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteControlStartupMode {
    ResolvePersisted,
    DisabledEphemeral,
    EnabledEphemeral,
}

/// Internal marker used by the daemon to disable remote control without requiring a new CLI flag.
pub const REMOTE_CONTROL_DISABLED_ENV_VAR: &str =
    "CODEX_INTERNAL_APP_SERVER_REMOTE_CONTROL_DISABLED";

/// Reads and removes the daemon's internal disabled-start marker before worker threads start.
pub fn take_remote_control_disabled_env() -> bool {
    let disabled =
        std::env::var_os(REMOTE_CONTROL_DISABLED_ENV_VAR).is_some_and(|value| value == "1");
    // SAFETY: app-server calls this synchronously at process startup, before spawning threads.
    unsafe { std::env::remove_var(REMOTE_CONTROL_DISABLED_ENV_VAR) };
    disabled
}

pub(super) struct QueuedServerEnvelope {
    pub(super) event: ServerEvent,
    pub(super) client_id: ClientId,
    pub(super) stream_id: StreamId,
    pub(super) write_complete_tx: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
struct RemoteControlSession {
    policy: RemoteControlPolicy,
    shutdown_token: CancellationToken,
    desired_state_tx: Arc<watch::Sender<RemoteControlDesiredState>>,
    desired_state_rpc_lock: Arc<Semaphore>,
    persistence: RemoteControlPersistence,
    status_tx: Arc<watch::Sender<RemoteControlStatusChangedNotification>>,
    state_db: Option<Arc<StateRuntime>>,
    remote_control_url: String,
    current_enrollment: CurrentRemoteControlEnrollment,
    pairing_persistence_key: RemoteControlPairingPersistenceKey,
    pairing_persistence_key_required: bool,
    auth_manager: auth::RemoteControlAuth,
}

// Pairing and websocket connect share one selected server so they cannot enroll or replace
// different persisted rows while either path is awaiting backend I/O.
type CurrentRemoteControlEnrollment = Arc<RemoteControlEnrollmentState>;
type RemoteControlPairingPersistenceKey = watch::Sender<Option<String>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteControlEnrollmentSelection {
    ReuseOrCreate,
    ReplaceExisting,
}

struct RemoteControlEnrollmentState {
    enrollment: StdMutex<Option<RemoteControlEnrollment>>,
    // Keep an observed server deadline across enrollment replacement and enable transitions.
    retry_at: StdMutex<Option<time::OffsetDateTime>>,
    lock: Semaphore,
}

impl RemoteControlEnrollmentState {
    fn new(enrollment: Option<RemoteControlEnrollment>) -> Self {
        Self {
            enrollment: StdMutex::new(enrollment),
            retry_at: StdMutex::new(None),
            lock: Semaphore::new(1),
        }
    }

    async fn lock_for_request(&self) -> io::Result<RemoteControlEnrollmentLease<'_>> {
        self.check_retry_after()?;
        let lease = self.lock().await;
        self.check_retry_after()?;
        Ok(lease)
    }

    // Check immediately before admitting network work. Requests admitted before
    // an overload response publishes its deadline may finish concurrently.
    fn check_retry_after(&self) -> io::Result<()> {
        let retry_at = *self
            .retry_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(retry_at) = retry_at
            && retry_at > time::OffsetDateTime::now_utc()
        {
            return Err(server_api::RemoteControlServerRequestError::retry_deferred(
                retry_at,
            ));
        }
        Ok(())
    }

    fn record_retry_after<T>(&self, result: io::Result<T>) -> io::Result<T> {
        if let Err(err) = &result
            && let Some(retry_at) = server_api::remote_control_retry_at(err)
        {
            let mut current_retry_at = self
                .retry_at
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *current_retry_at = Some(current_retry_at.map_or(retry_at, |at| at.max(retry_at)));
        }
        result
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteControlDisabledByRequirements;

impl fmt::Display for RemoteControlDisabledByRequirements {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "remote control is disabled by managed requirements")
    }
}

impl Error for RemoteControlDisabledByRequirements {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteControlEnableError {
    Unavailable(RemoteControlUnavailable),
    DisabledByRequirements(RemoteControlDisabledByRequirements),
    AuthenticationChanged,
}

impl fmt::Display for RemoteControlEnableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(err) => err.fmt(f),
            Self::DisabledByRequirements(err) => err.fmt(f),
            Self::AuthenticationChanged => f.write_str("remote control authentication changed"),
        }
    }
}

impl Error for RemoteControlEnableError {}

impl RemoteControlSession {
    pub fn ensure_remote_control_allowed(&self) -> Result<(), RemoteControlDisabledByRequirements> {
        match self.policy {
            RemoteControlPolicy::Allowed => Ok(()),
            RemoteControlPolicy::DisabledByRequirements => Err(RemoteControlDisabledByRequirements),
        }
    }

    fn ensure_remote_control_allowed_io(&self) -> io::Result<()> {
        self.ensure_remote_control_allowed()
            .map_err(|err| io::Error::new(io::ErrorKind::PermissionDenied, err))
    }

    pub fn enable_ephemeral(
        &self,
    ) -> Result<RemoteControlStatusChangedNotification, RemoteControlEnableError> {
        self.enable_with_preference(/*persistence_preference*/ None)
    }

    fn enable_with_preference(
        &self,
        persistence_preference: Option<bool>,
    ) -> Result<RemoteControlStatusChangedNotification, RemoteControlEnableError> {
        self.ensure_remote_control_allowed()
            .map_err(RemoteControlEnableError::DisabledByRequirements)?;
        if self.state_db.is_none() {
            warn!("remote control cannot be enabled because sqlite state db is unavailable");
            return Err(RemoteControlEnableError::Unavailable(
                RemoteControlUnavailable,
            ));
        }

        let mut effective_persistence_preference = persistence_preference;
        self.auth_manager
            .ensure_current()
            .map_err(|_| RemoteControlEnableError::AuthenticationChanged)?;
        let desired_state_changed = self.desired_state_tx.send_if_modified(|state| {
            if effective_persistence_preference.is_none()
                && matches!(
                    *state,
                    RemoteControlDesiredState::Enabled {
                        persistence_preference: Some(true)
                    }
                )
            {
                effective_persistence_preference = Some(true);
            }
            let next_state = RemoteControlDesiredState::Enabled {
                persistence_preference: effective_persistence_preference,
            };
            let changed = *state != next_state;
            *state = next_state;
            changed
        });

        let status = self.status();
        info!(
            desired_state_changed,
            ?effective_persistence_preference,
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

    pub async fn disable(
        &self,
        app_server_client_name: Option<&str>,
    ) -> io::Result<RemoteControlStatusChangedNotification> {
        self.ensure_remote_control_allowed_io()?;
        let _transition = self
            .desired_state_rpc_lock
            .acquire()
            .await
            .unwrap_or_else(|_| unreachable!());
        self.persist_preference(
            app_server_client_name,
            /*remote_control_enabled*/ false,
        )
        .await?;
        Ok(self.transition_disabled())
    }

    pub async fn disable_ephemeral(&self) -> RemoteControlStatusChangedNotification {
        let _transition = self
            .desired_state_rpc_lock
            .acquire()
            .await
            .unwrap_or_else(|_| unreachable!());
        let _persistence = self.persistence.lock().await;
        self.transition_disabled()
    }

    fn transition_disabled(&self) -> RemoteControlStatusChangedNotification {
        let desired_state_changed = self.desired_state_tx.send_if_modified(|state| {
            let changed = *state != RemoteControlDesiredState::Disabled;
            *state = RemoteControlDesiredState::Disabled;
            changed
        });
        let status = self.status();
        info!(
            desired_state_changed,
            current_status = ?status.status,
            environment_id = ?status.environment_id,
            installation_id = %status.installation_id,
            server_name = %status.server_name,
            "remote control disable requested"
        );
        self.publish_status(RemoteControlConnectionStatus::Disabled)
    }

    async fn persist_preference(
        &self,
        app_server_client_name: Option<&str>,
        remote_control_enabled: bool,
    ) -> io::Result<()> {
        let state_db = self
            .state_db
            .as_deref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, RemoteControlUnavailable))?;
        let auth = load_remote_control_auth(&self.auth_manager).await?;
        let remote_control_target = normalize_remote_control_url(&self.remote_control_url)?;
        let app_server_client_name = self.pairing_persistence_key(app_server_client_name)?;
        self.set_preference(
            state_db,
            &remote_control_target,
            &auth.account_id,
            app_server_client_name.as_deref(),
            remote_control_enabled,
            /*fallback_enrollment*/ None,
        )
        .await?;
        Ok(())
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
        self.ensure_remote_control_allowed_io()?;
        if !self.desired_state_tx.borrow().is_enabled() {
            return Err(Self::pairing_disabled_error());
        }
        let mut current_enrollment = self.current_enrollment.lock_for_request().await?;
        let mut auth = load_remote_control_auth(&self.auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        let status = self.status();
        let installation_id = status.installation_id;
        let app_server_client_name = self.pairing_persistence_key(app_server_client_name)?;
        let app_server_client_name = app_server_client_name.as_deref();
        let mut enrollment = self
            .load_or_enroll_pairing_server(
                &mut current_enrollment,
                &mut auth,
                &installation_id,
                &status.server_name,
                app_server_client_name,
                RemoteControlEnrollmentSelection::ReuseOrCreate,
            )
            .await?;
        if enrollment.should_refresh_server_token() {
            let refresh_result = refresh_pairing_enrollment(
                &mut current_enrollment,
                &self.auth_manager,
                &mut auth,
                &installation_id,
                &mut enrollment,
            )
            .await;
            if refresh_result
                .as_ref()
                .is_err_and(|err| err.kind() == io::ErrorKind::NotFound)
            {
                enrollment = self
                    .load_or_enroll_pairing_server(
                        &mut current_enrollment,
                        &mut auth,
                        &installation_id,
                        &status.server_name,
                        app_server_client_name,
                        RemoteControlEnrollmentSelection::ReplaceExisting,
                    )
                    .await?;
            } else {
                refresh_result?;
            }
        }
        let pairing_request = || protocol::StartRemoteControlPairingRequest {
            manual_code: params.manual_code,
        };
        self.current_enrollment.check_retry_after()?;
        let pairing_response = match enrollment
            .start_pairing(&auth.http_client_factory, pairing_request())
            .await
        {
            Err(err) if auth::is_auth_error(&err) => {
                clear_pairing_server_token(&mut current_enrollment, &mut enrollment)?;
                refresh_pairing_enrollment(
                    &mut current_enrollment,
                    &self.auth_manager,
                    &mut auth,
                    &installation_id,
                    &mut enrollment,
                )
                .await?;
                self.current_enrollment.check_retry_after()?;
                enrollment
                    .start_pairing(&auth.http_client_factory, pairing_request())
                    .await
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                enrollment = self
                    .load_or_enroll_pairing_server(
                        &mut current_enrollment,
                        &mut auth,
                        &installation_id,
                        &status.server_name,
                        app_server_client_name,
                        RemoteControlEnrollmentSelection::ReplaceExisting,
                    )
                    .await?;
                self.current_enrollment.check_retry_after()?;
                enrollment
                    .start_pairing(&auth.http_client_factory, pairing_request())
                    .await
            }
            pairing_response => pairing_response,
        };
        if let Err(err) = &pairing_response {
            if server_api::remote_control_retry_at(err).is_some() {
                return self.current_enrollment.record_retry_after(pairing_response);
            }
            match err.kind() {
                io::ErrorKind::NotFound => {
                    self.load_or_enroll_pairing_server(
                        &mut current_enrollment,
                        &mut auth,
                        &installation_id,
                        &status.server_name,
                        app_server_client_name,
                        RemoteControlEnrollmentSelection::ReplaceExisting,
                    )
                    .await?;
                    return Err(pairing_unavailable_error());
                }
                io::ErrorKind::PermissionDenied if auth::is_auth_error(err) => {
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
        if !self.desired_state_tx.borrow().is_enabled() {
            return Err(Self::pairing_disabled_error());
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
        selection: RemoteControlEnrollmentSelection,
    ) -> io::Result<RemoteControlEnrollment> {
        let (enrollment, created) = self
            .load_or_enroll_server(
                current_enrollment,
                auth,
                installation_id,
                server_name,
                app_server_client_name,
                selection,
            )
            .await?;
        if !created {
            publish_current_enrollment(current_enrollment, &enrollment);
            return Ok(enrollment);
        }

        let state_db = self
            .state_db
            .as_deref()
            .ok_or_else(pairing_unavailable_error)?;
        persistence::save_enrollment(
            &self.auth_manager,
            &self.persistence,
            state_db,
            &enrollment,
            app_server_client_name,
            &self.desired_state_tx,
        )
        .await?;
        publish_current_enrollment(current_enrollment, &enrollment);
        Ok(enrollment)
    }

    async fn load_or_enroll_server(
        &self,
        current_enrollment: &Option<RemoteControlEnrollment>,
        auth: &mut auth::RemoteControlConnectionAuth,
        installation_id: &str,
        server_name: &str,
        app_server_client_name: Option<&str>,
        selection: RemoteControlEnrollmentSelection,
    ) -> io::Result<(RemoteControlEnrollment, bool)> {
        let remote_control_target = normalize_remote_control_url(&self.remote_control_url)?;
        match selection {
            RemoteControlEnrollmentSelection::ReuseOrCreate => {
                if let Some(enrollment) = current_enrollment
                    .as_ref()
                    .filter(|enrollment| enrollment.account_id == auth.account_id)
                    .cloned()
                {
                    return Ok((enrollment, false));
                }

                let state_db = self
                    .state_db
                    .as_deref()
                    .ok_or_else(pairing_unavailable_error)?;
                let _persistence =
                    persistence::read_lock(&self.auth_manager, &self.persistence).await?;
                if let Some(mut enrollment) = load_persisted_remote_control_enrollment(
                    Some(state_db),
                    &remote_control_target,
                    &auth.account_id,
                    app_server_client_name,
                )
                .await?
                {
                    enrollment.server_name = server_name.to_string();
                    return Ok((enrollment, false));
                }
            }
            RemoteControlEnrollmentSelection::ReplaceExisting => {}
        }

        self.current_enrollment.check_retry_after()?;
        // Reused enrollments must still reach durable persistence during shutdown.
        let enrollment = tokio::select! {
            biased;
            _ = self.shutdown_token.cancelled() => {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "remote control is shutting down",
                ));
            }
            result = enroll_pairing_server(
                &self.current_enrollment,
                &self.auth_manager,
                auth,
                &remote_control_target,
                installation_id,
                server_name,
            ) => self.current_enrollment.record_retry_after(result)?,
        };
        Ok((enrollment, true))
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

    pub async fn pairing_status(
        &self,
        params: RemoteControlPairingStatusParams,
    ) -> io::Result<RemoteControlPairingStatusResponse> {
        self.ensure_remote_control_allowed_io()?;
        if !self.desired_state_tx.borrow().is_enabled() {
            return Err(Self::pairing_disabled_error());
        }
        let mut current_enrollment = self.current_enrollment.lock_for_request().await?;
        let mut auth = load_remote_control_auth(&self.auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        let app_server_client_name = self.pairing_persistence_key.borrow().clone();
        let app_server_client_name = app_server_client_name.as_deref();
        let mut enrollment = current_enrollment
            .as_ref()
            .filter(|enrollment| enrollment.account_id == auth.account_id)
            .cloned()
            .ok_or_else(pairing_unavailable_error)?;
        let status = self.status();
        let installation_id = status.installation_id;
        let server_name = status.server_name;
        if enrollment.should_refresh_server_token() {
            let refresh_result = refresh_pairing_enrollment(
                &mut current_enrollment,
                &self.auth_manager,
                &mut auth,
                &installation_id,
                &mut enrollment,
            )
            .await;
            if refresh_result
                .as_ref()
                .is_err_and(|err| err.kind() == io::ErrorKind::NotFound)
            {
                self.load_or_enroll_pairing_server(
                    &mut current_enrollment,
                    &mut auth,
                    &installation_id,
                    &server_name,
                    app_server_client_name,
                    RemoteControlEnrollmentSelection::ReplaceExisting,
                )
                .await?;
                return Err(pairing_unavailable_error());
            }
            refresh_result?;
        }
        let status_code = remote_control_pairing_status_code(&params)?;
        let pairing_status_request =
            || protocol::RemoteControlPairingStatusRequest::from(status_code.clone());
        self.current_enrollment.check_retry_after()?;
        let pairing_status_response = match enrollment
            .pairing_status(&auth.http_client_factory, pairing_status_request())
            .await
        {
            Err(err) if auth::is_auth_error(&err) => {
                clear_pairing_server_token(&mut current_enrollment, &mut enrollment)?;
                refresh_pairing_enrollment(
                    &mut current_enrollment,
                    &self.auth_manager,
                    &mut auth,
                    &installation_id,
                    &mut enrollment,
                )
                .await?;
                self.current_enrollment.check_retry_after()?;
                enrollment
                    .pairing_status(&auth.http_client_factory, pairing_status_request())
                    .await
            }
            pairing_status_response => pairing_status_response,
        };
        if let Err(err) = &pairing_status_response {
            if server_api::remote_control_retry_at(err).is_some() {
                return self
                    .current_enrollment
                    .record_retry_after(pairing_status_response);
            }
            match err.kind() {
                io::ErrorKind::NotFound => {
                    self.load_or_enroll_pairing_server(
                        &mut current_enrollment,
                        &mut auth,
                        &installation_id,
                        &server_name,
                        app_server_client_name,
                        RemoteControlEnrollmentSelection::ReplaceExisting,
                    )
                    .await?;
                    return Err(pairing_unavailable_error());
                }
                io::ErrorKind::PermissionDenied if auth::is_auth_error(err) => {
                    clear_pairing_server_token(&mut current_enrollment, &mut enrollment)?;
                    return Err(pairing_unavailable_error());
                }
                _ => {}
            }
        }
        if !self.desired_state_tx.borrow().is_enabled() {
            return Err(Self::pairing_disabled_error());
        }
        let current_auth = load_remote_control_auth(&self.auth_manager)
            .await
            .map_err(|_| pairing_unavailable_error())?;
        if current_auth.account_id != auth.account_id {
            return Err(pairing_unavailable_error());
        }
        pairing_status_response
    }

    pub async fn list_clients(
        &self,
        params: RemoteControlClientsListParams,
    ) -> io::Result<RemoteControlClientsListResponse> {
        self.ensure_remote_control_allowed_io()?;
        clients::list_remote_control_clients(&self.remote_control_url, &self.auth_manager, params)
            .await
    }

    pub async fn revoke_client(
        &self,
        params: RemoteControlClientsRevokeParams,
    ) -> io::Result<RemoteControlClientsRevokeResponse> {
        self.ensure_remote_control_allowed_io()?;
        clients::revoke_remote_control_client(&self.remote_control_url, &self.auth_manager, params)
            .await
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

async fn enroll_pairing_server(
    current_enrollment: &RemoteControlEnrollmentState,
    auth_manager: &auth::RemoteControlAuth,
    auth: &mut auth::RemoteControlConnectionAuth,
    remote_control_target: &protocol::RemoteControlTarget,
    installation_id: &str,
    server_name: &str,
) -> io::Result<RemoteControlEnrollment> {
    match enroll_remote_control_server(remote_control_target, auth, installation_id, server_name)
        .await
    {
        Ok(enrollment) => return Ok(enrollment),
        Err(err) if auth::is_auth_error(&err) => {
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
    current_enrollment.check_retry_after()?;
    enroll_remote_control_server(remote_control_target, auth, installation_id, server_name).await
}

fn remote_control_pairing_status_code(
    params: &RemoteControlPairingStatusParams,
) -> io::Result<RemoteControlPairingStatusCode> {
    match (&params.pairing_code, &params.manual_pairing_code) {
        (Some(pairing_code), None) => Ok(RemoteControlPairingStatusCode::PairingCode(
            pairing_code.clone(),
        )),
        (None, Some(manual_pairing_code)) => Ok(RemoteControlPairingStatusCode::ManualPairingCode(
            manual_pairing_code.clone(),
        )),
        (Some(_), Some(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "remote control pairing status accepts either pairingCode or manualPairingCode, not both",
        )),
        (None, None) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "remote control pairing status requires pairingCode or manualPairingCode",
        )),
    }
}

async fn refresh_pairing_enrollment(
    current_enrollment: &mut RemoteControlEnrollmentLease<'_>,
    auth_manager: &auth::RemoteControlAuth,
    auth: &mut auth::RemoteControlConnectionAuth,
    installation_id: &str,
    enrollment: &mut RemoteControlEnrollment,
) -> io::Result<()> {
    current_enrollment.state.check_retry_after()?;
    let mut refresh_result = refresh_remote_control_server(auth, installation_id, enrollment).await;
    if refresh_result.as_ref().is_err_and(auth::is_auth_error) {
        let mut auth_recovery = auth_manager.unauthorized_recovery();
        let mut auth_change_rx = auth_manager.auth_change_receiver();
        if recover_remote_control_auth(&mut auth_recovery, &mut auth_change_rx).await {
            match load_remote_control_auth(auth_manager).await {
                Ok(recovered_auth) if recovered_auth.account_id == enrollment.account_id => {
                    *auth = recovered_auth;
                    current_enrollment.state.check_retry_after()?;
                    refresh_result =
                        refresh_remote_control_server(auth, installation_id, enrollment).await;
                }
                Ok(_) | Err(_) => {
                    enrollment.clear_server_token();
                    refresh_result = Err(pairing_unavailable_error());
                }
            }
        } else {
            enrollment.clear_server_token();
        }
    }
    if refresh_result.as_ref().is_err_and(auth::is_auth_error) {
        enrollment.clear_server_token();
    }
    if !replace_current_enrollment(current_enrollment, enrollment) {
        Err(pairing_unavailable_error())
    } else {
        current_enrollment.state.record_retry_after(refresh_result)
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

#[cfg(test)]
mod segment_tests;
#[cfg(test)]
mod tests;
