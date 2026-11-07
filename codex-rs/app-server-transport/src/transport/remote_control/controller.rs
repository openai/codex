//! Owns the current login's relay session and the process-facing handle.
//! Replacement happens before cleanup; retired sessions cannot publish into their replacements.

use super::auth::RemoteControlAuth;
use super::*;
use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use tokio_util::task::TaskTracker;

#[derive(Clone)]
pub struct RemoteControlHandle {
    pub(super) inner: Arc<RemoteControl>,
}

struct CurrentSession {
    authenticated: bool,
    session: Arc<RemoteControlSession>,
}

pub(super) struct RemoteControl {
    config: RemoteControlStartConfig,
    state_db: Option<Arc<StateRuntime>>,
    auth_manager: Arc<AuthManager>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown: CancellationToken,
    tasks: TaskTracker,
    current: StdMutex<Option<CurrentSession>>,
    startup: RemoteControlDesiredState,
    persistence: RemoteControlPersistence,
    client_name: RemoteControlPairingPersistenceKey,
    requires_client_name: bool,
    status: watch::Sender<RemoteControlStatusChangedNotification>,
    session_changed: watch::Sender<()>,
}

impl RemoteControl {
    pub(super) fn session(&self) -> Arc<RemoteControlSession> {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(current) = current.as_ref()
            && current.session.auth_manager.owner.is_current()
        {
            return current.session.clone();
        }
        let (auth, authenticated) =
            RemoteControlAuth::capture(self.auth_manager.clone(), self.config.runtime_auth.clone());
        let desired = match current.take() {
            Some(previous) => {
                previous.session.shutdown_token.cancel();
                if previous.authenticated {
                    RemoteControlDesiredState::Disabled
                } else {
                    // Startup or an explicit enable while signed out may wait for first login.
                    *previous.session.desired_state_tx.borrow()
                }
            }
            None => self.startup,
        };
        let session = self.start_session(auth, desired);
        self.status.send_replace(session.status());
        *current = Some(CurrentSession {
            authenticated,
            session: session.clone(),
        });
        self.session_changed.send_replace(());
        session
    }

    fn start_session(
        &self,
        auth_manager: RemoteControlAuth,
        desired: RemoteControlDesiredState,
    ) -> Arc<RemoteControlSession> {
        let shutdown = self.shutdown.child_token();
        let (desired_state_tx, _) = watch::channel(desired);
        let desired_state_tx = Arc::new(desired_state_tx);
        let current_enrollment =
            Arc::new(RemoteControlEnrollmentState::new(/*enrollment*/ None));
        let server_name = gethostname().to_string_lossy().trim().to_string();
        let (status_tx, _) = watch::channel(RemoteControlStatusChangedNotification {
            status: if desired.is_enabled() {
                RemoteControlConnectionStatus::Connecting
            } else {
                RemoteControlConnectionStatus::Disabled
            },
            server_name: server_name.clone(),
            installation_id: self.config.installation_id.clone(),
            environment_id: None,
        });
        let session = Arc::new(RemoteControlSession {
            policy: self.config.policy,
            shutdown_token: shutdown.clone(),
            desired_state_tx: desired_state_tx.clone(),
            desired_state_rpc_lock: Arc::new(Semaphore::new(1)),
            persistence: self.persistence.clone(),
            status_tx: Arc::new(status_tx.clone()),
            state_db: self.state_db.clone(),
            remote_control_url: self.config.remote_control_url.clone(),
            current_enrollment: current_enrollment.clone(),
            pairing_persistence_key: self.client_name.clone(),
            pairing_persistence_key_required: self.requires_client_name,
            auth_manager: auth_manager.clone(),
        });
        let websocket = RemoteControlWebsocket::new(
            websocket::RemoteControlWebsocketConfig {
                remote_control_url: self.config.remote_control_url.clone(),
                installation_id: self.config.installation_id.clone(),
                remote_control_target: None,
                server_name,
            },
            self.state_db.clone(),
            auth_manager,
            RemoteControlChannels {
                transport_event_tx: self.transport_event_tx.clone(),
                status_publisher: RemoteControlStatusPublisher::new(status_tx),
                current_enrollment,
                pairing_persistence_key: self.client_name.clone(),
                persistence: self.persistence.clone(),
            },
            shutdown.clone(),
            desired_state_tx,
        );
        let client_name_rx = if self.requires_client_name {
            let (tx, rx) = oneshot::channel();
            let mut names = self.client_name.subscribe();
            self.tasks.spawn(async move {
                tokio::select! {
                    _ = shutdown.cancelled() => {}
                    name = names.wait_for(Option::is_some) => {
                        if let Ok(name) = name
                            && let Some(name) = name.as_ref()
                        {
                            let _ = tx.send(name.clone());
                        }
                    }
                }
            });
            Some(rx)
        } else {
            None
        };
        let process_shutdown = self.shutdown.clone();
        let failed_session = session.clone();
        self.tasks.spawn(async move {
            if let Err(panic) = AssertUnwindSafe(websocket.run(client_name_rx))
                .catch_unwind()
                .await
            {
                tracing::error!("remote control websocket task panicked");
                failed_session.publish_status(RemoteControlConnectionStatus::Disabled);
                process_shutdown.cancel();
                std::panic::resume_unwind(panic);
            }
        });
        session
    }
}

