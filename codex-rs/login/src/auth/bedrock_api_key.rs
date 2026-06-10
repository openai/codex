use std::io;

use serde::Deserialize;
use serde::Serialize;

use super::manager::AuthManager;
use super::storage::AuthDotJson;
use codex_app_server_protocol::AuthMode;

/// Managed Amazon Bedrock API key persisted in `auth.json`.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct BedrockApiKeyAuth {
    api_key: String,
}

impl BedrockApiKeyAuth {
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

impl AuthManager {
    pub async fn save_bedrock_api_key(&self, auth: BedrockApiKeyAuth) -> io::Result<()> {
        let storage = self.auth_storage();
        storage.save(&bedrock_auth_dot_json(auth))?;
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

pub(super) fn bedrock_auth_dot_json(auth: BedrockApiKeyAuth) -> AuthDotJson {
    AuthDotJson {
        auth_mode: Some(AuthMode::BedrockApiKey),
        openai_api_key: None,
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: Some(auth),
    }
}

#[cfg(test)]
#[path = "bedrock_api_key_tests.rs"]
mod tests;
