use std::path::PathBuf;

use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

use crate::CredentialSourceConfig;
use crate::FileSubjectTokenSource;

use super::*;

async fn test_client(
    server: &MockServer,
) -> anyhow::Result<WorkloadIdentityClient<FileSubjectTokenSource>> {
    let temp_dir = tempfile::tempdir()?;
    let token_file = temp_dir.keep().join("subject-token");
    tokio::fs::write(&token_file, "azure.subject.token\n").await?;
    Ok(WorkloadIdentityClient::new(
        WorkloadIdentityConfig {
            identity_provider_id: "idp_example".to_string(),
            identity_provider_mapping_id: "idpm_example".to_string(),
            audience: "https://auth.openai.com/workload-identity".to_string(),
            token_url: format!("{}/oauth/token", server.uri()),
            credential_source: CredentialSourceConfig::File {
                path: PathBuf::from(&token_file),
            },
        },
        "app_codex_cli",
        reqwest::Client::new(),
        FileSubjectTokenSource::new(token_file),
    ))
}

fn exchange_request() -> serde_json::Value {
    json!({
        "grant_type": TOKEN_EXCHANGE_GRANT_TYPE,
        "requested_token_type": ACCESS_TOKEN_TYPE,
        "subject_token": "azure.subject.token",
        "subject_token_type": crate::source::JWT_SUBJECT_TOKEN_TYPE,
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
        "user_id": "user_example",
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
        user_id: "user_example".to_string(),
        chatgpt_account_id: "workspace_example".to_string(),
        chatgpt_plan_type: Some("enterprise".to_string()),
    };
    assert_eq!(client.resolve().await?, expected);
    assert_eq!(client.resolve().await?, expected);
    Ok(())
}

#[tokio::test]
async fn concurrent_resolves_share_one_successful_exchange() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(exchange_response("shared.access.token"))
        .expect(1)
        .mount(&server)
        .await;
    let client = test_client(&server).await?;

    let (first, second, third) = tokio::join!(client.resolve(), client.resolve(), client.resolve());

    assert_eq!(first?, second?);
    assert_eq!(client.resolve().await?, third?);
    Ok(())
}

#[tokio::test]
async fn concurrent_resolves_share_one_rejected_exchange() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error": "invalid_grant"})))
        .expect(1)
        .mount(&server)
        .await;
    let client = test_client(&server).await?;

    let (first, second, third) = tokio::join!(client.resolve(), client.resolve(), client.resolve());
    let expected =
        "workload identity token exchange was rejected with HTTP 401 Unauthorized: invalid_grant";

    assert_eq!(
        first.expect_err("first exchange should fail").to_string(),
        expected
    );
    assert_eq!(
        second.expect_err("second exchange should fail").to_string(),
        expected
    );
    assert_eq!(
        third.expect_err("third exchange should fail").to_string(),
        expected
    );
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

#[tokio::test]
async fn refresh_rejects_a_changed_principal() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(exchange_response("first.access.token"))
        .expect(1)
        .mount(&server)
        .await;
    let client = test_client(&server).await?;
    client.resolve().await?;

    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "second.access.token",
            "issued_token_type": ACCESS_TOKEN_TYPE,
            "token_type": "Bearer",
            "expires_in": 3600,
            "user_id": "different_user",
            "chatgpt_account_id": "workspace_example",
        })))
        .expect(1)
        .mount(&server)
        .await;

    assert!(matches!(
        client.refresh().await,
        Err(WorkloadIdentityError::PrincipalChanged)
    ));
    assert!(client.cache.lock().expect("cache lock").token.is_none());
    Ok(())
}

#[tokio::test]
async fn reports_safe_nested_error_code() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {
                "code": "identity_provider_mapping_not_found",
                "message": "untrusted diagnostic text",
            },
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = test_client(&server).await?;

    assert_eq!(
        client
            .resolve()
            .await
            .expect_err("exchange should fail")
            .to_string(),
        "workload identity token exchange was rejected with HTTP 400 Bad Request: identity_provider_mapping_not_found"
    );
    Ok(())
}

#[tokio::test]
async fn rejection_does_not_echo_unstructured_server_text() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": "do not log secret subject token",
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = test_client(&server).await?;

    assert_eq!(
        client
            .resolve()
            .await
            .expect_err("exchange should fail")
            .to_string(),
        "workload identity token exchange was rejected with HTTP 401 Unauthorized: token endpoint rejected the request"
    );
    Ok(())
}

#[tokio::test]
async fn rejects_oversized_token_endpoint_response() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(vec![b'x'; MAX_TOKEN_RESPONSE_BYTES + 1]),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = test_client(&server).await?;

    assert!(matches!(
        client.resolve().await,
        Err(WorkloadIdentityError::ResponseTooLarge)
    ));
    Ok(())
}
