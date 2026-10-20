//! Apply runtime auth from the selected request credentials. No credential-only fallback is allowed.

use crate::ActorInfo;
use crate::RegistrationError;
use crate::RuntimeAgentInstance;
use crate::RuntimeAuthError;
use crate::RuntimeAuthManager;
use codex_api::AuthError;
use codex_api::AuthHeadersFuture;
use codex_api::AuthProvider;
use codex_api::AuthProviderFuture;
use codex_features::RuntimeAuthMode;
use codex_http_client::HttpClientFactory;
use codex_http_client::Request;
use codex_login::AuthChangeState;
use codex_login::CodexAuth;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use http::header::AUTHORIZATION;
use std::sync::Arc;

const AGENT_ASSERTION: &str = "x-openai-agent-assertion";

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

/// Auth adapter for OpenAI clients. Share its manager across requests and threads;
/// never select this provider for a third-party endpoint.
pub struct RuntimeAuthProvider {
    manager: Arc<RuntimeAuthManager>,
    mode: RuntimeAuthMode,
}

impl RuntimeAuthProvider {
    pub fn new(manager: Arc<RuntimeAuthManager>, mode: RuntimeAuthMode) -> Self {
        Self { manager, mode }
    }
}

impl AuthProvider for RuntimeAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), AuthError> {
        Err(AuthError::Build(
            "authentication requires async header resolution".to_string(),
        ))
    }

    fn resolve_auth_headers(&self) -> AuthHeadersFuture<'_> {
        Box::pin(async move {
            let auth = self.manager.auth().await?;
            auth.auth_headers(self.mode)
        })
    }

    fn apply_auth(&self, mut request: Request) -> AuthProviderFuture<'_> {
        Box::pin(async move {
            let headers = self.resolve_auth_headers().await?;
            for name in [
                AUTHORIZATION.as_str(),
                AGENT_ASSERTION,
                "chatgpt-account-id",
                "x-openai-fedramp",
            ] {
                request.headers.remove(name);
            }
            request.headers.extend(headers);
            Ok(request)
        })
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

    fn auth_headers(&self, mode: RuntimeAuthMode) -> Result<HeaderMap, AuthError> {
        let assertion = self
            .runtime()
            .assertion()
            .map_err(|_| AuthError::Build("failed to sign runtime assertion".to_string()))?;
        let mut headers = HeaderMap::new();
        match mode {
            RuntimeAuthMode::DualHeader => {
                let token = self.auth().get_token().map_err(|_| {
                    AuthError::Build("runtime actor credential is unavailable".to_string())
                })?;
                headers.insert(AUTHORIZATION, secret_header(&format!("Bearer {token}"))?);
                headers.insert(AGENT_ASSERTION, secret_header(assertion.as_str())?);
            }
            RuntimeAuthMode::AssertionOnly => {
                headers.insert(
                    AUTHORIZATION,
                    secret_header(&format!("AgentAssertion {}", assertion.as_str()))?,
                );
            }
        }
        if let ActorInfo::Chatgpt { account_id, .. } = self.runtime().actor().info() {
            headers.insert(
                "chatgpt-account-id",
                HeaderValue::from_str(account_id)
                    .map_err(|_| AuthError::Build("invalid runtime account header".to_string()))?,
            );
        }
        if self.auth().is_fedramp_account() {
            headers.insert("x-openai-fedramp", HeaderValue::from_static("true"));
        }
        Ok(headers)
    }
}

fn secret_header(value: &str) -> Result<HeaderValue, AuthError> {
    let mut header = HeaderValue::from_str(value)
        .map_err(|_| AuthError::Build("invalid runtime authentication header".to_string()))?;
    header.set_sensitive(true);
    Ok(header)
}

impl From<RuntimeAuthError> for AuthError {
    fn from(error: RuntimeAuthError) -> Self {
        let transient = match &error {
            RuntimeAuthError::Policy(denied)
            | RuntimeAuthError::Registration(RegistrationError::Policy(denied)) => {
                return Self::Policy(*denied);
            }
            RuntimeAuthError::Registration(RegistrationError::Transport) => true,
            RuntimeAuthError::Registration(RegistrationError::Rejected { status, .. }) => {
                status.is_server_error() || *status == StatusCode::TOO_MANY_REQUESTS
            }
            RuntimeAuthError::Unavailable
            | RuntimeAuthError::Unsupported
            | RuntimeAuthError::Changed
            | RuntimeAuthError::RefreshFailed
            | RuntimeAuthError::KeyGeneration
            | RuntimeAuthError::Registration(
                RegistrationError::InvalidEndpoint
                | RegistrationError::InvalidRequest
                | RegistrationError::InvalidResponse,
            ) => false,
        };
        if transient {
            Self::Transient(error.to_string())
        } else {
            Self::Build(error.to_string())
        }
    }
}
