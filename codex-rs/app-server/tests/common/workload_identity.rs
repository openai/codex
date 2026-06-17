use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use crate::ChatGptIdTokenClaims;
use crate::encode_id_token;

pub struct ExpiringWorkloadIdentityFixture {
    token_path: PathBuf,
}

impl ExpiringWorkloadIdentityFixture {
    pub async fn remove_and_wait_for_expiry(self) -> Result<()> {
        tokio::fs::remove_file(self.token_path).await?;
        tokio::time::sleep(Duration::from_secs(3)).await;
        Ok(())
    }
}

pub async fn configure_expiring_workload_identity(
    codex_home: &Path,
    server: &MockServer,
) -> Result<ExpiringWorkloadIdentityFixture> {
    configure_expiring_workload_identity_inner(codex_home, server, /*mock_cloud_config*/ true).await
}

pub async fn configure_expiring_workload_identity_without_cloud_config_mock(
    codex_home: &Path,
    server: &MockServer,
) -> Result<ExpiringWorkloadIdentityFixture> {
    configure_expiring_workload_identity_inner(codex_home, server, /*mock_cloud_config*/ false)
        .await
}

async fn configure_expiring_workload_identity_inner(
    codex_home: &Path,
    server: &MockServer,
    mock_cloud_config: bool,
) -> Result<ExpiringWorkloadIdentityFixture> {
    let token_path = codex_home.join("projected-workload-token");
    tokio::fs::write(&token_path, "external-subject-token\n").await?;
    let token_path_toml = serde_json::to_string(token_path.to_string_lossy().as_ref())?;
    let config_path = codex_home.join("config.toml");
    let mut config = std::fs::read_to_string(&config_path)?;
    config.push_str(&format!(
        r#"

[workload_identity]
identity_provider_id = "idp_test"
identity_provider_mapping_id = "idpm_test"
audience = "api://codex-test"
token_url = "{}/oauth/token"

[workload_identity.credential_source]
type = "file"
path = {token_path_toml}
"#,
        server.uri(),
    ));
    std::fs::write(config_path, config)?;

    let access_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("workload@example.com")
            .plan_type("enterprise")
            .chatgpt_user_id("user_test")
            .chatgpt_account_id("workspace_test"),
    )?;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": access_token.clone(),
            "issued_token_type": "urn:ietf:params:oauth:token-type:access_token",
            "token_type": "Bearer",
            "expires_in": 2,
            "user_id": "user_test",
            "chatgpt_account_id": "workspace_test",
            "chatgpt_plan_type": "enterprise",
        })))
        .expect(1..)
        .mount(server)
        .await;
    if mock_cloud_config {
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/config/bundle"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/backend-api/accounts/workspace_test/settings"))
        .and(header("authorization", format!("Bearer {access_token}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "beta_settings": { "enable_plugins": true },
        })))
        .mount(server)
        .await;

    Ok(ExpiringWorkloadIdentityFixture { token_path })
}
