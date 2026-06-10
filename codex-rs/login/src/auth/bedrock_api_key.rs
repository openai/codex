use std::io;

use serde::Deserialize;
use serde::Serialize;

use super::manager::AuthManager;
use super::storage::AuthDotJson;
use codex_app_server_protocol::AuthMode;

/// Managed Amazon Bedrock API key persisted in `auth.json`.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct BedrockApiKeyAuthRecord {
    api_key: String,
}

impl BedrockApiKeyAuthRecord {
    pub fn try_new(api_key: impl Into<String>) -> io::Result<Self> {
        let api_key = api_key.into().trim().to_string();
        if api_key.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Bedrock API key must not be empty",
            ));
        }
        Ok(Self { api_key })
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

/// Runtime authentication state for Amazon Bedrock bearer-token auth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BedrockApiKeyAuth {
    record: BedrockApiKeyAuthRecord,
}

impl BedrockApiKeyAuth {
    pub fn load(record: BedrockApiKeyAuthRecord) -> Self {
        Self { record }
    }

    pub fn api_key(&self) -> &str {
        self.record.api_key()
    }
}

impl AuthManager {
    pub fn bedrock_api_key_cached(&self) -> Option<BedrockApiKeyAuthRecord> {
        load_auth_dot_json(self)
            .ok()
            .flatten()
            .and_then(|auth| auth.bedrock_api_key)
    }

    pub fn has_bedrock_api_key(&self) -> bool {
        self.bedrock_api_key_cached().is_some()
    }

    pub async fn save_bedrock_api_key(&self, record: BedrockApiKeyAuthRecord) -> io::Result<()> {
        let storage = self.auth_storage();
        storage.save(&bedrock_auth_dot_json(record))?;
        self.reload().await;
        Ok(())
    }

    pub async fn clear_bedrock_api_key(&self) -> io::Result<bool> {
        let storage = self.auth_storage();
        let Some(auth) = storage.load()? else {
            return Ok(false);
        };
        if auth.resolved_mode() != AuthMode::BedrockApiKey {
            return Ok(false);
        }
        let removed = storage.delete()?;
        self.reload().await;
        Ok(removed)
    }
}

pub(super) fn bedrock_auth_dot_json(record: BedrockApiKeyAuthRecord) -> AuthDotJson {
    AuthDotJson {
        auth_mode: Some(AuthMode::BedrockApiKey),
        openai_api_key: None,
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: Some(record),
    }
}

fn load_auth_dot_json(manager: &AuthManager) -> io::Result<Option<AuthDotJson>> {
    manager.auth_storage().load()
}

#[cfg(test)]
fn empty_auth_dot_json() -> AuthDotJson {
    AuthDotJson {
        auth_mode: None,
        openai_api_key: None,
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: None,
    }
}

#[cfg(test)]
#[path = "bedrock_api_key_tests.rs"]
mod tests;
