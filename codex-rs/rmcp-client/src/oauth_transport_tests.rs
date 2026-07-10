use std::collections::HashMap;
use std::sync::Arc;

use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use reqwest::header::HeaderMap;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::auth::OAuthState;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use rmcp::transport::common::client_side_sse::SseRetryPolicy;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::OAuthTransportClient;
use super::OAuthTransportFailureState;
use crate::http_client_adapter::StreamableHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::WrappedOAuthTokenResponse;
use crate::oauth::request_oauth_token_response;
use crate::oauth_http_client::OAuthHttpClientAdapter;

const SERVER_NAME: &str = "oauth-transport-response-test";
const ACCESS_TOKEN_A: &str = "response-access-a";
const REFRESH_TOKEN_A: &str = "response-refresh-a";

#[tokio::test]
async fn rmcp_owned_response_reports_rejected_token_without_refreshing() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": ["scope-a"],
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN_A}")))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let server_url = format!("{}/mcp", server.uri());
    let initial_tokens = initial_tokens(&server_url);
    let http_client = codex_exec_server::Environment::default_for_tests().get_http_client();
    let oauth_http_client = Arc::new(OAuthHttpClientAdapter::new(
        Arc::clone(&http_client),
        HeaderMap::new(),
    ));
    let mut oauth_state =
        OAuthState::new_with_oauth_http_client(server_url.clone(), oauth_http_client).await?;
    oauth_state
        .set_credentials(
            &initial_tokens.client_id,
            request_oauth_token_response(&initial_tokens),
        )
        .await?;
    let manager = match oauth_state {
        OAuthState::Authorized(manager) | OAuthState::Unauthorized(manager) => manager,
        _ => anyhow::bail!("unexpected OAuth state during response test setup"),
    };
    let auth_client = AuthClient::new(
        StreamableHttpClientAdapter::new(
            Arc::clone(&http_client),
            HeaderMap::new(),
            /*auth_provider*/ None,
        )
        .with_rejected_token_attribution(),
        manager,
    );
    let failure_state = OAuthTransportFailureState::default();
    let client = OAuthTransportClient::new(auth_client, failure_state.clone());
    let response_message: ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": "server-request-1",
        "result": {
            "action": "accept",
            "content": { "confirmed": true }
        }
    }))?;

    let error = client
        .post_message(
            Arc::from(server_url),
            response_message,
            Some(Arc::from("response-session")),
            /*auth_token*/ None,
            HashMap::new(),
        )
        .await
        .expect_err("the server should reject the response token");

    assert!(matches!(
        error,
        StreamableHttpError::Client(StreamableHttpClientAdapterError::AccessTokenRejected { .. })
    ));
    assert_eq!(
        failure_state
            .pending_rejected_access_token()
            .as_ref()
            .map(|token| token.secret().as_str()),
        Some(ACCESS_TOKEN_A)
    );
    assert_eq!(
        failure_state.retry_policy().retry(/*current_times*/ 1),
        None
    );
    server.verify().await;
    Ok(())
}

#[test]
fn pending_auth_failure_stops_sse_retry_until_recovery_finishes() {
    let failure_state = OAuthTransportFailureState::default();
    let rejected_access_token = AccessToken::new(ACCESS_TOKEN_A.to_string());

    failure_state.record_rejected_access_token(rejected_access_token.clone());
    assert_eq!(
        failure_state.retry_policy().retry(/*current_times*/ 1),
        None
    );

    failure_state.finish_recovery(&rejected_access_token);
    assert!(failure_state.pending_rejected_access_token().is_none());
    assert!(
        failure_state
            .retry_policy()
            .retry(/*current_times*/ 1)
            .is_some()
    );
}

fn initial_tokens(server_url: &str) -> StoredOAuthTokens {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(ACCESS_TOKEN_A.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN_A.to_string())));
    response.set_expires_in(None);
    StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: None,
    }
}
