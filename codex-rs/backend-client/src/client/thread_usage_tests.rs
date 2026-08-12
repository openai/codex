use super::*;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test]
fn thread_usage_contract_uses_expected_paths_and_payload() {
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    assert_eq!(
        Client::new("https://example.test", factory.clone()).thread_usage_url(),
        "https://example.test/api/codex/usage/thread_usage/query"
    );
    assert_eq!(
        Client::new("https://chatgpt.com/backend-api", factory).thread_usage_url(),
        "https://chatgpt.com/backend-api/wham/usage/thread_usage/query"
    );
    assert_eq!(
        serde_json::to_value(ThreadUsageQueryRequest {
            thread_ids: ["thread-123"],
        })
        .expect("serialize thread usage request"),
        json!({ "thread_ids": ["thread-123"] })
    );
}

#[tokio::test]
async fn get_thread_usage_returns_requested_thread_totals() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/codex/usage/thread_usage/query"))
        .and(body_json(json!({ "thread_ids": ["thread-123"] })))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "threads": [{
                "thread_id": "thread-123",
                "estimated_usage_credits_micros": 46_000_000,
                "estimated_usage_usd_micros": 1_820_000,
                "groups": [{
                    "model": "gpt-5.4",
                    "reasoning_effort": "high",
                    "speed": "fast",
                    "estimated_usage_credits_micros": 46_000_000,
                    "net_new_input_tokens": 80,
                    "cached_input_tokens": 20,
                    "input_tokens": 100,
                    "output_tokens": 40,
                    "total_tokens": 140
                }]
            }]
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;

    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    assert_eq!(
        client
            .get_thread_usage("thread-123")
            .await
            .expect("read thread usage"),
        ThreadUsage {
            thread_id: "thread-123".to_string(),
            estimated_usage_credits_micros: 46_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: vec![ThreadUsageBreakdownGroup {
                model: Some("gpt-5.4".to_string()),
                reasoning_effort: Some("high".to_string()),
                speed: Some("fast".to_string()),
                estimated_usage_credits_micros: 46_000_000,
                net_new_input_tokens: Some(80),
                cached_input_tokens: Some(20),
                input_tokens: Some(100),
                output_tokens: Some(40),
                total_tokens: Some(140),
            }],
        }
    );
}

#[tokio::test]
async fn get_thread_usage_accepts_credits_without_usd_estimate() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/codex/usage/thread_usage/query"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "threads": [{
                "thread_id": "thread-123",
                "estimated_usage_credits_micros": 46_000_000,
                "estimated_usage_usd_micros": null
            }]
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;

    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    assert_eq!(
        client
            .get_thread_usage("thread-123")
            .await
            .expect("read credits without a dollar estimate"),
        ThreadUsage {
            thread_id: "thread-123".to_string(),
            estimated_usage_credits_micros: 46_000_000,
            estimated_usage_usd_micros: None,
            groups: Vec::new(),
        }
    );
}

#[tokio::test]
async fn get_thread_usage_rejects_totals_for_another_thread() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "threads": [{
                "thread_id": "another-thread",
                "estimated_usage_credits_micros": 1,
                "estimated_usage_usd_micros": 1
            }]
        })))
        .mount(&server)
        .await;

    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    let error = client
        .get_thread_usage("thread-123")
        .await
        .expect_err("reject usage for a different thread");
    assert!(error.to_string().contains("requested thread thread-123"));
}
