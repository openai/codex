use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::AuthorizationMetadata;
use rmcp::transport::auth::OAuthHttpRedirectPolicy;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES;
use super::OAuthHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::utils::MCP_USER_AGENT;
use crate::utils::build_default_headers;

async fn metadata_manager(
    resource: &MockServer,
    authorization: &MockServer,
    issuer_path: &str,
) -> Result<AuthorizationManager> {
    let resource_url = format!("{}/mcp", resource.uri());
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(401).insert_header(
            "www-authenticate",
            format!(
                "Bearer resource_metadata=\"{}/resource-metadata\"",
                resource.uri()
            ),
        ))
        .mount(resource)
        .await;
    Mock::given(method("GET"))
        .and(path("/resource-metadata"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource": resource_url,
            "authorization_servers": [format!("{}{issuer_path}", authorization.uri())]
        })))
        .mount(resource)
        .await;
    let adapter = OAuthHttpClientAdapter::new(
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        build_default_headers(
            Some(HashMap::from([(
                "X-Api-Key".to_string(),
                "resource-secret".to_string(),
            )])),
            /*env_http_headers*/ None,
        )?,
        &resource_url,
    );
    Ok(AuthorizationManager::new_with_oauth_http_client(resource_url, Arc::new(adapter)).await?)
}

#[tokio::test]
async fn oauth_metadata_503_falls_back_to_oidc() -> Result<()> {
    for (issuer_path, first_oidc_response) in [
        ("", None),
        ("/adfs", Some(ResponseTemplate::new(404))),
        ("/adfs", Some(ResponseTemplate::new(405))),
        ("/adfs", Some(ResponseTemplate::new(503))),
        (
            "/adfs",
            Some(ResponseTemplate::new(200).set_body_string("not metadata")),
        ),
    ] {
        let resource = MockServer::start().await;
        let authorization = MockServer::start().await;
        let manager = metadata_manager(&resource, &authorization, issuer_path).await?;
        let oauth_path = format!("/.well-known/oauth-authorization-server{issuer_path}");
        let oidc_path = format!("{issuer_path}/.well-known/openid-configuration");
        let metadata = json!({
            "issuer": format!("{}{issuer_path}", authorization.uri()),
            "authorization_endpoint": format!("{}/authorize", authorization.uri()),
            "token_endpoint": format!("{}/token", authorization.uri())
        });
        Mock::given(method("GET"))
            .and(path(oauth_path.clone()))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&authorization)
            .await;
        if let Some(response) = first_oidc_response {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/.well-known/openid-configuration{issuer_path}"
                )))
                .respond_with(response)
                .expect(1)
                .mount(&authorization)
                .await;
        }
        Mock::given(method("GET"))
            .and(path(oidc_path.clone()))
            .respond_with(ResponseTemplate::new(200).set_body_json(&metadata))
            .expect(1)
            .mount(&authorization)
            .await;

        let resolved = manager.resolve_metadata().await?;
        let expected: AuthorizationMetadata = serde_json::from_value(metadata)?;
        assert_eq!(
            serde_json::to_value(resolved.metadata)?,
            serde_json::to_value(expected)?
        );
        let requests = authorization.received_requests().await.unwrap();
        let mut expected_paths = vec![oauth_path];
        if !issuer_path.is_empty() {
            expected_paths.push(format!("/.well-known/openid-configuration{issuer_path}"));
        }
        expected_paths.push(oidc_path);
        assert_eq!(
            requests
                .iter()
                .map(|request| request.url.path().to_string())
                .collect::<Vec<_>>(),
            expected_paths
        );
        assert!(
            requests
                .iter()
                .all(|request| !request.headers.contains_key("x-api-key"))
        );
        authorization.verify().await;
    }
    Ok(())
}

