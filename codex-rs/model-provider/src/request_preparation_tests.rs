use super::prepare_provider_request;
use crate::create_model_provider;
use crate::provider_uses_first_party_auth_path;
use base64::Engine;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn explicit_provider_token_survives_a_workspace_switch_during_setup() {
    let home = TempDir::new().unwrap();
    let claims = json!({"https://api.openai.com/auth": {"chatgpt_user_id": "user-a"}});
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    let token = format!("header.{payload}.signature");
    let initial = CodexAuth::from_external_chatgpt_tokens(
        &token,
        "workspace-a",
        /*chatgpt_plan_type*/ None,
    )
    .unwrap();
    let manager = AuthManager::from_auth_for_testing_with_home(initial, home.path().to_path_buf());
    let mut info = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
    info.experimental_bearer_token = Some("provider-token".into());
    let provider = create_model_provider(info, Some(manager.clone()));

    let mut attempts = 0;
    let auth = prepare_provider_request(
        provider.as_ref(),
        provider_uses_first_party_auth_path(provider.info()),
        |_| {
            attempts += 1;
            async {
                let (_, auth) = provider.api_provider_and_auth().await?;
                codex_login::auth::login_with_chatgpt_auth_tokens(
                    home.path(),
                    &token,
                    "workspace-b",
                    /*chatgpt_plan_type*/ None,
                )?;
                manager.reload().await;
                Ok(auth)
            }
        },
    )
    .await
    .unwrap();

    assert_eq!(
        manager.auth_cached().unwrap().get_account_id().as_deref(),
        Some("workspace-b")
    );
    assert_eq!(attempts, 1);
    let mut headers = http::HeaderMap::new();
    auth.add_auth_headers(&mut headers).unwrap();
    assert_eq!(
        headers[http::header::AUTHORIZATION],
        "Bearer provider-token"
    );
}
