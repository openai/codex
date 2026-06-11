use super::RemoteControlHandle;
use super::RemoteControlUnavailable;
use super::enroll::update_persisted_remote_control_enrollment;
use super::protocol::normalize_remote_control_url;
use super::publish_current_enrollment;
use super::websocket::RemoteControlStatusPublisher;
use codex_app_server_protocol::RemoteControlStatusChangedNotification;
use std::io;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemoteControlDesiredState {
    // `Unknown` exists only on plain startup before auth and enrollment scope resolve. Persisted
    // `1` is `Enabled { persistence_preference: Some(true) }`; `0`, `NULL`, or no row are
    // `Disabled`. Runtime-only enable is `Enabled { persistence_preference: None }`, so new rows
    // keep `NULL`; durable RPC enable uses `Some(true)`, so new rows get `1`. Durable disable writes
    // `0` before entering `Disabled`; runtime-only disable does not write. `Disabled` carries no
    // preference because disabled sessions do not create enrollments.
    Unknown,
    Disabled,
    Enabled {
        persistence_preference: Option<bool>,
    },
}
impl RemoteControlDesiredState {
    pub(super) fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled { .. })
    }
}

pub(super) async fn acquire_persistence_lock(lock: &Semaphore) -> SemaphorePermit<'_> {
    lock.acquire().await.unwrap_or_else(|_| unreachable!())
}

impl RemoteControlHandle {
    pub async fn enable(
        &self,
        app_server_client_name: Option<&str>,
    ) -> io::Result<RemoteControlStatusChangedNotification> {
        let _transition = self
            .desired_state_rpc_lock
            .acquire()
            .await
            .unwrap_or_else(|_| unreachable!());
        let state_db = self
            .state_db
            .as_deref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, RemoteControlUnavailable))?;
        let mut auth = super::auth::load_remote_control_auth(&self.auth_manager).await?;
        let remote_control_target = normalize_remote_control_url(&self.remote_control_url)?;
        let app_server_client_name = self.pairing_persistence_key(app_server_client_name)?;
        let app_server_client_name = app_server_client_name.as_deref();
        let status = self.status();
        let mut current_enrollment = self.current_enrollment.lock().await;
        let (enrollment, _) = self
            .load_or_enroll_server(
                &current_enrollment,
                &mut auth,
                &status.installation_id,
                &status.server_name,
                app_server_client_name,
                super::RemoteControlEnrollmentSelection::ReuseOrCreate,
            )
            .await?;

        let current_auth = super::auth::load_remote_control_auth(&self.auth_manager).await?;
        if current_auth.account_id != auth.account_id {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "remote control account changed during enrollment",
            ));
        }

        let _persistence = acquire_persistence_lock(&self.desired_state_persistence_lock).await;
        let updated = state_db
            .set_remote_control_enabled(
                &remote_control_target.websocket_url,
                &auth.account_id,
                app_server_client_name,
                /*remote_control_enabled*/ true,
            )
            .await
            .map_err(io::Error::other)?;
        if updated == 0 {
            update_persisted_remote_control_enrollment(
                Some(state_db),
                &remote_control_target,
                &auth.account_id,
                app_server_client_name,
                Some(&enrollment),
                Some(true),
            )
            .await?;
        }
        publish_current_enrollment(&mut current_enrollment, &enrollment);
        self.enable_with_preference(Some(true))
            .map_err(|err| io::Error::new(io::ErrorKind::NotFound, err))?;
        RemoteControlStatusPublisher::new(self.status_tx.as_ref().clone())
            .publish_environment_id(Some(enrollment.environment_id));
        Ok(self.status())
    }
}
