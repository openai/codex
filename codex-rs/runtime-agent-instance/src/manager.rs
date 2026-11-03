//! Reuse one active runtime for the selected credential source and organization.

use crate::AgentHostAuthorization;
use crate::AuthenticatedActorAuthorization;
use crate::PlatformResourceSelection;
use crate::RegistrationError;
use crate::RuntimeAgentInstance;
use crate::RuntimeAuth;
use crate::RuntimeRegistration;
use crate::RuntimeRegistrationClient;
use crate::RuntimeRegistrationMetadata;
use codex_agent_identity::ChatGptEnvironment;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyDenied;
use codex_login::AuthChangeState;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::RefreshTokenError;
use http::StatusCode;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

/// Owned alongside an AuthManager, not created for every thread or request.
pub struct RuntimeAuthManager {
    credentials: Arc<AuthManager>,
    registration: RegistrationTarget,
    metadata: RuntimeRegistrationMetadata,
    state: Mutex<Option<CachedRuntime>>,
}

enum RegistrationTarget {
    Explicit {
        host: AgentHostAuthorization,
        url: Url,
    },
    ChatGpt {
        installation_id: String,
        base_url: String,
    },
}

impl RegistrationTarget {
    fn host(&self) -> Result<AgentHostAuthorization, RegistrationError> {
        match self {
            Self::Explicit { host, .. } => Ok(host.clone()),
            Self::ChatGpt {
                installation_id, ..
            } => Ok(AgentHostAuthorization::InstallationId {
                installation_id: installation_id
                    .parse()
                    .map_err(|_| RegistrationError::InvalidRequest)?,
            }),
        }
    }

    fn url(&self) -> Result<Url, RegistrationError> {
        match self {
            Self::Explicit { url, .. } => Ok(url.clone()),
            Self::ChatGpt { base_url, .. } => {
                let url = match ChatGptEnvironment::from_chatgpt_base_url(base_url)
                    .map_err(|_| RegistrationError::InvalidEndpoint)?
                {
                    ChatGptEnvironment::Production => crate::DEFAULT_REGISTRATION_URL,
                    ChatGptEnvironment::Staging => "https://auth.api.openai.org/agent/register",
                };
                url.parse().map_err(|_| RegistrationError::InvalidEndpoint)
            }
        }
    }
}

struct CachedRuntime {
    source_generation: u64,
    selection: PlatformResourceSelection,
    state: RuntimeState,
}

enum RuntimeState {
    Pending { registration: RuntimeRegistration },
    Ready { runtime: Arc<RuntimeAgentInstance> },
    Denied { error: RegistrationError },
}

impl std::fmt::Debug for RuntimeAuthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeAuthManager").finish_non_exhaustive()
    }
}

impl RuntimeAuthManager {
    /// Check that a consumer uses this owner's credential provider, not an ambient fallback.
    pub fn uses_credentials(&self, credentials: &Arc<AuthManager>) -> bool {
        Arc::ptr_eq(&self.credentials, credentials)
    }

    pub fn new(
        credentials: Arc<AuthManager>,
        host: AgentHostAuthorization,
        metadata: RuntimeRegistrationMetadata,
        registration_url: Url,
    ) -> Self {
        Self {
            credentials,
            registration: RegistrationTarget::Explicit {
                host,
                url: registration_url,
            },
            metadata,
            state: Mutex::new(None),
        }
    }

    /// Resolve the ChatGPT registration origin only when a first-party request needs a runtime.
    pub fn for_chatgpt(
        credentials: Arc<AuthManager>,
        installation_id: String,
        base_url: String,
        metadata: RuntimeRegistrationMetadata,
    ) -> Self {
        Self {
            credentials,
            registration: RegistrationTarget::ChatGpt {
                installation_id,
                base_url,
            },
            metadata,
            state: Mutex::new(None),
        }
    }

    pub async fn auth(&self) -> Result<RuntimeAuth, RuntimeAuthError> {
        self.auth_with_selection(&PlatformResourceSelection::default())
            .await
    }

