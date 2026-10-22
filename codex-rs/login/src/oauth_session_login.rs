//! Coordinates gateway login, readiness, and status notifications.
//! Reports login progress before credential I/O and success only after persistence.
//! Readiness I/O leaves the cache unlocked and preserves newer state.

use super::*;
use std::future::Future;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::OwnedMutexGuard;
use tokio::sync::broadcast;

/// Readiness and browser authorization progress for a gateway credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OAuthSessionStatus {
    NotReady,
    Started,
    Succeeded,
    Failed { message: String },
}

// Keep terminal status tied to its credential and retain successful credential expiry for busy reads.
// Deliberately omit Debug: this snapshot contains credentials.
#[derive(Clone)]
pub(super) struct OAuthSessionStatusSnapshot {
    pub(super) status: OAuthSessionStatus,
    pub(super) credential: Option<StoredToken>,
}

/// A credential status change scoped to the configured OAuth account.
#[derive(Clone, Debug)]
pub struct OAuthSessionStatusChange {
    pub config: OAuthSessionConfig,
    pub status: OAuthSessionStatus,
}

/// Browser policy shared by gateway managers with the same home and network configuration.
#[derive(Debug)]
pub struct OAuthLoginControl {
    pub(super) explicit_login: AtomicBool,
    status_tx: broadcast::Sender<OAuthSessionStatusChange>,
}

impl OAuthLoginControl {
    /// Returns the control shared across this runtime's provider configurations.
    pub fn for_runtime(runtime: &crate::AuthRuntimeConfig) -> Arc<Self> {
        Self::for_host(
            &runtime.codex_home,
            runtime.auth_route_config.http_client_factory(),
        )
    }

