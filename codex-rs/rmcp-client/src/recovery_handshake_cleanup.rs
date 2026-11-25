//! Owns sessions allocated during reply-expiry recovery until RMCP accepts the handshake.
//!
//! An attempt allocates at most one session. Its guard and RMCP share one owned cleanup task:
//! retries wait for that task, while cancellation detaches it without losing the DELETE claim.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use http::HeaderName;
use http::HeaderValue;
use rmcp::transport::auth::AuthError;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::StreamableHttpClientAdapter;
use super::StreamableHttpClientAdapterError;
use crate::oauth::OAuthRuntime;

type CleanupResult = Result<(), StreamableHttpError<StreamableHttpClientAdapterError>>;

#[derive(Clone)]
pub(crate) struct RecoveryHandshakeCleanup {
    state: Arc<Mutex<State>>,
    runtime: Option<tokio::runtime::Handle>,
}

#[derive(Default)]
struct State {
    allocation: Option<Allocation>,
    oauth: Option<OAuthRuntime>,
    refreshes: Vec<CancellationToken>,
    rejected: Vec<String>,
    completed: bool,
    cancelled: bool,
}

#[derive(Clone)]
struct Allocation {
    adapter: StreamableHttpClientAdapter,
    uri: Arc<str>,
    session: Arc<str>,
    auth: Option<String>,
    headers: HashMap<HeaderName, HeaderValue>,
    done: Option<CancellationToken>,
}

pub(crate) struct RecoveryHandshakeGuard {
    cleanup: RecoveryHandshakeCleanup,
    completed: bool,
}

pub(super) enum SdkDeleteClaim {
    Untracked,
    AlreadyClaimed(CancellationToken),
    Owned(JoinHandle<CleanupResult>),
}

impl RecoveryHandshakeGuard {
    pub(crate) fn new() -> Self {
        Self {
            cleanup: RecoveryHandshakeCleanup {
                state: Arc::default(),
                runtime: tokio::runtime::Handle::try_current().ok(),
            },
            completed: false,
        }
    }

    pub(crate) fn handle(&self) -> RecoveryHandshakeCleanup {
        self.cleanup.clone()
    }

    pub(crate) fn complete(mut self) {
        let mut state = self
            .cleanup
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        state.completed = true;
        state.allocation = None;
        state.oauth = None;
        state.refreshes.clear();
        state.rejected.clear();
        self.completed = true;
    }
}

impl Drop for RecoveryHandshakeGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.cleanup.cancel();
        }
    }
}

impl RecoveryHandshakeCleanup {
    pub(super) async fn resolve_token(&self, fallback: Option<String>) -> Option<String> {
        let Some(oauth) = self.authorization_state().0 else {
            return self.current_cleanup_token(fallback);
        };
        let mut waited = 0;
        loop {
            let (_, refreshes) = self.authorization_state();
            for refresh in &refreshes[waited..] {
                refresh.cancelled().await;
            }
            waited = refreshes.len();
            let token = oauth.cleanup_access_token().await;
            let (_, refreshes) = self.authorization_state();
            if refreshes.len() > waited {
                continue;
            }
            return self.current_cleanup_token(token);
        }
    }

    fn authorization_state(&self) -> (Option<OAuthRuntime>, Vec<CancellationToken>) {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        (state.oauth.clone(), state.refreshes.clone())
    }

