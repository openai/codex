use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use http::HeaderMap;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::OAuthState;
use serde_json::Value;
use serde_json::json;
use url::Url;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::McpOAuthClientRegistration;
use super::start_authorization;
use crate::oauth_http_client::OAuthHttpClientAdapter;

const CALLBACK_ID: &str = "abc123ABC_-x";

async fn oauth_server(overrides: Value) -> MockServer {
    let server = MockServer::start().await;
    let base_url = server.uri();
    let mut metadata = json!({
        "authorization_endpoint": format!("{base_url}/authorize"),
        "token_endpoint": format!("{base_url}/token"),
        "registration_endpoint": format!("{base_url}/register"),
        "client_id_metadata_document_supported": true,
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": ["read", "offline_access"],
    });
    metadata
        .as_object_mut()
        .expect("metadata should be an object")
        .extend(
            overrides
                .as_object()
                .expect("overrides should be an object")
                .clone(),
        );
    if metadata["authorization_response_iss_parameter_supported"] == json!(true)
        && metadata.get("issuer").is_none()
    {
        metadata["issuer"] = json!(format!("{base_url}/mcp"));
    }

    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/register"))
        .respond_with(|request: &Request| {
            let registration: Value = serde_json::from_slice(&request.body)
                .expect("dynamic registration should contain JSON");
            ResponseTemplate::new(200).set_body_json(json!({
                "client_id": "dcr-client",
                "redirect_uris": registration["redirect_uris"],
            }))
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test-access-token",
            "token_type": "Bearer",
            "refresh_token": "test-refresh-token",
        })))
        .mount(&server)
        .await;

    server
}

async fn requests_to(server: &MockServer, request_path: &str) -> Vec<Request> {
    server
        .received_requests()
        .await
        .expect("mock server should record requests")
        .into_iter()
        .filter(|request| request.url.path() == request_path)
        .collect()
}

async fn authorization(
    server: &MockServer,
    redirect_uri: &str,
    registration: McpOAuthClientRegistration,
) -> Result<(OAuthState, HashMap<String, String>)> {
    let state = start_authorization(
        &format!("{}/mcp", server.uri()),
        Arc::new(OAuthHttpClientAdapter::new(
            Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            ))),
            HeaderMap::new(),
        )),
        &["read"],
        redirect_uri,
        CALLBACK_ID,
        registration,
    )
    .await?;
    let query = Url::parse(&state.get_authorization_url().await?)?
        .query_pairs()
        .into_owned()
        .collect();

    Ok((state, query))
}

#[tokio::test]
async fn automatic_cimd_uses_callback_specific_identity() -> Result<()> {
    for host in ["127.0.0.1", "localhost"] {
        let server = oauth_server(json!({})).await;
        let redirect = format!("http://{host}:43123/callback/{CALLBACK_ID}");
        let (mut state, query) =
            authorization(&server, &redirect, McpOAuthClientRegistration::Auto).await?;
        let expected_id = format!("https://chatgpt.com/oauth/codex/{CALLBACK_ID}/client.json");
        assert_eq!(query["client_id"], expected_id);
        assert_eq!(query["redirect_uri"], redirect);
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["scope"], "read offline_access");

        state
            .handle_callback_with_issuer("valid-authorization-code", &query["state"], None)
            .await?;
        let token_requests = requests_to(&server, "/token").await;
        assert_eq!(token_requests.len(), 1);
        let request = &token_requests[0];
        let body: HashMap<_, _> = url::form_urlencoded::parse(&request.body)
            .into_owned()
            .collect();
        assert_eq!(body["client_id"], expected_id);
        assert_eq!(body["redirect_uri"], redirect);
        assert_eq!(body["grant_type"], "authorization_code");
        assert!(body.contains_key("code_verifier"));
        assert!(!body.contains_key("client_secret"));
        assert!(!request.headers.contains_key("authorization"));
        assert!(requests_to(&server, "/register").await.is_empty());
        assert_eq!(
            requests_to(&server, "/.well-known/oauth-authorization-server/mcp")
                .await
                .len(),
            1
        );
    }

    Ok(())
}

#[tokio::test]
async fn registration_selection_preserves_dcr_capabilities_and_exact_redirects() -> Result<()> {
    let native = "http://localhost:43123/callback/abc123ABC_-x";
    let custom = "https://callbacks.example.com/oauth/callback/abc123ABC_-x";
    for (metadata, redirect, registration, expected_redirect) in [
        (
            json!({"client_id_metadata_document_supported": false}),
            native,
            McpOAuthClientRegistration::Auto,
            native,
        ),
        (
            json!({"token_endpoint_auth_methods_supported": null}),
            native,
            McpOAuthClientRegistration::Auto,
            native,
        ),
        (
            json!({"token_endpoint_auth_methods_supported": ["private_key_jwt"]}),
            native,
            McpOAuthClientRegistration::Auto,
            native,
        ),
        (json!({}), custom, McpOAuthClientRegistration::Auto, custom),
        (
            json!({"authorization_response_iss_parameter_supported": true}),
            native,
            McpOAuthClientRegistration::Dcr,
            native,
        ),
    ] {
        let server = oauth_server(metadata).await;
        let (_, query) = authorization(&server, redirect, registration).await?;
        assert_eq!(query["client_id"], "dcr-client");
        assert_eq!(query["redirect_uri"], expected_redirect);
        let registrations = requests_to(&server, "/register").await;
        assert_eq!(registrations.len(), 1);
        let registration: Value = serde_json::from_slice(&registrations[0].body)?;
        assert_eq!(registration["redirect_uris"], json!([expected_redirect]));
    }

    Ok(())
}

#[tokio::test]
async fn invalid_cimd_metadata_and_redirects_fail_without_dynamic_registration() {
    let valid = "http://127.0.0.1:43123/callback/abc123ABC_-x";
    for (metadata, redirect, expected_error) in [
        (
            json!({"token_endpoint_auth_methods_supported": ["private_key_jwt"]}),
            valid,
            "token endpoint auth method `none`",
        ),
        (
            json!({}),
            "http://127.0.0.1.evil.example:43123/callback/abc123ABC_-x",
            "ephemeral loopback callback",
        ),
        (
            json!({}),
            "http://127.0.0.1/callback/abc123ABC_-x",
            "ephemeral loopback callback",
        ),
        (
            json!({}),
            "http://127.0.0.1:43123/callback/wrong-id",
            "ephemeral loopback callback",
        ),
        (
            json!({}),
            "http://127.0.0.1:43123/callback/abc123ABC_-x?unexpected=true",
            "ephemeral loopback callback",
        ),
        (
            json!({}),
            "http://[::1]:43123/callback/abc123ABC_-x",
            "ephemeral loopback callback",
        ),
    ] {
        let server = oauth_server(metadata).await;
        let error = authorization(&server, redirect, McpOAuthClientRegistration::Cimd)
            .await
            .err()
            .expect("invalid CIMD metadata or callback should fail");
        assert!(error.to_string().contains(expected_error));
        assert!(requests_to(&server, "/register").await.is_empty());
        assert!(requests_to(&server, "/token").await.is_empty());
    }

    let server = oauth_server(json!({"registration_endpoint": null})).await;
    let error = authorization(&server, valid, McpOAuthClientRegistration::Dcr)
        .await
        .err()
        .expect("explicit DCR should require an advertised registration endpoint");
    assert!(error.to_string().contains("registration not supported"));
}
