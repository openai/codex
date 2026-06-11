use std::path::PathBuf;

use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

async fn test_client(server: &MockServer) -> anyhow::Result<WorkloadIdentityClient> {
    let temp_dir = tempfile::tempdir()?;
    let token_file = temp_dir.keep().join("subject-token");
    tokio::fs::write(&token_file, "azure.subject.token\n").await?;
    Ok(WorkloadIdentityClient::new(
        WorkloadIdentityConfig {
            identity_provider_id: "idp_example".to_string(),
            identity_provider_mapping_id: "idpm_example".to_string(),
            token_url: format!("{}/oauth/token", server.uri()),
            credential_source: CredentialSourceConfig::Azure {
                token_file: Some(PathBuf::from(token_file)),
            },
        },
        "app_codex_cli",
        reqwest::Client::new(),
    ))
}

fn exchange_request() -> serde_json::Value {
    json!({
        "grant_type": TOKEN_EXCHANGE_GRANT_TYPE,
        "requested_token_type": ACCESS_TOKEN_TYPE,
        "subject_token": "azure.subject.token",
        "subject_token_type": JWT_TOKEN_TYPE,
        "identity_provider_id": "idp_example",
        "identity_provider_mapping_id": "idpm_example",
        "client_id": "app_codex_cli",
    })
}

fn exchange_response(access_token: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "access_token": access_token,
        "issued_token_type": ACCESS_TOKEN_TYPE,
        "token_type": "Bearer",
        "expires_in": 3600,
        "chatgpt_account_id": "workspace_example",
        "chatgpt_plan_type": "enterprise",
    }))
}

#[tokio::test]
async fn exchanges_azure_subject_token_and_caches_access_token() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_json(exchange_request()))
        .respond_with(exchange_response("first.access.token"))
        .expect(1)
        .mount(&server)
        .await;
    let client = test_client(&server).await?;

    let expected = WorkloadIdentityAccessToken {
        access_token: "first.access.token".to_string(),
        chatgpt_account_id: "workspace_example".to_string(),
        chatgpt_plan_type: Some("enterprise".to_string()),
    };
    assert_eq!(client.resolve().await?, expected);
    assert_eq!(client.resolve().await?, expected);
    Ok(())
}

#[tokio::test]
async fn forced_refresh_performs_a_new_exchange() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(exchange_response("access.token"))
        .expect(2)
        .mount(&server)
        .await;
    let client = test_client(&server).await?;

    client.resolve().await?;
    client.refresh().await?;
    Ok(())
}
