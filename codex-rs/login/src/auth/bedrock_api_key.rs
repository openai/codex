use std::io;

use serde::Deserialize;
use serde::Serialize;

use super::manager::AuthManager;
use super::storage::AuthDotJson;
use super::storage::create_auth_storage;

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

impl AuthDotJson {
    pub(super) fn has_primary_auth(&self) -> bool {
        self.auth_mode.is_some()
            || self.openai_api_key.is_some()
            || self.tokens.is_some()
            || self.agent_identity.is_some()
            || self.personal_access_token.is_some()
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

    pub fn save_bedrock_api_key(&self, record: BedrockApiKeyAuthRecord) -> io::Result<()> {
        let storage = create_auth_storage(
            self.codex_home_for_auth_storage(),
            self.auth_credentials_store_mode(),
        );
        let mut auth = storage.load()?.unwrap_or_else(empty_auth_dot_json);
        auth.bedrock_api_key = Some(record);
        storage.save(&auth)
    }

    pub fn clear_bedrock_api_key(&self) -> io::Result<bool> {
        let storage = create_auth_storage(
            self.codex_home_for_auth_storage(),
            self.auth_credentials_store_mode(),
        );
        let Some(mut auth) = storage.load()? else {
            return Ok(false);
        };
        if auth.bedrock_api_key.take().is_none() {
            return Ok(false);
        }
        if !auth.has_primary_auth() {
            storage.delete()?;
        } else {
            storage.save(&auth)?;
        }
        Ok(true)
    }
}

fn load_auth_dot_json(manager: &AuthManager) -> io::Result<Option<AuthDotJson>> {
    create_auth_storage(
        manager.codex_home_for_auth_storage(),
        manager.auth_credentials_store_mode(),
    )
    .load()
}

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