    fn current_cleanup_token(&self, token: Option<String>) -> Option<String> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        token.filter(|token| !state.rejected.contains(token))
    }

    pub(super) fn reject(&self, session: &Arc<str>, token: &str) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .allocation
            .as_ref()
            .is_some_and(|allocation| allocation.session == *session)
            && !state.rejected.iter().any(|rejected| rejected == token)
        {
            state.rejected.push(token.to_string());
        }
    }

    pub(super) fn set_oauth(&self, oauth: OAuthRuntime) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .oauth = Some(oauth);
    }

    pub(crate) async fn refresh(
        &self,
        oauth: &OAuthRuntime,
        rejected: &str,
    ) -> Result<String, AuthError> {
        let done = CancellationToken::new();
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .refreshes
            .push(done.clone());
        let oauth = oauth.clone();
        let rejected = rejected.to_string();
        // Own the operation before Legacy begins waiting for its credential transaction lock.
        let complete = done.drop_guard();
        let refresh = tokio::spawn(async move {
            let _complete = complete;
            oauth.refresh_access_token(&rejected).await
        });
        refresh
            .await
            .map_err(|_| AuthError::InternalError("MCP OAuth refresh task failed".to_string()))?
    }

    pub(super) fn register(
        &self,
        mut adapter: StreamableHttpClientAdapter,
        uri: Arc<str>,
        session: Arc<str>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) {
        adapter.handshake_cleanup = None;
        adapter.initialize_cancellation = None;
        let cancelled = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.completed {
                return;
            }
            debug_assert!(
                state
                    .allocation
                    .as_ref()
                    .is_none_or(|allocation| allocation.session == session)
            );
            state.allocation.get_or_insert(Allocation {
                adapter,
                uri,
                session,
                auth,
                headers,
                done: None,
            });
            state.cancelled
        };
        if cancelled {
            self.cancel();
        }
    }

    pub(super) fn update_auth_and_headers(
        &self,
        session: &Arc<str>,
        auth: &Option<String>,
        headers: &HashMap<HeaderName, HeaderValue>,
    ) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(allocation) = state
            .allocation
            .as_mut()
            .filter(|allocation| allocation.session == *session)
        {
            if auth.is_some() {
                allocation.auth.clone_from(auth);
            }
            allocation.headers.extend(headers.clone());
        }
    }

    pub(crate) fn tracks(&self, session: &Arc<str>) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .allocation
            .as_ref()
            .is_some_and(|allocation| allocation.session == *session)
    }

    pub(super) fn claim_sdk_delete(&self, session: &Arc<str>) -> SdkDeleteClaim {
        let (allocation, done, runtime) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(allocation) = state
                .allocation
                .as_mut()
                .filter(|allocation| allocation.session == *session)
            else {
                return SdkDeleteClaim::Untracked;
            };
            if let Some(done) = &allocation.done {
                return SdkDeleteClaim::AlreadyClaimed(done.clone());
            }
            let Some(runtime) = self
                .runtime
                .clone()
                .or_else(|| tokio::runtime::Handle::try_current().ok())
            else {
                return SdkDeleteClaim::Untracked;
            };
            let done = CancellationToken::new();
            allocation.done = Some(done.clone());
            (allocation.clone(), done, runtime)
        };
        let complete = done.drop_guard();
        let cleanup = self.clone();
        SdkDeleteClaim::Owned(runtime.spawn(async move {
            let _complete = complete;
            allocation
                .adapter
                .cleanup_session(
                    allocation.uri,
                    allocation.session,
                    allocation.auth,
                    allocation.headers,
                    cleanup,
                )
                .await
        }))
    }

    fn cancel(&self) {
        let session = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.cancelled = true;
            state
                .allocation
                .as_ref()
                .map(|allocation| Arc::clone(&allocation.session))
        };
        if let Some(session) = session {
            match self.claim_sdk_delete(&session) {
                SdkDeleteClaim::Owned(task) => {
                    drop(task);
                }
                SdkDeleteClaim::AlreadyClaimed(_) | SdkDeleteClaim::Untracked => {}
            }
        }
    }

    pub(crate) async fn wait_after_failure(&self) {
        let done = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .allocation
            .as_ref()
            .and_then(|allocation| allocation.done.clone());
        if let Some(done) = done {
            done.cancelled().await;
        }
    }
}

#[cfg(test)]
#[path = "recovery_handshake_cleanup_tests.rs"]
mod tests;
