use super::*;
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

#[test]
fn access_token_classifier_treats_at_prefix_as_personal_access_token() {
    assert!(matches!(
        classify_codex_access_token("at-example"),
        CodexAccessToken::PersonalAccessToken("at-example")
    ));
    assert!(matches!(
        classify_codex_access_token("header.payload.signature"),
        CodexAccessToken::AgentIdentityJwt("header.payload.signature")
    ));
}

#[tokio::test]
async fn hydrate_sends_bearer_token_and_preserves_nullable_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(WHOAMI_PATH))
        .and(header("authorization", "Bearer at-example"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response(/*email*/ None)))
        .expect(1)
        .mount(&server)
        .await;

    let auth = hydrate_personal_access_token(&create_client(), &server.uri(), "at-example")
        .await
        .expect("personal access token hydration should succeed");

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
