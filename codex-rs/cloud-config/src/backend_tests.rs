use super::*;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthRouteConfig;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn cloud_bundle_client_preserves_auto_proxy_config() {
    let codex_home = tempfile::tempdir().expect("create temp dir");
    let auth_manager = AuthManager::new_with_auth_route_config(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        Some("https://chatgpt.com/backend-api/".to_string()),
        Some(AuthRouteConfig::auto()),
    )
    .await;

    let client = BackendBundleClient::from_auth_manager(
        "https://chatgpt.com/backend-api/".to_string(),
        &auth_manager,
    );

    assert_eq!(client.auth_route_config, Some(AuthRouteConfig::auto()));
}