impl RemoteControlSession {
    async fn run<T>(
        &self,
        operation: impl std::future::Future<Output = io::Result<T>>,
    ) -> io::Result<T> {
        self.auth_manager.ensure_current()?;
        tokio::select! {
            biased;
            _ = self.auth_manager.owner.invalidated() => {
                Err(io::Error::new(io::ErrorKind::Interrupted, "remote control authentication changed"))
            }
            result = operation => {
                self.auth_manager.ensure_current()?;
                result
            }
        }
    }
}

impl RemoteControlHandle {
    pub fn ensure_remote_control_allowed(&self) -> Result<(), RemoteControlDisabledByRequirements> {
        self.inner.session().ensure_remote_control_allowed()
    }

    pub fn status(&self) -> RemoteControlStatusChangedNotification {
        self.inner.session().status()
    }

    pub fn status_receiver(&self) -> watch::Receiver<RemoteControlStatusChangedNotification> {
        self.inner.session();
        self.inner.status.subscribe()
    }

    pub fn enable_ephemeral(
        &self,
    ) -> Result<RemoteControlStatusChangedNotification, RemoteControlEnableError> {
        self.inner.session().enable_ephemeral()
    }

    pub async fn disable_ephemeral(&self) -> RemoteControlStatusChangedNotification {
        self.inner.session().disable_ephemeral().await
    }

    pub async fn enable(
        &self,
        app_server_client_name: Option<&str>,
    ) -> io::Result<RemoteControlStatusChangedNotification> {
        let session = self.inner.session();
        session.run(session.enable(app_server_client_name)).await
    }

    pub async fn disable(
        &self,
        app_server_client_name: Option<&str>,
    ) -> io::Result<RemoteControlStatusChangedNotification> {
        let session = self.inner.session();
        session.run(session.disable(app_server_client_name)).await
    }

    pub async fn resolve_persisted_preference(
        &self,
        app_server_client_name: Option<&str>,
    ) -> io::Result<bool> {
        let session = self.inner.session();
        session
            .run(session.resolve_persisted_preference(app_server_client_name))
            .await
    }

    pub async fn start_pairing(
        &self,
        params: RemoteControlPairingStartParams,
        app_server_client_name: Option<&str>,
    ) -> io::Result<RemoteControlPairingStartResponse> {
        let session = self.inner.session();
        session
            .run(session.start_pairing(params, app_server_client_name))
            .await
    }

