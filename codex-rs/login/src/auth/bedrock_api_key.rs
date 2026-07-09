use std::fmt;
use std::path::Path;

use codex_config::types::AuthCredentialsStoreMode;
use serde::Deserialize;
use serde::Serialize;

use super::manager::AuthManager;
use super::manager::load_auth_dot_json;
use super::manager::logout;
use super::manager::save_auth;
use super::storage::AuthDotJson;
use super::storage::AuthKeyringBackendKind;
use codex_protocol::auth::AuthMode;

/// Managed Amazon Bedrock API key persisted in `auth.json`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct BedrockApiKeyAuth {
    pub api_key: String,
    pub region: String,
}

impl fmt::Debug for BedrockApiKeyAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BedrockApiKeyAuth")
            .field("api_key", &"<redacted>")
            .field("region", &self.region)
            .finish()
    }
}

/// Writes an `auth.json` that contains only the Amazon Bedrock API key auth.
pub fn login_with_bedrock_api_key(
    codex_home: &Path,
    api_key: &str,
    region: &str,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson {
        auth_mode: Some(AuthMode::BedrockApiKey),
        openai_api_key: None,
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: Some(BedrockApiKeyAuth {
            api_key: api_key.to_string(),
            region: region.to_string(),
        }),
    };
    save_auth(
        codex_home,
        &auth_dot_json,
        auth_credentials_store_mode,
        keyring_backend_kind,
    )
}

pub(super) fn load_stored_bedrock_api_key(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<Option<BedrockApiKeyAuth>> {
    load_stored_bedrock_api_key_auth(
        codex_home,
        auth_credentials_store_mode,
        keyring_backend_kind,
    )
    .map(|auth| auth.and_then(|auth| auth.bedrock_api_key))
}

fn load_stored_bedrock_api_key_auth(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<Option<AuthDotJson>> {
    load_auth_dot_json(
        codex_home,
        auth_credentials_store_mode,
        keyring_backend_kind,
    )
    .map(|auth| auth.filter(AuthDotJson::is_bedrock_api_key))
}

impl AuthManager {
    pub fn has_stored_bedrock_api_key_auth(&self) -> std::io::Result<bool> {
        load_stored_bedrock_api_key_auth(
            &self.codex_home,
            self.auth_credentials_store_mode,
            self.keyring_backend_kind,
        )
        .map(|auth| auth.is_some())
    }

    /// Persists managed Bedrock auth without changing the in-memory auth snapshot.
    ///
    /// The account coordinator publishes the provider-scoped cache update only
    /// after the associated config mutation succeeds.
    pub fn persist_bedrock_api_key_auth(&self, api_key: &str, region: &str) -> std::io::Result<()> {
        login_with_bedrock_api_key(
            &self.codex_home,
            api_key,
            region,
            self.auth_credentials_store_mode,
            self.keyring_backend_kind,
        )
    }

    /// Reloads only managed Bedrock auth, preserving cached general Codex auth.
    pub fn reload_bedrock_api_key_auth(&self) -> std::io::Result<bool> {
        let bedrock_api_key = load_stored_bedrock_api_key(
            &self.codex_home,
            self.auth_credentials_store_mode,
            self.keyring_backend_kind,
        )?;
        Ok(self.set_cached_bedrock_api_key_auth(bedrock_api_key))
    }

    /// Removes only managed Bedrock auth, preserving cached general Codex auth.
    pub fn logout_bedrock_api_key_auth(&self) -> std::io::Result<bool> {
        let stored_bedrock_auth = self.has_stored_bedrock_api_key_auth()?;
        let removed = if stored_bedrock_auth {
            logout(
                &self.codex_home,
                self.auth_credentials_store_mode,
                self.keyring_backend_kind,
            )?
        } else {
            false
        };
        self.set_cached_bedrock_api_key_auth(/*new_bedrock_api_key*/ None);
        Ok(removed)
    }
}

#[cfg(test)]
#[path = "bedrock_api_key_tests.rs"]
mod tests;
