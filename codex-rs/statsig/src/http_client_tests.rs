#![cfg(test)]

use std::time::Duration;

use bytes::Bytes;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyController;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RouteAwareClientPool;
use http::Request;
use http::StatusCode;
use pretty_assertions::assert_eq;
use statsig_client::HttpClient;
use statsig_client::NetworkOptions;
use statsig_client::StatsigError;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_bytes;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

use super::StatsigHttpClient;

#[tokio::test]
async fn preserves_http_errors_and_bounds_decoded_responses() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let client = StatsigHttpClient::new(RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
    ));
    let request_body = br#"{"user":{"userID":"test-user"}}"#;
    let expected_body = vec![b'x'; 128];
    let size = expected_body.len();
    Mock::given(method("POST"))
        .and(path("/initialize"))
        .and(query_param("k", "client-key"))
        .and(header("content-type", "application/json"))
        .and(header("accept-encoding", "identity"))
        .and(body_bytes(request_body.as_slice()))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_bytes(expected_body.clone())
                .insert_header("x-statsig-region", "us"),
        )
        .expect(2)
        .mount(&server)
        .await;
    let request = Request::post(format!("{}/initialize?k=client-key", server.uri()))
        .header("content-type", "application/json")
        .body(Bytes::from_static(request_body))?;
    let send = |limit| {
        client.send(
            request.clone(),
            NetworkOptions {
                response_body_byte_limit: Some(limit),
                ..Default::default()
            },
        )
    };
    let response = send(size).await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["x-statsig-region"], "us");
    assert_eq!(response.body().as_ref(), expected_body.as_slice());
    assert!(matches!(
        send(size - 1).await,
        Err(StatsigError::NetworkResponseTooLarge { limit, actual })
            if limit == size - 1 && actual == size
    ));
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn honors_the_supplied_network_policy() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let controller = NetworkPolicyController::default();
    let client = StatsigHttpClient::new(RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(controller.policy()),
        ClientRouteClass::Api,
    ));
    let request = Request::get(format!("{}/?key=secret", server.uri())).body(Bytes::new())?;
    let error = client
        .send(request, NetworkOptions::default())
        .await
        .unwrap_err();
    assert!(matches!(error, StatsigError::Network(_)));
    assert!(!error.to_string().contains("secret"));
    assert!(server.received_requests().await.unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn preserves_timeout_errors() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(/* secs */ 1)))
        .mount(&server)
        .await;
    let client = StatsigHttpClient::new(RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
    ));
    let timeout = Duration::from_millis(/* millis */ 25);
    let request = Request::get(server.uri()).body(Bytes::new())?;
    let result = client
        .send(
            request,
            NetworkOptions {
                timeout,
                ..Default::default()
            },
        )
        .await;
    assert!(matches!(result, Err(StatsigError::NetworkTimeout(actual)) if actual == timeout));
    Ok(())
}
