use std::sync::Arc;

use anyhow::Result;
use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::authorization_header_for_agent_task;
#[cfg(test)]
use codex_agent_identity::generate_agent_key_material;
use codex_agent_identity::normalize_chatgpt_base_url;
use codex_agent_identity::public_key_ssh_from_private_key_pkcs8_base64;
use codex_agent_identity::supports_background_agent_task_auth;
#[cfg(test)]
use codex_protocol::protocol::SessionSource;
use tracing::debug;
use tracing::warn;

use crate::AgentIdentityAuthRecord;
use crate::AuthManager;
use crate::CodexAuth;

#[derive(Clone)]
pub(crate) struct BackgroundAgentTaskManager {
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    auth_mode: BackgroundAgentTaskAuthMode,
}

impl std::fmt::Debug for BackgroundAgentTaskManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundAgentTaskManager")
            .field("chatgpt_base_url", &self.chatgpt_base_url)
            .field("auth_mode", &self.auth_mode)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackgroundAgentTaskAuthMode {
    Enabled,
    #[default]
    Disabled,
}

impl BackgroundAgentTaskAuthMode {
    pub fn from_feature_enabled(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredAgentIdentity {
    binding_id: String,
    chatgpt_account_id: String,
    chatgpt_user_id: Option<String>,
    agent_runtime_id: String,
    private_key_pkcs8_base64: String,
    public_key_ssh: String,
    registered_at: String,
    background_task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentIdentityBinding {
    binding_id: String,
    chatgpt_account_id: String,
    chatgpt_user_id: Option<String>,
}

impl BackgroundAgentTaskManager {
    #[cfg(test)]
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        chatgpt_base_url: String,
        _session_source: SessionSource,
    ) -> Self {
        Self::new_with_auth_mode(
            auth_manager,
            chatgpt_base_url,
            BackgroundAgentTaskAuthMode::Disabled,
        )
    }

    pub(crate) fn new_with_auth_mode(
        auth_manager: Arc<AuthManager>,
        chatgpt_base_url: String,
        auth_mode: BackgroundAgentTaskAuthMode,
    ) -> Self {
        Self {
            auth_manager,
            chatgpt_base_url: normalize_chatgpt_base_url(&chatgpt_base_url),
            auth_mode,
        }
    }

    pub(crate) async fn authorization_header_value_for_auth(
        &self,
        auth: &CodexAuth,
    ) -> Result<Option<String>> {
        if !self.auth_mode.is_enabled() {
            debug!("skipping background agent task auth because agent identity is disabled");
            return Ok(None);
        }

        if !supports_background_agent_task_auth(&self.chatgpt_base_url) {
            debug!(
                chatgpt_base_url = %self.chatgpt_base_url,
                "skipping background agent task auth for unsupported backend host"
            );
            return Ok(None);
        }

        let Some(binding) =
            AgentIdentityBinding::from_auth(auth, self.auth_manager.forced_chatgpt_workspace_id())
        else {
            debug!("skipping background agent task auth because ChatGPT auth is unavailable");
            return Ok(None);
        };

        let Some(stored_identity) = self.load_stored_identity(auth, &binding)? else {
            return Ok(None);
        };
        let Some(background_task_id) = stored_identity.background_task_id.as_ref() else {
            debug!(
                agent_runtime_id = %stored_identity.agent_runtime_id,
                "skipping background agent task auth because stored agent identity has no background task id"
            );
            return Ok(None);
        };

        Ok(Some(authorization_header_for_task(
            &stored_identity,
            background_task_id,
        )?))
    }

    pub(crate) async fn authorization_header_value_or_bearer(
        &self,
        auth: &CodexAuth,
    ) -> Option<String> {
        match self.authorization_header_value_for_auth(auth).await {
            Ok(Some(authorization_header_value)) => Some(authorization_header_value),
            Ok(None) => auth
                .get_token()
                .ok()
                .filter(|token| !token.is_empty())
                .map(|token| format!("Bearer {token}")),
            Err(error) => {
                warn!(
                    error = %error,
                    "falling back to bearer authorization because background agent task auth failed"
                );
                auth.get_token()
                    .ok()
                    .filter(|token| !token.is_empty())
                    .map(|token| format!("Bearer {token}"))
            }
        }
    }

    fn load_stored_identity(
        &self,
        auth: &CodexAuth,
        binding: &AgentIdentityBinding,
    ) -> Result<Option<StoredAgentIdentity>> {
        let Some(record) = auth.get_agent_identity(&binding.chatgpt_account_id) else {
            return Ok(None);
        };

        let stored_identity = match StoredAgentIdentity::from_auth_record(binding, record) {
            Ok(stored_identity) => stored_identity,
            Err(error) => {
                warn!(
                    binding_id = %binding.binding_id,
                    error = %error,
                    "stored agent identity is invalid; deleting cached value"
                );
                auth.remove_agent_identity()?;
                return Ok(None);
            }
        };

        if !stored_identity.matches_binding(binding) {
            warn!(
                binding_id = %binding.binding_id,
                "stored agent identity binding no longer matches current auth; deleting cached value"
            );
            auth.remove_agent_identity()?;
            return Ok(None);
        }

        if let Err(error) = stored_identity.validate_key_material() {
            warn!(
                agent_runtime_id = %stored_identity.agent_runtime_id,
                binding_id = %binding.binding_id,
                error = %error,
                "stored agent identity key material is invalid; deleting cached value"
            );
            auth.remove_agent_identity()?;
            return Ok(None);
        }

        Ok(Some(stored_identity))
    }
}

pub fn cached_background_agent_task_authorization_header_value(
    auth: &CodexAuth,
    auth_mode: BackgroundAgentTaskAuthMode,
) -> Result<Option<String>> {
    if !auth_mode.is_enabled() {
        return Ok(None);
    }

    let Some(binding) = AgentIdentityBinding::from_auth(auth, /*forced_workspace_id*/ None) else {
        return Ok(None);
    };
    let Some(record) = auth.get_agent_identity(&binding.chatgpt_account_id) else {
        return Ok(None);
    };
    let stored_identity = StoredAgentIdentity::from_auth_record(&binding, record)?;
    if !stored_identity.matches_binding(&binding) {
        return Ok(None);
    }
    stored_identity.validate_key_material()?;
    let Some(background_task_id) = stored_identity.background_task_id.as_ref() else {
        return Ok(None);
    };
    authorization_header_for_task(&stored_identity, background_task_id).map(Some)
}

impl StoredAgentIdentity {
    fn from_auth_record(
        binding: &AgentIdentityBinding,
        record: AgentIdentityAuthRecord,
    ) -> Result<Self> {
        if record.workspace_id != binding.chatgpt_account_id {
            anyhow::bail!(
                "stored agent identity workspace {:?} does not match current workspace {:?}",
                record.workspace_id,
                binding.chatgpt_account_id
            );
        }
        let public_key_ssh =
            public_key_ssh_from_private_key_pkcs8_base64(&record.agent_private_key)?;
        Ok(Self {
            binding_id: binding.binding_id.clone(),
            chatgpt_account_id: binding.chatgpt_account_id.clone(),
            chatgpt_user_id: record.chatgpt_user_id,
            agent_runtime_id: record.agent_runtime_id.clone(),
            private_key_pkcs8_base64: record.agent_private_key,
            public_key_ssh,
            registered_at: record.registered_at,
            background_task_id: record.background_task_id,
        })
    }

    fn matches_binding(&self, binding: &AgentIdentityBinding) -> bool {
        binding.matches_parts(
            &self.binding_id,
            &self.chatgpt_account_id,
            self.chatgpt_user_id.as_deref(),
        )
    }

    fn validate_key_material(&self) -> Result<()> {
        let derived_public_key =
            public_key_ssh_from_private_key_pkcs8_base64(&self.private_key_pkcs8_base64)?;
        anyhow::ensure!(
            self.public_key_ssh == derived_public_key,
            "stored public key does not match the private key"
        );
        Ok(())
    }

    fn agent_identity_key(&self) -> AgentIdentityKey<'_> {
        AgentIdentityKey {
            agent_runtime_id: &self.agent_runtime_id,
            private_key_pkcs8_base64: &self.private_key_pkcs8_base64,
        }
    }
}

impl AgentIdentityBinding {
    fn matches_parts(
        &self,
        binding_id: &str,
        chatgpt_account_id: &str,
        chatgpt_user_id: Option<&str>,
    ) -> bool {
        binding_id == self.binding_id
            && chatgpt_account_id == self.chatgpt_account_id
            && match self.chatgpt_user_id.as_deref() {
                Some(expected_user_id) => chatgpt_user_id == Some(expected_user_id),
                None => true,
            }
    }

