//! Credentials and transport selected with the runtime for one request.

use crate::RuntimeAgentInstance;
use codex_http_client::HttpClientFactory;
use codex_login::AuthChangeState;
use codex_login::CodexAuth;
use std::sync::Arc;

/// Request-scoped credentials and transport selected with their matching runtime.
#[derive(Clone)]
pub struct RuntimeAuth {
    pub(crate) runtime: Arc<RuntimeAgentInstance>,
    pub(crate) auth: CodexAuth,
    pub(crate) http_client_factory: HttpClientFactory,
    pub(crate) change_state: AuthChangeState,
}

impl std::fmt::Debug for RuntimeAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeAuth")
            .field("runtime", &self.runtime)
            .field("auth_mode", &self.auth.api_auth_mode())
            .field("change_state", &self.change_state)
            .finish_non_exhaustive()
    }
}

impl RuntimeAuth {
    pub fn runtime(&self) -> &Arc<RuntimeAgentInstance> {
        &self.runtime
    }

    pub fn auth(&self) -> &CodexAuth {
        &self.auth
    }

    pub fn http_client_factory(&self) -> &HttpClientFactory {
        &self.http_client_factory
    }

    pub fn change_state(&self) -> AuthChangeState {
        self.change_state
    }
}