    pub(super) fn for_host(codex_home: &std::path::Path, factory: &HttpClientFactory) -> Arc<Self> {
        type Controls = Vec<(PathBuf, HttpClientFactory, Arc<OAuthLoginControl>)>;
        static CONTROLS: OnceLock<std::sync::Mutex<Controls>> = OnceLock::new();
        let mut controls = CONTROLS
            .get_or_init(std::sync::Mutex::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Keep subscriptions alive between manager lifetimes, including before a gateway is configured.
        controls.retain(|(_, _, control)| {
            Arc::strong_count(control) > 1
                || control.status_tx.strong_count() > 1
                || control.status_tx.receiver_count() > 0
        });
        if let Some((_, _, control)) = controls
            .iter()
            .find(|(home, routes, _)| home == codex_home && routes == factory)
        {
            return Arc::clone(control);
        }
        let control = Arc::new(Self {
            explicit_login: AtomicBool::new(/*v*/ false),
            status_tx: broadcast::channel(/*capacity*/ 16).0,
        });
        controls.push((
            codex_home.to_path_buf(),
            factory.clone(),
            Arc::clone(&control),
        ));
        control
    }

    /// Requires caller-initiated browser login, including during app-server startup.
    pub fn require_explicit_login(&self) {
        self.explicit_login.store(/*val*/ true, Ordering::Relaxed);
    }

    /// Restores automatic browser authorization for legacy clients after initialization.
    pub fn allow_automatic_login(&self) {
        self.explicit_login.store(/*val*/ false, Ordering::Relaxed);
    }
}

/// Subscribes to gateway status changes for this home and network configuration.
/// The subscription also covers managers created after configuration changes.
pub fn subscribe_oauth_session_status(
    runtime: &crate::AuthRuntimeConfig,
) -> broadcast::Receiver<OAuthSessionStatusChange> {
    OAuthLoginControl::for_runtime(runtime)
        .status_tx
        .subscribe()
}

pub(super) fn status_sender_for_host(
    codex_home: &std::path::Path,
    factory: &HttpClientFactory,
) -> broadcast::Sender<OAuthSessionStatusChange> {
    OAuthLoginControl::for_host(codex_home, factory)
        .status_tx
        .clone()
}

impl OAuthSessionManager {
    /// Identifies the persisted sign-in without refreshing an expired access token.
    /// Refresh preserves this ID; explicit login creates a new one.
    pub async fn authorization_id(&self) -> io::Result<String> {
        let _lock = storage::lock_credentials(&self.state.codex_home).await?;
        self.load_token_async()
            .await?
            .and_then(|token| token.authorization_id)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    OAuthSessionError::LoginRequired,
                )
            })
    }

    pub async fn login(&self) -> io::Result<()> {
        self.login_with_browser(std::future::pending(), open_browser)
            .await
    }

    /// Removes the persisted grant and its in-memory copy without contacting the issuer.
    pub async fn logout(&self) -> io::Result<bool> {
        let mut cached = self.lock_cached_token().await?;
        let credential_lock = storage::lock_credentials(&self.state.codex_home).await?;
        let manager = self.clone();
        tokio::task::spawn_blocking(move || {
            let _credential_lock = credential_lock;
            let removed = manager.state.storage.delete(&manager.credential_id())?;
            *cached = OAuthSessionCache::default();
            manager.publish_status(OAuthSessionStatus::NotReady, /*credential*/ None);
            Ok(removed)
        })
        .await
        .map_err(|_| io::Error::other("provider OAuth credential removal task failed"))?
    }

    /// Returns the OAuth account configuration used to scope status notifications.
    pub fn config(&self) -> &OAuthSessionConfig {
        &self.state.config
    }

    /// Returns whether the status changed and a notification was published.
    pub(super) fn publish_status(
        &self,
        status: OAuthSessionStatus,
        credential: Option<&StoredToken>,
    ) -> bool {
        let credential = match status {
            OAuthSessionStatus::NotReady
            | OAuthSessionStatus::Failed { .. }
            | OAuthSessionStatus::Succeeded => credential.cloned(),
            OAuthSessionStatus::Started => None,
        };
        let mut changed = false;
        self.state.status.send_if_modified(|previous| {
            changed = previous
                .as_ref()
                .is_none_or(|previous| previous.status != status);
            if !changed
                && previous
                    .as_ref()
                    .is_some_and(|previous| previous.credential == credential)
            {
                return false;
            }
            *previous = Some(OAuthSessionStatusSnapshot {
                status: status.clone(),
                credential,
            });
            if changed {
                let _ = self.state.status_tx.send(OAuthSessionStatusChange {
                    config: self.state.config.clone(),
                    status: status.clone(),
                });
            }
            true
        });
        changed
    }

    pub(super) fn login_required_error(&self, credential: Option<&StoredToken>) -> io::Error {
        // Requests must re-prompt even after an earlier NotReady notification was dismissed.
        // Passive readiness reads continue to use the deduplicated publisher above.
        if !self.publish_status(OAuthSessionStatus::NotReady, credential) {
            let _ = self.state.status_tx.send(OAuthSessionStatusChange {
                config: self.state.config.clone(),
                status: OAuthSessionStatus::NotReady,
            });
        }
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            OAuthSessionError::LoginRequired,
        )
    }

    pub(super) async fn lock_cached_token(&self) -> io::Result<OwnedMutexGuard<OAuthSessionCache>> {
        let mut status = self.state.status.subscribe();
        let lock = Arc::clone(&self.state.cached_token).lock_owned();
        tokio::pin!(lock);
        loop {
            if status
                .borrow_and_update()
                .as_ref()
                .is_some_and(|snapshot| snapshot.status == OAuthSessionStatus::Started)
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    OAuthSessionError::LoginInProgress,
                ));
            }
            tokio::select! {
                biased;
                _ = status.changed() => {}
                guard = &mut lock => return Ok(guard),
            }
        }
    }

    /// Reads readiness without refreshing credentials, opening a browser, or waiting for login.
    pub async fn status(&self) -> io::Result<OAuthSessionStatus> {
        let manager = self.clone();
        tokio::task::spawn_blocking(move || manager.status_blocking())
            .await
            .map_err(|_| io::Error::other("provider OAuth readiness task failed"))?
    }

    fn status_blocking(&self) -> io::Result<OAuthSessionStatus> {
        let mut status_changes = self.state.status.subscribe();
        let previous_cache = {
            let Ok(cached) = self.state.cached_token.try_lock() else {
                return Ok(self.snapshot_status());
            };
            if status_changes
                .borrow_and_update()
                .as_ref()
                .is_some_and(|snapshot| snapshot.status == OAuthSessionStatus::Started)
            {
                return Ok(OAuthSessionStatus::Started);
            }
            cached.clone()
        };
        // Keyring I/O may stall even after the caller drops this readiness request.
        // Leave the cache available to inference and concurrent login/refresh work.
        let stored = self.load_token();
        let Ok(mut cached) = self.state.cached_token.try_lock() else {
            return Ok(self.snapshot_status());
        };
        if *cached != previous_cache
            || status_changes.has_changed().unwrap_or(/*default*/ true)
        {
            return Ok(self.snapshot_status());
        }
        let snapshot = status_changes.borrow().clone();
        let stored = stored?;
        let terminal = snapshot.as_ref().filter(|snapshot| {
            matches!(
                snapshot.status,
                OAuthSessionStatus::NotReady | OAuthSessionStatus::Failed { .. }
            )
        });
        if stored.as_ref().is_some_and(token_is_usable)
            && (cached.pending.is_none() || stored != cached.token)
            && stored != cached.login_required
            && terminal.is_none_or(|snapshot| stored != snapshot.credential)
        {
            // Only a persisted replacement supersedes a failed-save baseline or rejected token.
            cached.token = stored;
            cached.pending = None;
            cached.login_required = None;
            self.publish_status(OAuthSessionStatus::Succeeded, cached.token.as_ref());
            return Ok(OAuthSessionStatus::Succeeded);
        }
        if let Some(snapshot) = terminal {
            return Ok(snapshot.status.clone());
        }
        self.publish_status(OAuthSessionStatus::NotReady, stored.as_ref());
        Ok(OAuthSessionStatus::NotReady)
    }

    fn snapshot_status(&self) -> OAuthSessionStatus {
        let mut status = OAuthSessionStatus::NotReady;
        // Check and downgrade atomically so an expired observation cannot overwrite
        // a completed refresh or newly started login.
        self.state.status.send_if_modified(|snapshot| {
            let Some(snapshot) = snapshot else {
                return false;
            };
            let expired = snapshot.status == OAuthSessionStatus::Succeeded
                && !snapshot.credential.as_ref().is_some_and(token_is_usable);
            if expired {
                snapshot.status = OAuthSessionStatus::NotReady;
                let _ = self.state.status_tx.send(OAuthSessionStatusChange {
                    config: self.state.config.clone(),
                    status: OAuthSessionStatus::NotReady,
                });
            }
            status = snapshot.status.clone();
            expired
        });
        status
    }

    /// Starts browser authorization using a caller-provided URL handoff and cancellation future.
    /// Completion means the credential was saved and is visible to existing request consumers.
    pub async fn login_with_browser(
        &self,
        cancel: impl Future<Output = ()>,
        open_browser: impl FnOnce(&Url),
    ) -> io::Result<()> {
        validate_config(&self.state.config)?;
        let _login_attempt = Arc::clone(&self.state.login_attempt)
            .try_lock_owned()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    OAuthSessionError::LoginInProgress,
                )
            })?;
        tokio::pin!(cancel);
        let mut cached = tokio::select! {
            biased;
            cached = Arc::clone(&self.state.cached_token).lock_owned() => cached,
            _ = &mut cancel => return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Gateway sign-in was canceled",
            )),
        };
        let mut guard = LoginStatusGuard {
            manager: self,
            credential: cached.token.clone(),
            completed: false,
        };
        self.publish_status(OAuthSessionStatus::Started, /*credential*/ None);
        let result = tokio::select! {
            biased;
            _ = &mut cancel => Err(io::Error::new(io::ErrorKind::Interrupted, "Gateway sign-in was canceled")),
            result = async {
                // A blocking read may outlive cancellation; it must not retain the login locks.
                guard.credential = self.load_token_async().await?;
                self.authorize_with_browser(&mut cached, open_browser).await.map(|_| ())
            } => result,
        }
        .map_err(|error| {
            // Issuer errors may echo arbitrary credentials. Only retain safe text in the
            // returned error and shared status, which is broadcast and can be debug-printed.
            let message = if error.kind() == io::ErrorKind::Interrupted {
                "Gateway sign-in was canceled"
            } else {
                "Gateway sign-in failed; check the gateway configuration and credential store."
            };
            io::Error::new(error.kind(), message)
        });
        if result.is_ok() {
            // Success describes the newly saved credential, not the pre-login baseline.
            guard.credential = cached.token.clone();
        }
        guard.finish(&result);
        result
    }
}

struct LoginStatusGuard<'a> {
    manager: &'a OAuthSessionManager,
    credential: Option<StoredToken>,
    completed: bool,
}

impl LoginStatusGuard<'_> {
    fn finish<T>(&mut self, result: &io::Result<T>) {
        self.manager.publish_status(
            match result {
                Ok(_) => OAuthSessionStatus::Succeeded,
                Err(error) => OAuthSessionStatus::Failed {
                    message: error.to_string(),
                },
            },
            self.credential.as_ref(),
        );
        self.completed = true;
    }
}

impl Drop for LoginStatusGuard<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.manager.publish_status(
                OAuthSessionStatus::Failed {
                    message: "Gateway sign-in was canceled".to_string(),
                },
                self.credential.as_ref(),
            );
        }
    }
}

// The legacy CLI/TUI flow hands browser authorization directly to the user.
pub(super) fn open_browser(url: &Url) {
    eprintln!("Authorize the model provider by opening this URL:\n{url}\n");
    if webbrowser::open(url.as_str()).is_err() {
        eprintln!("Browser launch failed; open the URL above manually.");
    }
}