#[tokio::test]
async fn oauth_metadata_fallback_preserves_terminal_failures() -> Result<()> {
    let resource = MockServer::start().await;
    let authorization = MockServer::start().await;
    let manager = metadata_manager(&resource, &authorization, "/adfs").await?;
    let redirect_target = MockServer::start().await;
    let wrong_issuer = json!({
        "issuer": format!("{}/other-tenant", authorization.uri()),
        "authorization_endpoint": format!("{}/authorize", authorization.uri()),
        "token_endpoint": format!("{}/token", authorization.uri())
    });
    for (fallback, expected_error, request_count) in [
        (ResponseTemplate::new(404), "503", 3),
        (
            ResponseTemplate::new(200).set_body_json(wrong_issuer),
            "issuer mismatch",
            2,
        ),
        (
            ResponseTemplate::new(307).insert_header("location", redirect_target.uri()),
            "503",
            2,
        ),
        (ResponseTemplate::new(408), "408", 2),
        (ResponseTemplate::new(425), "425", 2),
        (ResponseTemplate::new(429), "429", 2),
        (ResponseTemplate::new(500), "500", 2),
    ] {
        authorization.reset().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/oauth-authorization-server/adfs"))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&authorization)
            .await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration/adfs"))
            .respond_with(fallback)
            .expect(1)
            .mount(&authorization)
            .await;
        let error = manager
            .resolve_metadata()
            .await
            .expect_err("discovery must fail closed");
        assert!(error.to_string().contains(expected_error), "{error}");
        assert_eq!(
            authorization.received_requests().await.unwrap().len(),
            request_count
        );
        authorization.verify().await;
    }
    assert_eq!(redirect_target.received_requests().await.unwrap().len(), 0);
    Ok(())
}

#[tokio::test]
async fn oauth_metadata_fallback_does_not_retry_other_requests_or_statuses() -> Result<()> {
    for (verb, request_path, status) in [
        ("POST", "/.well-known/oauth-authorization-server/adfs", 503),
        ("GET", "/.well-known/oauth-protected-resource/mcp", 503),
        ("GET", "/.well-known/oauth-authorization-server-other", 503),
        ("GET", "/.well-known/oauth-authorization-server/adfs", 500),
    ] {
        let server = MockServer::start().await;
        Mock::given(method(verb))
            .and(path(request_path))
            .respond_with(ResponseTemplate::new(status))
            .mount(&server)
            .await;
        let adapter = OAuthHttpClientAdapter::new(
            Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            ))),
            Default::default(),
            &format!("{}/mcp", server.uri()),
        );
        let response = adapter
            .execute_with_metadata_fallback(
                oauth2::http::Request::builder()
                    .method(verb)
                    .uri(format!("{}{request_path}", server.uri()))
                    .body(Vec::new())?,
                OAuthHttpRedirectPolicy::Stop,
                Some(Duration::from_secs(/*secs*/ 5)),
            )
            .await
            .map_err(|error| anyhow::anyhow!(error))?;
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
    Ok(())
}

struct DelayedMetadataHttpClient;

impl HttpClient for DelayedMetadataHttpClient {
    fn http_request(
        &self,
        _params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        Box::pin(async { panic!("OAuth requests must stream responses") })
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        Box::pin(async move {
            // Each response fits the per-request timeout. Only the shared
            // deadline can prevent the fallback from finishing.
            tokio::time::sleep(Duration::from_millis(/*millis*/ 100)).await;
            Ok((
                HttpRequestResponse {
                    status: if params.url.contains("oauth-authorization-server") {
                        503
                    } else {
                        404
                    },
                    headers: Vec::new(),
                    body: Vec::new().into(),
                },
                HttpResponseBodyStream::from_chunks(Vec::new()),
            ))
        })
    }
}

#[tokio::test]
async fn oauth_metadata_fallback_shares_the_original_timeout() -> Result<()> {
    let adapter = OAuthHttpClientAdapter::new(
        Arc::new(DelayedMetadataHttpClient),
        Default::default(),
        "https://resource.example/mcp",
    );
    let error = adapter
        .execute_with_metadata_fallback(
            oauth2::http::Request::builder()
                .method("GET")
                .uri("https://issuer.example/.well-known/oauth-authorization-server/adfs")
                .body(Vec::new())?,
            OAuthHttpRedirectPolicy::Stop,
            Some(Duration::from_millis(/*millis*/ 150)),
        )
        .await
        .expect_err("fallback requests must share the original deadline");
    assert!(error.to_string().contains("timed out"), "{error}");
    Ok(())
}

