use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::OAuthHttpRedirectPolicy;
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
