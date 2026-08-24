use std::path::Path;

use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::auth::AuthMode;
use serde::Deserialize;
use serde::Serialize;

use super::manager::save_auth;
use super::storage::AuthDotJson;
use super::storage::AuthKeyringBackendKind;

/// Managed Amazon Bedrock AWS access keys persisted in auth storage.
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct BedrockAccessKeysAuth {
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
}

impl std::fmt::Debug for BedrockAccessKeysAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BedrockAccessKeysAuth")
            .field("access_key_id", &"<redacted>")
            .field("secret_access_key", &"<redacted>")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// Writes auth storage containing only Amazon Bedrock AWS access keys.
pub fn login_with_bedrock_access_keys(
    codex_home: &Path,
    access_key_id: &str,
    secret_access_key: &str,
    session_token: Option<&str>,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson {
        auth_mode: Some(AuthMode::BedrockAccessKeys),
        openai_api_key: None,
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: None,
        bedrock_access_keys: Some(BedrockAccessKeysAuth {
            access_key_id: access_key_id.to_string(),
            secret_access_key: secret_access_key.to_string(),
            session_token: session_token.map(str::to_string),
        }),
    };
    save_auth(
        codex_home,
        &auth_dot_json,
        auth_credentials_store_mode,
        keyring_backend_kind,
    )
}
