//! Share one runtime for the current credential source, including bootstrap retries and refreshes.

use crate::AgentHostAuthorization;
use crate::AuthenticatedActorAuthorization;
use crate::RegistrationError;
use crate::RuntimeAgentInstance;
use crate::RuntimeAuth;
use crate::RuntimeRegistration;
use crate::RuntimeRegistrationClient;
use crate::RuntimeRegistrationMetadata;
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
    host: AgentHostAuthorization,
    metadata: RuntimeRegistrationMetadata,
    registration_url: Url,
    state: Mutex<Option<RuntimeState>>,
}

enum RuntimeState {
    Pending {
        source_generation: u64,
        registration: RuntimeRegistration,
    },
    Ready {
        source_generation: u64,
        runtime: Arc<RuntimeAgentInstance>,
    },
    Denied {
        source_generation: u64,
        error: RegistrationError,
    },
}

impl RuntimeAuthManager {
    pub fn new(
        credentials: Arc<AuthManager>,
        host: AgentHostAuthorization,
        metadata: RuntimeRegistrationMetadata,
        registration_url: Url,
    ) -> Self {
        Self {
            credentials,
            host,
            metadata,
            registration_url,
            state: Mutex::new(None),
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "runtime initialization serializes registration and refresh for one credential source"
    )]
    pub async fn auth(&self) -> Result<RuntimeAuth, RuntimeAuthError> {
        // This initializer's lock can span IO. No AuthManager lock is held across registration.
        let mut state = self.state.lock().await;
        let (auth, http_client_factory) = self
            .credentials
            .auth_with_http_client_factory()
            .await
            .ok_or(RuntimeAuthError::Unavailable)?;
        let mut recovery = self.credentials.unauthorized_recovery();
        let mut result = self.auth_once(&mut state, auth, http_client_factory).await;
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
            result = self.auth_once(&mut state, auth, http_client_factory).await;
        }
        result
    }

    async fn auth_once(
        &self,
        state: &mut Option<RuntimeState>,
        auth: CodexAuth,
        http_client_factory: HttpClientFactory,
    ) -> Result<RuntimeAuth, RuntimeAuthError> {
        let actor_auth = actor_authorization(&auth)?;
        let change_state = self.auth_state(&auth)?;
        let same_source = match state.as_ref() {
            Some(
                RuntimeState::Pending {
                    source_generation, ..
                }
                | RuntimeState::Ready {
                    source_generation, ..
                }
                | RuntimeState::Denied {
                    source_generation, ..
                },
            ) => *source_generation == change_state.source_generation,
            None => false,
        };
        if !same_source {
            *state = Some(RuntimeState::Pending {
                registration: RuntimeRegistration::new(
                    actor_auth.clone(),
                    self.host.clone(),
                    self.metadata.clone(),
                )
                .map_err(|_| RuntimeAuthError::KeyGeneration)?,
                source_generation: change_state.source_generation,
            });
        }
        let registration = match state.as_mut().ok_or(RuntimeAuthError::Unavailable)? {
            RuntimeState::Ready { runtime, .. } => {
                return Ok(RuntimeAuth {
                    runtime: runtime.clone(),
                    auth,
                    http_client_factory,
                    change_state,
                });
            }
            RuntimeState::Denied { error, .. } => return Err(error.clone().into()),
            RuntimeState::Pending { registration, .. } => {
                registration.request.actor_auth = actor_auth;
                registration
            }
        };
        let client = RuntimeRegistrationClient::new(
            http_client_factory.clone(),
            self.registration_url.clone(),
        )?;
        let result = client.register(registration).await;
        if self.auth_state(&auth)?.source_generation != change_state.source_generation {
            // A late response must not publish an old identity after a credential switch.
            *state = None;
            return Err(RuntimeAuthError::Changed);
        }
        match result {
            Ok(info) => {
                let runtime = Arc::new(registration.finish(info));
                *state = Some(RuntimeState::Ready {
                    runtime: runtime.clone(),
                    source_generation: change_state.source_generation,
                });
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
                *state = Some(RuntimeState::Denied {
                    source_generation: change_state.source_generation,
                    error: error.clone(),
                });
                Err(error.into())
            }
            Err(error) => Err(error.into()),
        }
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
) -> Result<AuthenticatedActorAuthorization, RuntimeAuthError> {
    let credential = auth
        .get_token()
        .map_err(|_| RuntimeAuthError::Unsupported)?
        .into();
    Ok(match auth {
        CodexAuth::ApiKey(_) => AuthenticatedActorAuthorization::PlatformApiKey { credential },
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
