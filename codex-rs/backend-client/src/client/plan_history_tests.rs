//! Plan history's undeployed route and wire defaults remain distinct from failures.
use super::*;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

#[tokio::test]
async fn plan_history_handles_unavailable_errors_and_approximation_default() {
    let server = MockServer::start().await;
    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    for status in [404, 503, 200] {
        server.reset().await;
        Mock::given(method("GET"))
            .and(path("/api/codex/usage/plan_limit_history"))
            .and(query_param("days", "7"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({
                "data_as_of": null, "coverage_start": null, "coverage_complete": false,
                "boundary_tolerance_seconds": null, "periods": []
            })))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let response = client.get_plan_limit_history().await;
        match status {
            404 => assert_eq!(response.unwrap(), None),
            503 => assert_eq!(response.unwrap_err().status().unwrap().as_u16(), status),
            200 => assert_eq!(
                response.unwrap(),
                Some(PlanLimitHistory {
                    data_as_of: None,
                    coverage_start: None,
                    coverage_complete: false,
                    approximate: true,
                    boundary_tolerance_seconds: None,
                    periods: vec![],
                })
            ),
            _ => unreachable!(),
        }
    }
}
