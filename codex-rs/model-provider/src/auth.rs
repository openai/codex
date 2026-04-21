use std::sync::Arc;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::AgentTaskAuthorizationTarget;
use codex_agent_identity::authorization_header_for_agent_task;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use http::HeaderMap;
use http::HeaderValue;

use crate::bearer_auth_provider::BearerAuthProvider;

#[derive(Clone, Debug)]
struct CodexAuthProvider {
    auth: CodexAuth,
}

impl AuthProvider for CodexAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let header_value = match &self.auth {
            CodexAuth::AgentIdentity(auth) => {
                let record = auth.record();
                let process_task_id = auth.process_task_id().ok_or_else(|| {
                    std::io::Error::other("agent identity process task is not initialized")
                });
                process_task_id.and_then(|task_id| {
                    authorization_header_for_agent_task(
                        AgentIdentityKey {
                            agent_runtime_id: &record.agent_runtime_id,
                            private_key_pkcs8_base64: &record.agent_private_key,
                        },
                        AgentTaskAuthorizationTarget {
                            agent_runtime_id: &record.agent_runtime_id,
                            task_id,
                        },
                    )
                    .map_err(std::io::Error::other)
                })
            }
            CodexAuth::ApiKey(_) => self.auth.api_key().map_or_else(
                || Err(std::io::Error::other("API key auth missing API key")),
                |api_key| Ok(format!("Bearer {api_key}")),
            ),
            CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_) => {
                self.auth.get_token().map(|token| format!("Bearer {token}"))
            }
        };

        if let Ok(header_value) = header_value
            && let Ok(header) = HeaderValue::from_str(&header_value)
        {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }

        if let Some(account_id) = self.auth.get_account_id()
            && let Ok(header) = HeaderValue::from_str(&account_id)
        {
            let _ = headers.insert("ChatGPT-Account-ID", header);
        }

        if self.auth.is_fedramp_account() {
            let _ = headers.insert("X-OpenAI-Fedramp", HeaderValue::from_static("true"));
        }
    }
}

/// Returns the provider-scoped auth manager when this provider uses command-backed auth.
///
/// Providers without custom auth continue using the caller-supplied base manager, when present.
pub(crate) fn auth_manager_for_provider(
    auth_manager: Option<Arc<AuthManager>>,
    provider: &ModelProviderInfo,
) -> Option<Arc<AuthManager>> {
    match provider.auth.clone() {
        Some(config) => Some(AuthManager::external_bearer_only(config)),
        None => auth_manager,
    }
}

pub(crate) fn resolve_provider_auth(
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<SharedAuthProvider> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(Arc::new(BearerAuthProvider {
            token: Some(api_key),
            account_id: None,
            is_fedramp_account: false,
        }));
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(Arc::new(BearerAuthProvider {
            token: Some(token),
            account_id: None,
            is_fedramp_account: false,
        }));
    }

    let Some(auth) = auth else {
        return Ok(Arc::new(BearerAuthProvider {
            token: None,
            account_id: None,
            is_fedramp_account: false,
        }));
    };

    Ok(auth_provider_from_auth(auth))
}

/// Builds request-header auth for a first-party Codex auth snapshot.
pub fn auth_provider_from_auth(auth: &CodexAuth) -> SharedAuthProvider {
    Arc::new(CodexAuthProvider { auth: auth.clone() })
}