    pub async fn pairing_status(
        &self,
        params: RemoteControlPairingStatusParams,
    ) -> io::Result<RemoteControlPairingStatusResponse> {
        let session = self.inner.session();
        session.run(session.pairing_status(params)).await
    }

    pub async fn list_clients(
        &self,
        params: RemoteControlClientsListParams,
    ) -> io::Result<RemoteControlClientsListResponse> {
        let session = self.inner.session();
        session.run(session.list_clients(params)).await
    }

    pub async fn revoke_client(
        &self,
        params: RemoteControlClientsRevokeParams,
    ) -> io::Result<RemoteControlClientsRevokeResponse> {
        let session = self.inner.session();
        session.run(session.revoke_client(params)).await
    }
}

pub async fn start_remote_control(
    config: RemoteControlStartConfig,
    state_db: Option<Arc<StateRuntime>>,
    auth_manager: Arc<AuthManager>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
    app_server_client_name_rx: Option<oneshot::Receiver<String>>,
    startup_mode: RemoteControlStartupMode,
) -> io::Result<(JoinHandle<()>, RemoteControlHandle)> {
    let startup =
        if config.policy == RemoteControlPolicy::DisabledByRequirements || state_db.is_none() {
            RemoteControlDesiredState::Disabled
        } else {
            match startup_mode {
                RemoteControlStartupMode::ResolvePersisted => RemoteControlDesiredState::Unknown,
                RemoteControlStartupMode::DisabledEphemeral => RemoteControlDesiredState::Disabled,
                RemoteControlStartupMode::EnabledEphemeral => {
                    normalize_remote_control_url(&config.remote_control_url)?;
                    RemoteControlDesiredState::Enabled {
                        persistence_preference: None,
                    }
                }
            }
        };
    let (status, _) = watch::channel(RemoteControlStatusChangedNotification {
        status: RemoteControlConnectionStatus::Disabled,
        server_name: gethostname().to_string_lossy().trim().to_string(),
        installation_id: config.installation_id.clone(),
        environment_id: None,
    });
    let inner = Arc::new(RemoteControl {
        config,
        state_db,
        auth_manager,
        transport_event_tx,
        shutdown: shutdown_token,
        tasks: TaskTracker::new(),
        current: StdMutex::new(None),
        startup,
        persistence: RemoteControlPersistence::default(),
        client_name: watch::channel(None).0,
        requires_client_name: app_server_client_name_rx.is_some(),
        status,
        session_changed: watch::channel(()).0,
    });
    inner.session();
    if let Some(rx) = app_server_client_name_rx {
        let names = inner.client_name.clone();
        let shutdown = inner.shutdown.clone();
        inner.tasks.spawn(async move {
            tokio::select! {
                _ = shutdown.cancelled() => {}
                name = rx => match name {
                    Ok(name) => { names.send_replace(Some(name)); }
                    Err(_) => shutdown.cancel(),
                }
            }
        });
    }
    let handle = RemoteControlHandle {
        inner: inner.clone(),
    };
    let task = tokio::spawn(async move {
        let mut session_changed = inner.session_changed.subscribe();
        loop {
            // Reconcile on both API entry and notification delivery, so watcher latency is harmless.
            let session = inner.session();
            let mut status = session.status_receiver();
            session_changed.borrow_and_update();
            {
                let current = inner
                    .current
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if current
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(&current.session, &session))
                    && session.auth_manager.owner.is_current()
                {
                    inner.status.send_if_modified(|current| {
                        let next = status.borrow_and_update().clone();
                        if *current == next {
                            return false;
                        }
                        *current = next;
                        true
                    });
                }
            }
            tokio::select! {
                biased;
                _ = inner.shutdown.cancelled() => break,
                _ = session.auth_manager.owner.invalidated() => {}
                _ = session_changed.changed() => {}
                _ = status.changed() => {}
            }
        }
        inner.tasks.close();
        inner.tasks.wait().await;
        inner.persistence.tasks.close();
        inner.persistence.tasks.wait().await;
    });
    Ok((task, handle))
}
