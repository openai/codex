use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use http::HeaderMap;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::AuthorizationMetadata;
use rmcp::transport::auth::OAuthState;
use serde_json::Value;
use serde_json::json;
use url::Url;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::McpOAuthClientRegistration;
use super::start_authorization;
use crate::oauth::validate_authorization_server_endpoints;
use crate::oauth_http_client::OAuthHttpClientAdapter;
use crate::utils::MCP_USER_AGENT;
use crate::utils::build_default_headers;

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
    let prepared = start_authorization(
        &format!("{}/mcp", server.uri()),
        Arc::new(OAuthHttpClientAdapter::new(
            Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            ))),
            HeaderMap::new(),
            &format!("{}/mcp", server.uri()),
        )),
        &["read"],
        redirect_uri,
        CALLBACK_ID,
        registration,
    )
    .await?;
    let state = prepared.oauth_state;
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

#[test]
fn legacy_provider_exceptions_require_exact_issuer_and_endpoint_origins() -> Result<()> {
    for (issuer, authorization_endpoint, token_endpoint, accepted) in [
        (
            "https://api.figma.com",
            "https://www.figma.com/oauth/mcp",
            "https://api.figma.com/v1/oauth/token",
            true,
        ),
        (
            "https://agent.robinhood.com/mcp/trading",
            "https://robinhood.com/oauth",
            "https://api.robinhood.com/oauth2/token/",
            true,
        ),
        (
            "https://api.figma.com.attacker.example",
            "https://www.figma.com/oauth/mcp",
            "https://api.figma.com.attacker.example/token",
            false,
        ),
        (
            "https://api.figma.com",
            "https://www.figma.com/oauth/mcp",
            "https://attacker.example/token",
            false,
        ),
        (
            "http://api.figma.com",
            "https://www.figma.com/oauth/mcp",
            "http://api.figma.com/v1/oauth/token",
            false,
        ),
        (
            "https://agent.robinhood.com/mcp/attacker",
            "https://robinhood.com/oauth",
            "https://api.robinhood.com/oauth2/token/",
            false,
        ),
        (
            "https://agent.robinhood.com/mcp/trading",
            "https://robinhood.com.attacker.example/oauth",
            "https://api.robinhood.com/oauth2/token/",
            false,
        ),
    ] {
        let metadata: AuthorizationMetadata = serde_json::from_value(json!({
            "issuer": issuer,
            "authorization_endpoint": authorization_endpoint,
            "token_endpoint": token_endpoint,
        }))?;
        assert_eq!(
            validate_authorization_server_endpoints(&metadata).is_ok(),
            accepted,
            "unexpected validation result for issuer {issuer}",
        );
    }

    Ok(())
}

#[tokio::test]
async fn verified_issuer_can_delegate_authorization_and_token_to_one_origin() -> Result<()> {
    let issuer_server = MockServer::start().await;
    let endpoint_server = MockServer::start().await;
    let issuer = format!("{}/mcp", issuer_server.uri());
    let redirect_uri = format!("http://127.0.0.1:43123/callback/{CALLBACK_ID}");

    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{}/authorize", endpoint_server.uri()),
            "token_endpoint": format!("{}/token", endpoint_server.uri()),
            "registration_endpoint": format!("{}/register", endpoint_server.uri()),
            "token_endpoint_auth_methods_supported": ["none"],
            "code_challenge_methods_supported": ["S256"],
        })))
        .expect(1)
        .mount(&issuer_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/register"))
        .respond_with(|request: &Request| {
            let registration: Value = serde_json::from_slice(&request.body)
                .expect("dynamic registration should contain JSON");
            ResponseTemplate::new(200).set_body_json(json!({
                "client_id": "delegated-provider-client",
                "redirect_uris": registration["redirect_uris"],
            }))
        })
        .expect(1)
        .mount(&endpoint_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "delegated-provider-token",
            "token_type": "Bearer",
        })))
        .expect(1)
        .mount(&endpoint_server)
        .await;

    let (mut state, query) = authorization(
        &issuer_server,
        &redirect_uri,
        McpOAuthClientRegistration::Dcr,
    )
    .await?;
    assert_eq!(query["client_id"], "delegated-provider-client");
    assert_eq!(
        Url::parse(&state.get_authorization_url().await?)?.origin(),
        Url::parse(&endpoint_server.uri())?.origin(),
    );

    state
        .handle_callback_with_issuer("valid-authorization-code", &query["state"], None)
        .await?;
    let token_requests = requests_to(&endpoint_server, "/token").await;
    assert_eq!(token_requests.len(), 1);
    assert!(
        url::form_urlencoded::parse(&token_requests[0].body)
            .any(|(name, _)| name == "code_verifier")
    );
    issuer_server.verify().await;
    endpoint_server.verify().await;
    Ok(())
}

