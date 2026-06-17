use super::*;
use codex_config::CloudConfigBundleLoadErrorCode;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn storage_loader_consults_workload_identity_without_stored_auth() {
    let codex_home = tempdir().expect("tempdir");
    let missing_token_path = codex_home.path().join("missing-subject-token");
    let workload_identity = serde_json::from_value(json!({
        "identity_provider_id": "idp_test",
        "identity_provider_mapping_id": "idpm_test",
        "audience": "api://codex-test",
        "token_url": "http://127.0.0.1:1/oauth/token",
        "credential_source": {
            "type": "file",
            "path": missing_token_path,
        },
    }))
    .expect("valid workload identity config");

    let loader = cloud_config_bundle_loader_for_storage(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        "http://127.0.0.1:1/backend-api".to_string(),
        Some(workload_identity),
        /*forced_chatgpt_workspace_id*/ None,
    )
    .await;

    let error = loader
        .get()
        .await
        .expect_err("missing WIF source should fail managed config auth");
    assert_eq!(error.code(), CloudConfigBundleLoadErrorCode::Auth);
}

#[test]
fn storage_auth_config_propagates_forced_workspace_ids() {
    let expected = vec!["workspace_allowed".to_string()];
    let config = StorageAuthManagerConfig {
        codex_home: PathBuf::new(),
        credentials_store_mode: AuthCredentialsStoreMode::File,
        keyring_backend_kind: AuthKeyringBackendKind::default(),
        chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
        workload_identity: None,
        forced_chatgpt_workspace_id: Some(expected.clone()),
    };

    assert_eq!(config.forced_chatgpt_workspace_id(), Some(expected));
}
