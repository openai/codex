use codex_app_server_protocol::AuthMode;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use super::*;
use crate::auth::storage::AuthStorageBackend;
use crate::auth::storage::FileAuthStorage;
use crate::auth::storage::get_auth_file;

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
    let mut auth = empty_auth_dot_json();
    auth.bedrock_api_key = Some(bedrock_record());
    auth
}

fn bedrock_record() -> BedrockApiKeyAuthRecord {
    BedrockApiKeyAuthRecord::try_new(" bedrock-api-key-test ")
        .expect("record should normalize non-empty key")
}

#[tokio::test]
async fn save_bedrock_api_key_preserves_openai_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    storage.save(&api_key_auth())?;
    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
    )
    .await;

    auth_manager.save_bedrock_api_key(bedrock_record())?;

    let loaded = storage.load()?.expect("auth should be stored");
    assert_eq!(
        loaded,
        AuthDotJson {
            bedrock_api_key: Some(BedrockApiKeyAuthRecord::try_new("bedrock-api-key-test")?),
            ..api_key_auth()
        }
    );
    assert_eq!(
        auth_manager
            .bedrock_api_key_cached()
            .as_ref()
            .map(BedrockApiKeyAuthRecord::api_key),
        Some("bedrock-api-key-test")
    );
    assert!(auth_manager.has_bedrock_api_key());
    Ok(())
}

#[tokio::test]
async fn clear_bedrock_api_key_preserves_openai_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    storage.save(&AuthDotJson {
        bedrock_api_key: Some(bedrock_record()),
        ..api_key_auth()
    })?;
    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
    )
    .await;

    assert!(auth_manager.clear_bedrock_api_key()?);

    assert_eq!(storage.load()?, Some(api_key_auth()));
    assert!(!auth_manager.has_bedrock_api_key());
    Ok(())
}

#[tokio::test]
async fn clear_bedrock_api_key_without_entry_is_noop() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    storage.save(&api_key_auth())?;
    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
    )
    .await;

    assert!(!auth_manager.clear_bedrock_api_key()?);

    assert_eq!(storage.load()?, Some(api_key_auth()));
    Ok(())
}

#[tokio::test]
async fn bedrock_only_auth_storage_does_not_create_primary_auth() -> anyhow::Result<()> {
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

    assert_eq!(auth_manager.auth_cached(), None);
    assert!(auth_manager.has_bedrock_api_key());
    Ok(())
}

#[tokio::test]
async fn clear_bedrock_only_auth_storage_removes_auth_file() -> anyhow::Result<()> {
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

    assert!(auth_manager.clear_bedrock_api_key()?);

    assert!(!get_auth_file(codex_home.path()).exists());
    assert_eq!(storage.load()?, None);
    Ok(())
}
