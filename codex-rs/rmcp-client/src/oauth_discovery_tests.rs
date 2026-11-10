#![cfg(test)]

//! Tests published metadata discovery without attaching application credentials.

use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::HttpClient;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::discover_authorization_metadata;

fn http_client() -> Arc<dyn HttpClient> {
    Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    )))
}

async fn assert_no_credentials(server: &MockServer) {
    for request in server.received_requests().await.unwrap() {
        assert!(request.body.is_empty());
        for header in ["authorization", "cookie", "x-gateway-authorization"] {
            assert!(!request.headers.contains_key(header));
        }
    }
}

#[tokio::test]
async fn returns_conventional_endpoints_only_when_published() -> Result<()> {
    let server = MockServer::start().await;
    let issuer = server.uri();
    let authorization_url = format!("{issuer}/authorize");
    let token_url = format!("{issuer}/token");
    let device_url = format!("{issuer}/device");

    // Every candidate returns 404. RMCP guesses the same URLs this server will publish.
    assert!(
        discover_authorization_metadata(&issuer, http_client())
            .await?
            .is_none()
    );

    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer,
            "authorization_endpoint": authorization_url,
            "token_endpoint": token_url,
            "device_authorization_endpoint": device_url,
        })))
        .expect(1)
        .mount(&server)
        .await;
    let metadata = discover_authorization_metadata(&issuer, http_client())
        .await?
        .expect("the server published authorization metadata");

    assert_eq!(metadata.authorization_endpoint, authorization_url);
    assert_eq!(metadata.token_endpoint, token_url);
    assert_eq!(
        metadata
            .additional_fields
            .get("device_authorization_endpoint"),
        Some(&json!(device_url))
    );
    Ok(())
}

#[tokio::test]
async fn propagates_discovery_failures() {
    let server = MockServer::start().await;
    let origin = server.uri();
    let issuer = format!("{origin}/tenant-a");
    let wrong_tenant = json!({
        "issuer": format!("{origin}/tenant-b"),
        "authorization_endpoint": format!("{origin}/authorize"),
        "token_endpoint": format!("{origin}/token"),
    });

    for (case, response) in [
        ("unavailable issuer", ResponseTemplate::new(503)),
        (
            "same host, different tenant",
            ResponseTemplate::new(200).set_body_json(wrong_tenant),
        ),
    ] {
        let _mock = Mock::given(method("GET"))
            .and(path("/.well-known/oauth-authorization-server/tenant-a"))
            .respond_with(response)
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        let result = discover_authorization_metadata(&issuer, http_client()).await;
        assert!(result.is_err(), "{case} should fail discovery");
    }
}

#[tokio::test]
async fn follows_protected_resource_metadata_to_a_separate_issuer() -> Result<()> {
    let resource = MockServer::start().await;
    let authorization = MockServer::start().await;
    let issuer = authorization.uri();
    let token_url = format!("{issuer}/exchange");

    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-protected-resource"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource": resource.uri(),
            "authorization_servers": [issuer],
        })))
        .expect(1)
        .mount(&resource)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/login"),
            "token_endpoint": token_url,
        })))
        .expect(1)
        .mount(&authorization)
        .await;

    let metadata = discover_authorization_metadata(&resource.uri(), http_client())
        .await?
        .expect("the separate issuer published authorization metadata");
    assert_eq!(metadata.issuer.as_deref(), Some(issuer.as_str()));
    assert_eq!(metadata.token_endpoint, token_url);
    assert_no_credentials(&resource).await;
    assert_no_credentials(&authorization).await;
    Ok(())
}
