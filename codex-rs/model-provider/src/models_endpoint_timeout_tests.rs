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
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "models": [] }))
                .set_delay(MODELS_REFRESH_TIMEOUT * 2),
        )
        .expect(1)
        .mount(&server)
        .await;
    let endpoint = OpenAiModelsEndpoint::new(
        ModelProviderInfo::create_openai_provider(Some(server.uri())),
        /*auth_manager*/ None,
        /*gateway_auth_manager*/ None,
    );

    let error = endpoint
        .list_models(
            "0.0.0",
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await
        .expect_err("delayed catalog request should time out");

    assert!(
        matches!(error.details(), CodexErrorDetails::RequestTimeout),
        "{error}"
    );
}
