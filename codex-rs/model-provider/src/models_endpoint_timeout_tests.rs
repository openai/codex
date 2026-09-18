use super::*;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::error::CodexErrorDetails;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn catalog_deadline_returns_request_timeout() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "models": [] }))
                .set_delay(Duration::from_secs(/*secs*/ 60)),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let endpoint = OpenAiModelsEndpoint::new(
        ModelProviderInfo::create_openai_provider(Some(server.uri())),
        /*auth_manager*/ None,
        /*gateway_auth_manager*/ None,
    );

    tokio::time::pause();
    let request = tokio::time::timeout(
        MODELS_REFRESH_TIMEOUT * 2,
        endpoint.list_models(
            "0.0.0",
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ),
    );
    tokio::pin!(request);
    let received = mock.wait_until_satisfied();
    tokio::pin!(received);
    let deadline = std::time::Instant::now() + Duration::from_secs(/*secs*/ 30);
    // Keep the paused runtime runnable until the request reaches the server.
    // Otherwise auto-advance can expire the catalog deadline during dispatch.
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "server did not receive the catalog request"
        );
        tokio::select! {
            () = &mut received => break,
            result = &mut request => {
                panic!("catalog request finished before reaching the server: {result:?}");
            }
            () = tokio::task::yield_now() => {}
        }
    }
    tokio::time::advance(MODELS_REFRESH_TIMEOUT + Duration::from_millis(/*millis*/ 1)).await;
    let error = request
        .await
        .expect("catalog deadline should expire")
        .expect_err("delayed catalog request should time out");
    tokio::time::resume();

    assert!(
        matches!(error.details(), CodexErrorDetails::RequestTimeout),
        "{error}"
    );
}