    /// Select auth and a runtime for the requested organization and project.
    pub async fn auth_with_selection(
        &self,
        selection: &PlatformResourceSelection,
    ) -> Result<RuntimeAuth, RuntimeAuthError> {
        self.select_auth(selection, /*expected_source*/ None).await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "runtime initialization serializes registration and refresh for one credential source"
    )]
    async fn select_auth(
        &self,
        selection: &PlatformResourceSelection,
        expected_source: Option<u64>,
    ) -> Result<RuntimeAuth, RuntimeAuthError> {
        // This initializer's lock can span IO. No AuthManager lock is held across registration.
        let mut state = self.state.lock().await;
        self.check_source(expected_source)?;
        let (auth, http_client_factory) = self
            .credentials
            .auth_with_http_client_factory()
            .await
            .ok_or(RuntimeAuthError::Unavailable)?;
        let mut recovery = self.credentials.unauthorized_recovery();
        let mut result = self
            .auth_once(
                &mut state,
                auth,
                http_client_factory,
                selection,
                expected_source,
            )
            .await;
        while matches!(
            &result,
            Err(RuntimeAuthError::Registration(
                RegistrationError::Rejected {
                    status: StatusCode::UNAUTHORIZED,
                    ..
                }
            ))
        ) && recovery.has_next()
        {
            self.check_source(expected_source)?;
            recovery.next().await.map_err(|error| match error {
                RefreshTokenError::Policy(denied) => RuntimeAuthError::Policy(denied),
                RefreshTokenError::Permanent(_) | RefreshTokenError::Transient(_) => {
                    RuntimeAuthError::RefreshFailed
                }
            })?;
            let (auth, http_client_factory) = self
                .credentials
                .auth_with_http_client_factory()
                .await
                .ok_or(RuntimeAuthError::Unavailable)?;
            result = self
                .auth_once(
                    &mut state,
                    auth,
                    http_client_factory,
                    selection,
                    expected_source,
                )
                .await;
        }
        result
    }

    async fn auth_once(
        &self,
        state: &mut Option<CachedRuntime>,
        auth: CodexAuth,
        http_client_factory: HttpClientFactory,
        selection: &PlatformResourceSelection,
        expected_source: Option<u64>,
    ) -> Result<RuntimeAuth, RuntimeAuthError> {
        let actor_auth = actor_authorization(&auth, selection)?;
        let http_client_factory = http_client_factory.with_sensitive_header_protection();
        let change_state = self.auth_state(&auth)?;
        self.check_source(expected_source)?;
        // Registered runtimes can serve different projects. Pending and denied
        // registrations retain their original scope and idempotency key.
        if state.as_ref().is_some_and(|cached| {
            cached.source_generation != change_state.source_generation
                || cached.selection.organization_id != selection.organization_id
                || (!matches!(cached.state, RuntimeState::Ready { .. })
                    && cached.selection != *selection)
        }) {
            *state = None;
        }
        let cached = match state {
            Some(cached) => cached,
            None => state.insert(CachedRuntime {
                source_generation: change_state.source_generation,
                selection: selection.clone(),
                state: RuntimeState::Pending {
                    registration: RuntimeRegistration::new(
                        actor_auth.clone(),
                        self.registration.host()?,
                        self.metadata.clone(),
                    )
                    .map_err(|_| RuntimeAuthError::KeyGeneration)?,
                },
            }),
        };
        let runtime_state = &mut cached.state;
        let registration = match runtime_state {
            RuntimeState::Ready { runtime } => {
                return Ok(RuntimeAuth {
                    runtime: runtime.clone(),
                    auth,
                    http_client_factory,
                    change_state,
                });
            }
            RuntimeState::Denied { error } => return Err(error.clone().into()),
            RuntimeState::Pending { registration } => {
                registration.request.actor_auth = actor_auth;
                registration
            }
        };
        let client =
            RuntimeRegistrationClient::new(http_client_factory.clone(), self.registration.url()?)?;
        let result = client.register(registration).await;
        if self.auth_state(&auth)?.source_generation != change_state.source_generation {
            // A late response must not publish an old identity after a credential switch.
            *state = None;
            return Err(RuntimeAuthError::Changed);
        }
        match result {
            Ok(info) => {
                let runtime = Arc::new(registration.finish(info));
                *runtime_state = RuntimeState::Ready {
                    runtime: runtime.clone(),
                };
                Ok(RuntimeAuth {
                    runtime,
                    auth,
                    http_client_factory,
                    change_state,
                })
            }
            Err(
                error @ RegistrationError::Rejected {
                    status: StatusCode::FORBIDDEN,
                    ..
                },
            ) => {
                *runtime_state = RuntimeState::Denied {
                    error: error.clone(),
                };
                Err(error.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Resolve auth for work whose routing, cache key, or payload uses a captured credential.
    /// A same-source refresh is allowed; a different source must not take over that work.
    /// This binds credential identity, not when the caller captured it. Restoring the exact
    /// same credential can select a new runtime after logout; a different source cannot.
    pub async fn auth_for(
        &self,
        expected: &CodexAuth,
        selection: &PlatformResourceSelection,
    ) -> Result<RuntimeAuth, RuntimeAuthError> {
        let source = self
            .credentials
            .auth_change_state_for(expected)
            .ok_or(RuntimeAuthError::Changed)?;
        self.auth_for_source(source.source_generation, selection)
            .await
    }

    /// Select another request within a reusable client's original credential source.
    pub async fn auth_for_source(
        &self,
        source_generation: u64,
        selection: &PlatformResourceSelection,
    ) -> Result<RuntimeAuth, RuntimeAuthError> {
        self.select_auth(selection, Some(source_generation)).await
    }

    fn check_source(&self, expected: Option<u64>) -> Result<(), RuntimeAuthError> {
        if expected.is_some_and(|expected| {
            self.credentials
                .auth_change_state_receiver()
                .borrow()
                .source_generation
                != expected
        }) {
            return Err(RuntimeAuthError::Changed);
        }
        Ok(())
    }

    fn auth_state(&self, auth: &CodexAuth) -> Result<AuthChangeState, RuntimeAuthError> {
        let state = self
            .credentials
            .auth_change_state_for(auth)
            .ok_or(RuntimeAuthError::Changed)?;
        if self.credentials.refresh_failure_for_auth(auth).is_some() {
            return Err(RuntimeAuthError::RefreshFailed);
        }
        Ok(state)
    }
}

fn actor_authorization(
    auth: &CodexAuth,
    selection: &PlatformResourceSelection,
) -> Result<AuthenticatedActorAuthorization, RuntimeAuthError> {
    if !matches!(auth, CodexAuth::ApiKey(_)) && selection != &PlatformResourceSelection::default() {
        return Err(RuntimeAuthError::UnsupportedResourceSelection);
    }
    let credential = auth
        .get_token()
        .map_err(|_| RuntimeAuthError::Unsupported)?
        .into();
    Ok(match auth {
        CodexAuth::ApiKey(_) => AuthenticatedActorAuthorization::PlatformApiKey {
            credential,
            selection: selection.clone(),
        },
        CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_) => {
            AuthenticatedActorAuthorization::Oauth { credential }
        }
        CodexAuth::PersonalAccessToken(_) => {
            AuthenticatedActorAuthorization::PersonalAccessToken { credential }
        }
        CodexAuth::Headers(_)
        | CodexAuth::AgentIdentity(_)
        | CodexAuth::BedrockApiKey(_)
        | CodexAuth::BedrockAccessKeys(_) => return Err(RuntimeAuthError::Unsupported),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeAuthError {
    #[error("runtime registration requires an actor credential")]
    Unavailable,
    #[error("this credential cannot register a runtime; use a supported OpenAI sign-in")]
    Unsupported,
    #[error("organization and project selection requires an OpenAI API key")]
    UnsupportedResourceSelection,
    #[error("credentials changed during runtime registration; try again")]
    Changed,
    #[error("runtime registration could not refresh actor credentials")]
    RefreshFailed,
    #[error("failed to generate runtime signing key")]
    KeyGeneration,
    #[error(transparent)]
    Policy(#[from] NetworkPolicyDenied),
    #[error(transparent)]
    Registration(RegistrationError),
}

impl From<RegistrationError> for RuntimeAuthError {
    fn from(error: RegistrationError) -> Self {
        match error {
            RegistrationError::Policy(denied) => Self::Policy(denied),
            error => Self::Registration(error),
        }
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