#[tokio::test]
async fn resource_headers_follow_same_origin_registration_redirect_and_sdk_auth_wins() -> Result<()>
{
    const RESOURCE_AUTHORIZATION: &str = "Bearer resource-only-secret";
    const RESOURCE_API_KEY: &str = "resource-api-key-secret";
    // These dummy OAuth credentials exist only in this test's local mock server.
    const DUMMY_CLIENT_ID: &str = "dummy-test-client-id";
    const DUMMY_CLIENT_SECRET: &str = "dummy-test-client-secret";
    let sdk_authorization = format!(
        "Basic {}",
        STANDARD.encode(format!("{DUMMY_CLIENT_ID}:{DUMMY_CLIENT_SECRET}"))
    );

    let server = MockServer::start().await;
    let authorization_server = MockServer::start().await;
    let resource_url = format!("{}/mcp", server.uri());
    let resource_metadata_url = format!("{}/resource-metadata", server.uri());
    let redirect_uri = format!("http://127.0.0.1:43123/callback/{CALLBACK_ID}");

    Mock::given(method("GET"))
        .and(path("/mcp"))
        .and(header("authorization", RESOURCE_AUTHORIZATION))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .respond_with(ResponseTemplate::new(401).insert_header(
            "www-authenticate",
            format!("Bearer resource_metadata=\"{resource_metadata_url}\""),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/resource-metadata"))
        .and(header("authorization", RESOURCE_AUTHORIZATION))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource": resource_url,
            "authorization_servers": [authorization_server.uri()],
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .and(header("user-agent", MCP_USER_AGENT))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": authorization_server.uri(),
            "authorization_endpoint": format!("{}/authorize", authorization_server.uri()),
            "token_endpoint": format!("{}/token", server.uri()),
            "registration_endpoint": format!("{}/register", server.uri()),
            "token_endpoint_auth_methods_supported": ["client_secret_basic"],
            "code_challenge_methods_supported": ["S256"],
        })))
        .expect(1)
        .mount(&authorization_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/register"))
        .and(header("authorization", RESOURCE_AUTHORIZATION))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "/register/"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/register/"))
        .and(header("authorization", RESOURCE_AUTHORIZATION))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "client_id": DUMMY_CLIENT_ID,
            "client_secret": DUMMY_CLIENT_SECRET,
            "redirect_uris": [redirect_uri],
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(header("authorization", sdk_authorization))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test-access-token",
            "token_type": "Bearer",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let default_headers = build_default_headers(
        Some(HashMap::from([
            (
                "Authorization".to_string(),
                RESOURCE_AUTHORIZATION.to_string(),
            ),
            ("X-Api-Key".to_string(), RESOURCE_API_KEY.to_string()),
        ])),
        /*env_http_headers*/ None,
    )?;
    let prepared = start_authorization(
        &resource_url,
        Arc::new(OAuthHttpClientAdapter::new(
            Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            ))),
            default_headers,
            &resource_url,
        )),
        &[],
        &redirect_uri,
        CALLBACK_ID,
        McpOAuthClientRegistration::Dcr,
    )
    .await?;
    assert_eq!(
        prepared.authorization_server_issuer.as_deref(),
        Some(authorization_server.uri().as_str())
    );
    let mut state = prepared.oauth_state;
    let csrf_state = Url::parse(&state.get_authorization_url().await?)?
        .query_pairs()
        .find(|(name, _)| name == "state")
        .expect("authorization request should contain a CSRF state")
        .1
        .into_owned();
    state
        .handle_callback_with_issuer("valid-authorization-code", &csrf_state, None)
        .await?;

    let authorization_requests = authorization_server
        .received_requests()
        .await
        .expect("authorization server should record requests");
    assert_eq!(authorization_requests.len(), 1);
    assert_eq!(authorization_requests[0].headers.get("authorization"), None);
    assert_eq!(authorization_requests[0].headers.get("x-api-key"), None);
    server.verify().await;
    authorization_server.verify().await;
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