    fn from_auth(auth: &CodexAuth, forced_workspace_id: Option<String>) -> Option<Self> {
        if !auth.is_chatgpt_auth() {
            return None;
        }

        let token_data = auth.get_token_data().ok()?;
        let resolved_account_id =
            forced_workspace_id
                .filter(|value| !value.is_empty())
                .or(token_data
                    .account_id
                    .clone()
                    .filter(|value| !value.is_empty()))?;

        Some(Self {
            binding_id: format!("chatgpt-account-{resolved_account_id}"),
            chatgpt_account_id: resolved_account_id,
            chatgpt_user_id: token_data
                .id_token
                .chatgpt_user_id
                .filter(|value| !value.is_empty()),
        })
    }
}

fn authorization_header_for_task(
    stored_identity: &StoredAgentIdentity,
    background_task_id: &str,
) -> Result<String> {
    authorization_header_for_agent_task(
        stored_identity.agent_identity_key(),
        codex_agent_identity::AgentTaskAuthorizationTarget {
            agent_runtime_id: &stored_identity.agent_runtime_id,
            task_id: background_task_id,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_background_agent_task_auth_returns_none_for_supported_host() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
        let manager = BackgroundAgentTaskManager::new_with_auth_mode(
            auth_manager,
            "https://chatgpt.com/backend-api".to_string(),
            BackgroundAgentTaskAuthMode::Disabled,
        );

        let authorization_header_value = manager
            .authorization_header_value_for_auth(&auth)
            .await
            .expect("disabled manager should not fail");

        assert_eq!(None, authorization_header_value);
    }

    #[tokio::test]
    async fn default_background_agent_task_auth_returns_none_for_supported_host() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
        let manager = BackgroundAgentTaskManager::new(
            auth_manager,
            "https://chatgpt.com/backend-api".to_string(),
            SessionSource::Cli,
        );

        let authorization_header_value = manager
            .authorization_header_value_for_auth(&auth)
            .await
            .expect("default manager should not fail");

        assert_eq!(None, authorization_header_value);
    }

    #[test]
    fn cached_background_agent_task_auth_honors_disabled_mode() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let key_material = generate_agent_key_material().expect("generate key material");
        auth.set_agent_identity(AgentIdentityAuthRecord {
            workspace_id: "account_id".to_string(),
            chatgpt_user_id: None,
            agent_runtime_id: "agent_123".to_string(),
            agent_private_key: key_material.private_key_pkcs8_base64,
            registered_at: "2026-04-13T12:00:00Z".to_string(),
            background_task_id: Some("task_123".to_string()),
        })
        .expect("set agent identity");

        let disabled_authorization_header_value =
            cached_background_agent_task_authorization_header_value(
                &auth,
                BackgroundAgentTaskAuthMode::Disabled,
            )
            .expect("disabled cached auth should not fail");
        let enabled_authorization_header_value =
            cached_background_agent_task_authorization_header_value(
                &auth,
                BackgroundAgentTaskAuthMode::Enabled,
            )
            .expect("enabled cached auth should not fail");

        assert_eq!(None, disabled_authorization_header_value);
        assert!(enabled_authorization_header_value.is_some());
    }
}
