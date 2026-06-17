use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::auth::AuthMode;
use codex_protocol::config_types::ForcedLoginMethod;
use pretty_assertions::assert_eq;
use serial_test::serial;
use tempfile::tempdir;

use super::*;
use crate::auth::AuthConfig;
use crate::auth::AuthKeyringBackendKind;
use crate::auth::AuthManager;
use crate::auth::enforce_login_restrictions;
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

fn auth_config(codex_home: &std::path::Path, forced_login_method: ForcedLoginMethod) -> AuthConfig {
    AuthConfig {
        codex_home: codex_home.to_path_buf(),
        auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        keyring_backend_kind: AuthKeyringBackendKind::default(),
        forced_login_method: Some(forced_login_method),
        chatgpt_base_url: None,
        forced_chatgpt_workspace_id: None,
        auth_route_config: None,
    }
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn auth_manager_login_with_bedrock_api_key_replaces_openai_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    storage.save(&api_key_auth())?;
    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        AuthKeyringBackendKind::default(),
        /*auth_route_config*/ None,
    )
    .await;

    auth_manager
        .login_with_bedrock_api_key("bedrock-api-key-test", "us-east-1")
        .await?;

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
    assert_eq!(auth_manager.auth_mode(), None);
    assert_eq!(
        auth_manager.account_auth_mode_cached(),
        Some(AuthMode::BedrockApiKey)
    );
    assert_eq!(
        auth_manager.bedrock_api_key_auth_cached(),
        Some(bedrock_auth())
    );
    assert_eq!(auth_manager.auth_cached(), None);
    Ok(())
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn auth_manager_logout_removes_bedrock_api_key_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    login_with_bedrock_api_key(
        codex_home.path(),
        "bedrock-api-key-test",
        "us-east-1",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        AuthKeyringBackendKind::default(),
        /*auth_route_config*/ None,
    )
    .await;

    assert!(auth_manager.logout().await?);

    assert_eq!(storage.load()?, None);
    assert_eq!(auth_manager.auth_cached(), None);
    assert_eq!(auth_manager.bedrock_api_key_auth_cached(), None);
    assert_eq!(auth_manager.account_auth_mode_cached(), None);
    Ok(())
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn bedrock_only_auth_storage_does_not_create_codex_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    storage.save(&bedrock_only_auth())?;

    let auth_manager = AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        AuthKeyringBackendKind::default(),
        /*auth_route_config*/ None,
    )
    .await;

    assert_eq!(auth_manager.auth_mode(), None);
    assert_eq!(
        auth_manager.account_auth_mode_cached(),
        Some(AuthMode::BedrockApiKey)
    );
    assert_eq!(
        auth_manager.bedrock_api_key_auth_cached(),
        Some(bedrock_auth())
    );
    assert_eq!(auth_manager.auth_cached(), None);
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
        AuthKeyringBackendKind::default(),
    )?;

    crate::auth::login_with_api_key(
        codex_home.path(),
        "sk-test-key",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    assert_eq!(storage.load()?, Some(api_key_auth()));
    Ok(())
}

#[tokio::test]
async fn forced_chatgpt_login_removes_bedrock_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    storage.save(&bedrock_only_auth())?;

    let error =
        enforce_login_restrictions(&auth_config(codex_home.path(), ForcedLoginMethod::Chatgpt))
            .await
            .expect_err("Bedrock auth should violate forced ChatGPT login");

    assert_eq!(
        error.to_string(),
        "ChatGPT login is required, but an API key is currently being used. Logging out."
    );
    assert_eq!(storage.load()?, None);
    Ok(())
}

#[tokio::test]
async fn forced_api_login_allows_bedrock_auth() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
    let auth = bedrock_only_auth();
    storage.save(&auth)?;

    enforce_login_restrictions(&auth_config(codex_home.path(), ForcedLoginMethod::Api)).await?;

    assert_eq!(storage.load()?, Some(auth));
    Ok(())
}
