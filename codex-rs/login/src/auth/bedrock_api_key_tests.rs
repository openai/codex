use codex_app_server_protocol::AuthMode;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use super::*;
use crate::auth::AuthManager;
use crate::auth::CodexAuth;
use crate::auth::storage::AuthStorageBackend;
use crate::auth::storage::FileAuthStorage;

fn api_key_auth() -> AuthDotJson {
    AuthDotJson {
        auth_mode: Some(AuthMode::ApiKey),
        openai_api_key: Some("sk-test-key".to_string()),
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: None,
    }
}

fn bedrock_only_auth() -> AuthDotJson {
    AuthDotJson {
        auth_mode: None,
        openai_api_key: None,
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: Some(bedrock_auth()),
    }
}

fn bedrock_auth() -> BedrockApiKeyAuth {
    BedrockApiKeyAuth {
        api_key: "bedrock-api-key-test".to_string(),
        region: "us-east-1".to_string(),
    }
}

#[tokio::test]
async fn login_with_bedrock_api_key_replaces_openai_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    storage.save(&api_key_auth())?;
    login_with_bedrock_api_key(
        codex_home.path(),
        "bedrock-api-key-test",
        "us-east-1",
        AuthCredentialsStoreMode::File,
    )?;

    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
    )
    .await;

    let loaded = storage.load()?.expect("auth should be stored");
    let expected = AuthDotJson {
        auth_mode: Some(AuthMode::BedrockApiKey),
        openai_api_key: None,
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: Some(bedrock_auth()),
    };
    assert_eq!(loaded, expected);
    assert_eq!(auth_manager.auth_mode(), Some(AuthMode::BedrockApiKey));
    assert_eq!(
        auth_manager.auth_cached().and_then(|auth| match auth {
            CodexAuth::BedrockApiKey(auth) => Some(auth),
            CodexAuth::ApiKey(_)
            | CodexAuth::Chatgpt(_)
            | CodexAuth::ChatgptAuthTokens(_)
            | CodexAuth::AgentIdentity(_)
            | CodexAuth::PersonalAccessToken(_) => None,
        }),
        Some(bedrock_auth())
    );
    Ok(())
}

#[tokio::test]
async fn logout_removes_bedrock_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    login_with_bedrock_api_key(
        codex_home.path(),
        "bedrock-api-key-test",
        "us-east-1",
        AuthCredentialsStoreMode::File,
    )?;
    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
    )
    .await;

    assert!(auth_manager.logout().await?);

    assert_eq!(storage.load()?, None);
    assert_eq!(auth_manager.auth_cached(), None);
    Ok(())
}

#[tokio::test]
async fn bedrock_only_auth_storage_creates_primary_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    storage.save(&bedrock_only_auth())?;

    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
    )
    .await;

    assert_eq!(auth_manager.auth_mode(), Some(AuthMode::BedrockApiKey));
    assert_eq!(
        auth_manager.auth_cached().and_then(|auth| match auth {
            CodexAuth::BedrockApiKey(auth) => Some(auth),
            CodexAuth::ApiKey(_)
            | CodexAuth::Chatgpt(_)
            | CodexAuth::ChatgptAuthTokens(_)
            | CodexAuth::AgentIdentity(_)
            | CodexAuth::PersonalAccessToken(_) => None,
        }),
        Some(bedrock_auth())
    );
    Ok(())
}

#[tokio::test]
async fn login_with_api_key_clears_bedrock_api_key() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    login_with_bedrock_api_key(
        codex_home.path(),
        "bedrock-api-key-test",
        "us-east-1",
        AuthCredentialsStoreMode::File,
    )?;

    crate::auth::login_with_api_key(
        codex_home.path(),
        "sk-test-key",
        AuthCredentialsStoreMode::File,
    )?;

    assert_eq!(storage.load()?, Some(api_key_auth()));
    Ok(())
}
