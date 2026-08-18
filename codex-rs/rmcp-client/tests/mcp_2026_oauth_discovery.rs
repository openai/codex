use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use codex_exec_server::Environment;
use codex_exec_server::HttpClient;
use codex_rmcp_client::OAuthDiscoveryTimeout;
use codex_rmcp_client::StreamableHttpOAuthDiscovery;
use codex_rmcp_client::StreamableHttpRedirectMode;
use codex_rmcp_client::discover_streamable_http_oauth;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::AuthError;
use serde_json::json;
use tokio::time::timeout;
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
// This is a test safety ceiling, not rmcp's private redirect limit.
const MAX_METADATA_REDIRECT_REQUESTS: u64 = 100;
const REDIRECT_DISCOVERY_TEST_TIMEOUT: Duration = Duration::from_secs(5);

type DiscoveryResult = anyhow::Result<Option<StreamableHttpOAuthDiscovery>>;

#[derive(Clone, Copy)]
enum AuthorizationMetadataIssuer {
    Matching,
    Missing,
    Mismatched,
}

#[derive(Clone, Copy)]
enum MetadataDelivery {
    Direct,
    SameOriginRedirects,
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

fn local_http_client() -> Arc<dyn HttpClient> {
    Environment::default_for_tests().get_http_client()
}

async fn discover_with_local_http_client(
    resource_url: &str,
    redirect_mode: StreamableHttpRedirectMode,
) -> DiscoveryResult {
    discover_streamable_http_oauth(
        resource_url,
        resource_headers(),
        /*env_http_headers*/ None,
        local_http_client(),
        OAuthDiscoveryTimeout::LOCAL,
        redirect_mode,
    )
    .await
}

async fn assert_authorization_requests_exclude_resource_headers(
    authorization_server: &MockServer,
) -> anyhow::Result<()> {
    let requests = authorization_server
        .received_requests()
        .await
        .context("authorization-server request recording should be enabled")?;
    assert!(
        !requests.is_empty(),
        "OAuth discovery must contact the authorization server"
    );
    for request in requests {
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
    Ok(())
}

fn assert_cross_origin_redirect_rejected(
    discovery: DiscoveryResult,
    redirect_target: &str,
) -> anyhow::Result<()> {
    let error = discovery
        .err()
        .context("cross-origin OAuth metadata redirects must be rejected")?;
    assert!(
        matches!(
            error.downcast_ref::<AuthError>(),
            Some(AuthError::MetadataError(reason))
                if reason.contains("OAuth discovery redirect to non-same-origin URL rejected")
                    && reason.contains(redirect_target)
        ),
        "expected the cross-origin redirect rejection for `{redirect_target}`: {error:#}",
    );
    Ok(())
}

async fn assert_legacy_oauth_without_starting_an_mcp_session(
    metadata_issuer: AuthorizationMetadataIssuer,
    metadata_delivery: MetadataDelivery,
) -> anyhow::Result<()> {
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

    let (resource_metadata_path, authorization_metadata_path) = match metadata_delivery {
        MetadataDelivery::Direct => (
            "/resource-metadata",
            "/.well-known/oauth-authorization-server",
        ),
        MetadataDelivery::SameOriginRedirects => {
            Mock::given(method("GET"))
                .and(path("/resource-metadata"))
                .and(header("authorization", RESOURCE_AUTHORIZATION))
                .and(header("x-api-key", RESOURCE_API_KEY))
                .and(header("user-agent", RESOURCE_USER_AGENT))
                .respond_with(
                    ResponseTemplate::new(302)
                        .insert_header("location", "/redirected-resource-metadata"),
                )
                .expect(2)
                .mount(&resource_server)
                .await;
            Mock::given(method("GET"))
                .and(path("/.well-known/oauth-authorization-server"))
                .respond_with(
                    ResponseTemplate::new(302)
                        .insert_header("location", "/redirected-authorization-metadata"),
                )
                .expect(2)
                .mount(&authorization_server)
                .await;
            (
                "/redirected-resource-metadata",
                "/redirected-authorization-metadata",
            )
        }
    };

    Mock::given(method("GET"))
        .and(path(resource_metadata_path))
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
        .and(path(authorization_metadata_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
        .expect(2)
        .mount(&authorization_server)
        .await;

    for redirect_mode in [
        StreamableHttpRedirectMode::Legacy,
        StreamableHttpRedirectMode::AgentPluginV1,
    ] {
        let discovery = discover_with_local_http_client(&resource_url, redirect_mode).await;
        match metadata_issuer {
            AuthorizationMetadataIssuer::Matching | AuthorizationMetadataIssuer::Missing => {
                assert_eq!(
                    discovery?,
                    Some(StreamableHttpOAuthDiscovery {
                        scopes_supported: Some(vec!["mcp:read".to_string()]),
                    }),
                );
            }
            AuthorizationMetadataIssuer::Mismatched => {
                let error = discovery
                    .err()
                    .context("a mismatched issuer must not be accepted")?;
                assert!(
                    matches!(
                        error.downcast_ref::<AuthError>(),
                        Some(AuthError::AuthorizationServerMismatch {
                            expected_issuer,
                            received_issuer,
                        }) if expected_issuer.trim_end_matches('/') == authorization_server.uri()
                            && received_issuer == "https://unexpected-issuer.example"
                    ),
                    "expected the original authorization-server issuer to remain bound: {error:#}",
                );
            }
        }
    }

    resource_server.verify().await;
    authorization_server.verify().await;
    assert_authorization_requests_exclude_resource_headers(&authorization_server).await?;
    Ok(())
}

#[tokio::test]
async fn oauth_discovery_uses_get_first_without_starting_a_legacy_mcp_session() -> anyhow::Result<()>
{
    assert_legacy_oauth_without_starting_an_mcp_session(
        AuthorizationMetadataIssuer::Matching,
        MetadataDelivery::Direct,
    )
    .await
}

#[tokio::test]
async fn legacy_oauth_discovery_follows_same_origin_metadata_redirects() -> anyhow::Result<()> {
    assert_legacy_oauth_without_starting_an_mcp_session(
        AuthorizationMetadataIssuer::Matching,
        MetadataDelivery::SameOriginRedirects,
    )
    .await
}

#[tokio::test]
async fn legacy_oauth_discovery_rejects_cross_origin_authorization_metadata_redirects()
-> anyhow::Result<()> {
    let resource_server = MockServer::start().await;
    let authorization_server = MockServer::start().await;
    let redirect_target = MockServer::start().await;
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
        .expect(1)
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
        .expect(1)
        .mount(&resource_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(302).insert_header(
            "location",
            format!(
                "{}/redirected-authorization-metadata",
                redirect_target.uri()
            ),
        ))
        .expect(1)
        .mount(&authorization_server)
        .await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&redirect_target)
        .await;

    let discovery =
        discover_with_local_http_client(&resource_url, StreamableHttpRedirectMode::Legacy).await;

    assert_cross_origin_redirect_rejected(discovery, &redirect_target.uri())?;
    assert!(
        redirect_target
            .received_requests()
            .await
            .context("cross-origin request recording should be enabled")?
            .is_empty(),
        "OAuth authorization-server metadata discovery must not contact a cross-origin redirect target",
    );
    redirect_target.verify().await;
    authorization_server.verify().await;
    resource_server.verify().await;
    assert_authorization_requests_exclude_resource_headers(&authorization_server).await?;
    Ok(())
}

#[tokio::test]
async fn legacy_oauth_discovery_rejects_authorization_metadata_redirect_cycles()
-> anyhow::Result<()> {
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
        .expect(1)
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
        .expect(1)
        .mount(&resource_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", "/.well-known/oauth-authorization-server"),
        )
        .expect(2..=MAX_METADATA_REDIRECT_REQUESTS)
        .mount(&authorization_server)
        .await;

    let discovery = timeout(
        REDIRECT_DISCOVERY_TEST_TIMEOUT,
        discover_with_local_http_client(&resource_url, StreamableHttpRedirectMode::Legacy),
    )
    .await
    .context("OAuth metadata redirect cycles must fail within the bounded discovery timeout")?;
    let error = discovery
        .err()
        .context("OAuth metadata redirect cycles must be rejected")?;
    assert!(
        matches!(
            error.downcast_ref::<AuthError>(),
            Some(AuthError::MetadataError(reason))
                if reason.contains("OAuth discovery exceeded ") && reason.contains(" redirects")
        ),
        "expected the SDK to report its bounded OAuth discovery redirect limit: {error:#}",
    );
    authorization_server.verify().await;
    resource_server.verify().await;
    assert_authorization_requests_exclude_resource_headers(&authorization_server).await?;
    Ok(())
}

#[tokio::test]
async fn legacy_oauth_discovery_rejects_cross_origin_resource_metadata_redirects()
-> anyhow::Result<()> {
    let resource_server = MockServer::start().await;
    let redirect_target = MockServer::start().await;
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
        .expect(1)
        .mount(&resource_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/resource-metadata"))
        .and(header("authorization", RESOURCE_AUTHORIZATION))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .and(header("user-agent", RESOURCE_USER_AGENT))
        .respond_with(ResponseTemplate::new(302).insert_header(
            "location",
            format!("{}/redirect-target", redirect_target.uri()),
        ))
        .expect(1)
        .mount(&resource_server)
        .await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&redirect_target)
        .await;

    let discovery =
        discover_with_local_http_client(&resource_url, StreamableHttpRedirectMode::Legacy).await;

    assert_cross_origin_redirect_rejected(discovery, &redirect_target.uri())?;
    assert!(
        redirect_target
            .received_requests()
            .await
            .context("cross-origin request recording should be enabled")?
            .is_empty(),
        "OAuth discovery must not contact a cross-origin redirect target",
    );
    redirect_target.verify().await;
    resource_server.verify().await;
    Ok(())
}

#[tokio::test]
async fn legacy_oauth_discovery_accepts_authorization_metadata_without_an_issuer()
-> anyhow::Result<()> {
    for metadata_delivery in [
        MetadataDelivery::Direct,
        MetadataDelivery::SameOriginRedirects,
    ] {
        assert_legacy_oauth_without_starting_an_mcp_session(
            AuthorizationMetadataIssuer::Missing,
            metadata_delivery,
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn legacy_oauth_discovery_rejects_an_explicit_mismatched_issuer() -> anyhow::Result<()> {
    for metadata_delivery in [
        MetadataDelivery::Direct,
        MetadataDelivery::SameOriginRedirects,
    ] {
        assert_legacy_oauth_without_starting_an_mcp_session(
            AuthorizationMetadataIssuer::Mismatched,
            metadata_delivery,
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn oauth_discovery_does_not_invent_support_for_an_unauthenticated_legacy_server()
-> anyhow::Result<()> {
    let resource_server = MockServer::start().await;

    let server_url = format!("{}/mcp", resource_server.uri());
    let local_discovery = discover_streamable_http_oauth(
        &server_url,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        local_http_client(),
        OAuthDiscoveryTimeout::LOCAL,
        StreamableHttpRedirectMode::Legacy,
    )
    .await?;

    assert_eq!(local_discovery, None);
    Ok(())
}
