//! Apply runtime auth from the selected request credentials. No credential-only fallback is allowed.

use crate::ActorInfo;
use crate::RegistrationError;
use crate::RuntimeAgentInstance;
use crate::RuntimeAuthError;
use codex_api::AuthError;
use codex_api::AuthHeaderTelemetry;
use codex_api::AuthHeadersFuture;
use codex_api::AuthProvider;
use codex_features::RuntimeAuthMode;
use codex_http_client::CHATGPT_ACCOUNT_ID_HEADER;
use codex_http_client::HttpClientFactory;
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

/// Request-scoped provider: retries sign fresh assertions from the same selected runtime/credential.
struct RuntimeRequestAuth {
    auth: RuntimeAuth,
    mode: RuntimeAuthMode,
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

    /// Use this selection for a request and its retries; select auth again for the next request.
    pub fn into_auth_provider(self, mode: RuntimeAuthMode) -> codex_api::SharedAuthProvider {
        Arc::new(RuntimeRequestAuth { auth: self, mode })
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
                CHATGPT_ACCOUNT_ID_HEADER,
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

impl AuthProvider for RuntimeRequestAuth {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError> {
        headers.extend(self.auth.auth_headers(self.mode)?);
        Ok(())
    }

    fn auth_header_telemetry(&self) -> Result<AuthHeaderTelemetry, AuthError> {
        Ok(AuthHeaderTelemetry {
            attached: true,
            name: Some("authorization"),
        })
    }

    fn apply_auth_headers(&self, headers: HeaderMap) -> AuthHeadersFuture<'_> {
        Box::pin(async move {
            Ok(replace_auth_headers(
                headers,
                self.auth.auth_headers(self.mode)?,
            ))
        })
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

fn replace_auth_headers(mut headers: HeaderMap, auth: HeaderMap) -> HeaderMap {
    for name in [
        AUTHORIZATION.as_str(),
        AGENT_ASSERTION,
        CHATGPT_ACCOUNT_ID_HEADER,
        "x-openai-fedramp",
    ] {
        headers.remove(name);
    }
    headers.extend(auth);
    headers
}