#[tokio::test]
async fn oauth_registration_redirects_never_forward_resource_only_headers() -> Result<()> {
    const RESOURCE_API_KEY: &str = "resource-api-key-secret";
    const RESOURCE_USER_AGENT: &str = "resource-only-user-agent";

    for (redirect_mode, has_resource_only_headers) in [
        (StreamableHttpRedirectMode::Legacy, true),
        (StreamableHttpRedirectMode::AgentPluginV1, true),
        (StreamableHttpRedirectMode::Legacy, false),
    ] {
        let resource_server = MockServer::start().await;
        let redirect_target = MockServer::start().await;
        let resource_url = format!("{}/mcp", resource_server.uri());

        Mock::given(method("POST"))
            .and(path("/register"))
            .and(header("content-type", "application/json"))
            .and(header(
                "user-agent",
                if has_resource_only_headers {
                    RESOURCE_USER_AGENT
                } else {
                    MCP_USER_AGENT
                },
            ))
            .respond_with(ResponseTemplate::new(307).insert_header(
                "location",
                format!("{}/redirected-register", redirect_target.uri()),
            ))
            .expect(1)
            .mount(&resource_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/redirected-register"))
            .and(header("content-type", "application/json"))
            .and(header("user-agent", MCP_USER_AGENT))
            .respond_with(ResponseTemplate::new(201))
            .expect(u64::from(!has_resource_only_headers))
            .mount(&redirect_target)
            .await;

        let configured_headers = if has_resource_only_headers {
            HashMap::from([
                ("X-Api-Key".to_string(), RESOURCE_API_KEY.to_string()),
                ("User-Agent".to_string(), RESOURCE_USER_AGENT.to_string()),
            ])
        } else {
            HashMap::from([(
                "Content-Type".to_string(),
                "resource-only-content-type".to_string(),
            )])
        };
        let adapter = OAuthHttpClientAdapter::new_with_redirect_mode(
            Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            ))),
            build_default_headers(Some(configured_headers), /*env_http_headers*/ None)?,
            &resource_url,
            /*has_configured_headers*/ true,
            redirect_mode,
        )?;
        let response = adapter
            .execute_request(
                oauth2::http::Request::builder()
                    .method("POST")
                    .uri(format!("{}/register", resource_server.uri()))
                    .header("content-type", "application/json")
                    .body(br#"{"client_name":"Codex"}"#.to_vec())?,
                OAuthHttpRedirectPolicy::Follow,
                /*timeout*/ None,
            )
            .await
            .map_err(|error| anyhow::anyhow!(error))?;

        assert_eq!(
            response.status(),
            if has_resource_only_headers {
                oauth2::http::StatusCode::TEMPORARY_REDIRECT
            } else {
                oauth2::http::StatusCode::CREATED
            }
        );
        resource_server.verify().await;
        redirect_target.verify().await;
    }

    Ok(())
}

#[tokio::test]
async fn same_origin_redirects_preserve_timeout_and_response_body_limits() -> Result<()> {
    for oversized_redirect_body in [false, true] {
        let server = MockServer::start().await;
        let resource_url = format!("{}/mcp", server.uri());
        let redirect = ResponseTemplate::new(307).insert_header("location", "/register/");
        let redirect = if oversized_redirect_body {
            redirect.set_body_bytes(vec![0; MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES + 1])
        } else {
            redirect.set_delay(Duration::from_millis(/*millis*/ 400))
        };
        Mock::given(method("POST"))
            .and(path("/register"))
            .respond_with(redirect)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/register/"))
            .respond_with(
                ResponseTemplate::new(201).set_delay(Duration::from_millis(/*millis*/ 400)),
            )
            .expect(u64::from(!oversized_redirect_body))
            .mount(&server)
            .await;

        let adapter = OAuthHttpClientAdapter::new(
            Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            ))),
            build_default_headers(
                Some(HashMap::from([(
                    "X-Api-Key".to_string(),
                    "resource-api-key-secret".to_string(),
                )])),
                /*env_http_headers*/ None,
            )?,
            &resource_url,
        );
        let error = adapter
            .execute_request(
                oauth2::http::Request::builder()
                    .method("POST")
                    .uri(format!("{}/register", server.uri()))
                    .body(Vec::new())?,
                OAuthHttpRedirectPolicy::Follow,
                (!oversized_redirect_body).then_some(Duration::from_millis(/*millis*/ 700)),
            )
            .await
            .expect_err("redirects must preserve request timeout and response body limits");
        if oversized_redirect_body {
            assert!(error.to_string().contains("exceeds"));
        }
        server.verify().await;
    }

    Ok(())
}
