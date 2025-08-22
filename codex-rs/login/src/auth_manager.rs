use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use crate::AuthMode;
use crate::CodexAuth;

/// Internal cached auth state.
#[derive(Clone, Debug)]
struct CachedAuth {
    preferred_auth_mode: AuthMode,
    auth: Option<CodexAuth>,
}

/// Central manager providing a single source of truth for auth.json derived
/// authentication data. It loads once (or on preference change) and then
/// hands out cloned `CodexAuth` values so the rest of the program has a
/// consistent snapshot.
///
/// External modifications to `auth.json` will NOT be observed until
/// `reload()` is called explicitly. This matches the design goal of avoiding
/// different parts of the program seeing inconsistent auth data mid‑run.
#[derive(Debug)]
pub struct AuthManager {
    codex_home: PathBuf,
    inner: RwLock<CachedAuth>,
}

impl AuthManager {
    /// Create a new manager loading the initial auth using the provided
    /// preferred auth method. Errors loading auth are swallowed; `auth()` will
    /// simply return `None` in that case so callers can treat it as an
    /// unauthenticated state.
    pub fn new(codex_home: PathBuf, preferred_auth_mode: AuthMode) -> Self {
        let auth = crate::CodexAuth::from_codex_home(&codex_home, preferred_auth_mode)
            .ok()
            .flatten();
        Self {
            codex_home,
            inner: RwLock::new(CachedAuth {
                preferred_auth_mode,
                auth,
            }),
        }
    }

    /// Create an AuthManager with a specific CodexAuth, for testing only.
    pub fn from_auth_for_testing(auth: CodexAuth) -> Arc<Self> {
        let preferred_auth_mode = auth.mode;
        let cached = CachedAuth {
            preferred_auth_mode,
            auth: Some(auth),
        };
        Arc::new(Self {
            codex_home: PathBuf::new(),
            inner: RwLock::new(cached),
        })
    }

    /// Current cached auth (clone). May be `None` if not logged in or load failed.
    pub fn auth(&self) -> Option<CodexAuth> {
        self.inner.read().ok().and_then(|c| c.auth.clone())
    }

    /// Preferred auth method used when (re)loading.
    pub fn preferred_auth_method(&self) -> AuthMode {
        self.inner
            .read()
            .map(|c| c.preferred_auth_mode)
            .unwrap_or(AuthMode::ApiKey)
    }

    /// Force a reload using the existing preferred auth method. Returns
    /// whether the auth value changed.
    pub fn reload(&self) -> bool {
        let preferred = self.preferred_auth_method();
        let new_auth = crate::CodexAuth::from_codex_home(&self.codex_home, preferred)
            .ok()
            .flatten();
        if let Ok(mut guard) = self.inner.write() {
            let changed = !AuthManager::auths_equal(&guard.auth, &new_auth);
            guard.auth = new_auth;
            changed
        } else {
            false
        }
    }

    fn auths_equal(a: &Option<CodexAuth>, b: &Option<CodexAuth>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Convenience constructor returning an `Arc` wrapper.
    pub fn shared(codex_home: PathBuf, preferred_auth_mode: AuthMode) -> Arc<Self> {
        Arc::new(Self::new(codex_home, preferred_auth_mode))
    }

    /// Attempt to refresh the current auth token (if any). On success, reload
    /// the auth state from disk so other components observe refreshed token.
    pub async fn refresh_token(&self) -> std::io::Result<Option<String>> {
        let auth = match self.auth() {
            Some(a) => a,
            None => return Ok(None),
        };
        match auth.refresh_token().await {
            Ok(token) => {
                // Reload to pick up persisted changes.
                self.reload();
                Ok(Some(token))
            }
            Err(e) => Err(e),
        }
    }

    /// Log out by deleting the on‑disk auth.json (if present). Returns Ok(true)
    /// if a file was removed, Ok(false) if no auth file existed. On success,
    /// reloads the in‑memory auth cache so callers immediately observe the
    /// unauthenticated state.
    pub fn logout(&self) -> std::io::Result<bool> {
        let removed = crate::logout(&self.codex_home)?;
        // Always reload to clear any cached auth (even if file absent).
        self.reload();
        Ok(removed)
    }
}
