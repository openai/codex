use super::*;
use crate::default_client::create_client;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn response(email: Option<&str>) -> serde_json::Value {
    json!({
        "email": email,
        "chatgpt_user_id": "user-123",
        "chatgpt_account_id": "account-123",
        "chatgpt_plan_type": "enterprise",
        "chatgpt_account_is_fedramp": true,
    })
}

#[tokio::test]
async fn hydrate_sends_bearer_token_and_preserves_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(WHOAMI_PATH))
        .and(header("authorization", "Bearer at-example"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response(Some("user@example.com"))))
        .expect(1)
        .mount(&server)
        .await;

    let endpoint = whoami_endpoint(&server.uri());
    let auth = hydrate_personal_access_token(&create_client(), &endpoint, "at-example")
        .await
        .expect("personal access token hydration should succeed");

    assert_eq!(
        auth,
        PersonalAccessTokenAuth {
            access_token: "at-example".to_string(),
            metadata: PersonalAccessTokenMetadata {
                email: Some("user@example.com".to_string()),
                chatgpt_user_id: "user-123".to_string(),
                chatgpt_account_id: "account-123".to_string(),
                chatgpt_plan_type: "enterprise".to_string(),
                chatgpt_account_is_fedramp: true,
            },
        }
    );
    server.verify().await;
}

#[tokio::test]
async fn hydrate_preserves_missing_email() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(WHOAMI_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(response(/*email*/ None)))
        .expect(1)
        .mount(&server)
        .await;

    let endpoint = whoami_endpoint(&server.uri());
    let auth = hydrate_personal_access_token(&create_client(), &endpoint, "at-example")
        .await
        .expect("personal access token hydration should accept missing email");

    assert_eq!(
        auth,
        PersonalAccessTokenAuth {
            access_token: "at-example".to_string(),
            metadata: PersonalAccessTokenMetadata {
                email: None,
                chatgpt_user_id: "user-123".to_string(),
                chatgpt_account_id: "account-123".to_string(),
                chatgpt_plan_type: "enterprise".to_string(),
                chatgpt_account_is_fedramp: true,
            },
        }
    );
    server.verify().await;
}
