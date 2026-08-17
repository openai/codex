use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_rmcp_client::OAuthDiscoveryTimeout;
use codex_rmcp_client::StreamableHttpOAuthDiscovery;
use codex_rmcp_client::StreamableHttpRedirectMode;
use codex_rmcp_client::discover_streamable_http_oauth;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::AuthError;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const RESOURCE_AUTHORIZATION: &str = "Bearer resource-only-secret";
const RESOURCE_API_KEY: &str = "resource-api-key-secret";
const RESOURCE_USER_AGENT: &str = "resource-only-user-agent";
const MCP_USER_AGENT: &str = concat!("codex-mcp-client/", env!("CARGO_PKG_VERSION"));

type DiscoveryResult = anyhow::Result<Option<StreamableHttpOAuthDiscovery>>;

#[derive(Clone, Copy)]
enum AuthorizationMetadataIssuer {
    Matching,
    Missing,
    Mismatched,
}

fn resource_headers() -> Option<HashMap<String, String>> {
    Some(HashMap::from([
        (
            "Authorization".to_string(),
            RESOURCE_AUTHORIZATION.to_string(),
        ),
        ("X-Api-Key".to_string(), RESOURCE_API_KEY.to_string()),
        ("User-Agent".to_string(), RESOURCE_USER_AGENT.to_string()),
    ]))
}

async fn discover_legacy_oauth_without_starting_an_mcp_session(
    metadata_issuer: AuthorizationMetadataIssuer,
) -> anyhow::Result<(DiscoveryResult, DiscoveryResult)> {
    let resource_server = MockServer::start().await;
    let authorization_server = MockServer::start().await;
    let resource_url = format!("{}/mcp", resource_server.uri());
    let resource_metadata_url = format!("{}/resource-metadata", resource_server.uri());

    Mock::given(method("GET"))
        .and(path("/mcp"))
        .and(header("authorization", RESOURCE_AUTHORIZATION))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .and(header("user-agent", RESOURCE_USER_AGENT))
        .respond_with(ResponseTemplate::new(401).insert_header(
            "www-authenticate",
            format!("Bearer resource_metadata=\"{resource_metadata_url}\""),
        ))
        .expect(2)
        .mount(&resource_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&resource_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/resource-metadata"))
        .and(header("authorization", RESOURCE_AUTHORIZATION))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .and(header("user-agent", RESOURCE_USER_AGENT))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource": resource_url,
            "authorization_servers": [authorization_server.uri()],
        })))
        .expect(2)
        .mount(&resource_server)
        .await;

    let mut metadata = json!({
        "authorization_endpoint": format!("{}/authorize", authorization_server.uri()),
        "token_endpoint": format!("{}/token", authorization_server.uri()),
        "scopes_supported": ["mcp:read"],
        "code_challenge_methods_supported": ["S256"],
    });
    match metadata_issuer {
        AuthorizationMetadataIssuer::Matching => {
            metadata["issuer"] = json!(authorization_server.uri());
        }
        AuthorizationMetadataIssuer::Missing => {}
        AuthorizationMetadataIssuer::Mismatched => {
            metadata["issuer"] = json!("https://unexpected-issuer.example");
        }
    }
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
        .expect(2)
        .mount(&authorization_server)
        .await;

    let local_discovery = discover_streamable_http_oauth(
        &resource_url,
        resource_headers(),
        /*env_http_headers*/ None,
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        OAuthDiscoveryTimeout::LOCAL,
        StreamableHttpRedirectMode::Legacy,
    )
    .await;
    let plugin_discovery = discover_streamable_http_oauth(
        &resource_url,
        resource_headers(),
        /*env_http_headers*/ None,
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        OAuthDiscoveryTimeout::LOCAL,
        StreamableHttpRedirectMode::AgentPluginV1,
    )
    .await;

    resource_server.verify().await;
    authorization_server.verify().await;
    for request in authorization_server
        .received_requests()
        .await
        .ok_or_else(|| {
            anyhow::anyhow!("authorization-server request recording should be enabled")
        })?
    {
        assert_eq!(request.headers.get("authorization"), None);
        assert_eq!(request.headers.get("x-api-key"), None);
        assert_eq!(
            request
                .headers
                .get("user-agent")
                .map(wiremock::http::HeaderValue::as_bytes),
            Some(MCP_USER_AGENT.as_bytes())
        );
    }
    Ok((local_discovery, plugin_discovery))
}

#[tokio::test]
async fn oauth_discovery_uses_get_first_without_starting_a_legacy_mcp_session() -> anyhow::Result<()>
{
    let discoveries = discover_legacy_oauth_without_starting_an_mcp_session(
        AuthorizationMetadataIssuer::Matching,
    )
    .await?;

    for discovery in [discoveries.0, discoveries.1] {
        assert_eq!(
            discovery?,
            Some(StreamableHttpOAuthDiscovery {
                scopes_supported: Some(vec!["mcp:read".to_string()]),
            }),
        );
    }
    Ok(())
}

#[tokio::test]
async fn legacy_oauth_discovery_accepts_authorization_metadata_without_an_issuer()
-> anyhow::Result<()> {
    let discoveries =
        discover_legacy_oauth_without_starting_an_mcp_session(AuthorizationMetadataIssuer::Missing)
            .await?;

    for discovery in [discoveries.0, discoveries.1] {
        assert_eq!(
            discovery?,
            Some(StreamableHttpOAuthDiscovery {
                scopes_supported: Some(vec!["mcp:read".to_string()]),
            }),
        );
    }
    Ok(())
}

#[tokio::test]
async fn legacy_oauth_discovery_rejects_an_explicit_mismatched_issuer() -> anyhow::Result<()> {
    let discoveries = discover_legacy_oauth_without_starting_an_mcp_session(
        AuthorizationMetadataIssuer::Mismatched,
    )
    .await?;

    for discovery in [discoveries.0, discoveries.1] {
        let error = discovery.expect_err("a mismatched issuer must not be accepted");
        assert!(
            matches!(
                error.downcast_ref::<AuthError>(),
                Some(AuthError::AuthorizationServerMismatch { .. }),
            ),
            "expected an authorization-server issuer mismatch: {error:#}",
        );
    }
    Ok(())
}

#[tokio::test]
async fn oauth_discovery_does_not_invent_support_for_an_unauthenticated_legacy_server()
-> anyhow::Result<()> {
    let resource_server = MockServer::start().await;

    let server_url = format!("{}/mcp", resource_server.uri());
    let executor_discovery = discover_streamable_http_oauth(
        &server_url,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        OAuthDiscoveryTimeout::LOCAL,
        StreamableHttpRedirectMode::Legacy,
    )
    .await?;
    let local_discovery = discover_streamable_http_oauth(
        &server_url,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        OAuthDiscoveryTimeout::LOCAL,
        StreamableHttpRedirectMode::Legacy,
    )
    .await?;

    assert_eq!(executor_discovery, None);
    assert_eq!(local_discovery, None);
    Ok(())
}
