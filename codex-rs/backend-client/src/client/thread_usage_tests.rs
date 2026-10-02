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
            thread_ids: &["thread-123"],
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
                "native_usage_usd_micros": 3_250_000,
                "groups": [{
                    "model": "gpt-5.4",
                    "reasoning_effort": "high",
                    "speed": "fast",
                    "estimated_usage_credits_micros": 46_000_000,
                    "native_usage_usd_micros": 3_250_000,
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
            native_usage_usd_micros: Some(3_250_000),
            groups: vec![ThreadUsageBreakdownGroup {
                model: Some("gpt-5.4".to_string()),
                reasoning_effort: Some("high".to_string()),
                speed: Some("fast".to_string()),
                estimated_usage_credits_micros: 46_000_000,
                native_usage_usd_micros: Some(3_250_000),
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
            native_usage_usd_micros: None,
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
    assert!(error.to_string().contains("unexpected threads"));
}

#[tokio::test]
async fn batch_usage_rejects_invalid_requests_before_http() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(/*s*/ 500))
        .expect(/*r*/ 0)
        .mount(&server)
        .await;
    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    let too_many = (0..101)
        .map(|id| format!("thread-{id}"))
        .collect::<Vec<_>>();
    for ids in [
        Vec::new(),
        vec!["duplicate", "duplicate"],
        too_many.iter().map(String::as_str).collect(),
    ] {
        let error = client.get_threads_usage(&ids).await.unwrap_err();
        assert!(error.to_string().contains("1–100 distinct thread IDs"));
    }
}

#[tokio::test]
async fn batch_usage_rejects_duplicate_and_unrequested_response_rows() {
    for returned in [["first", "first"], ["first", "unexpected"]] {
        for second_amount in [Some(1), None] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(body_json(json!({"thread_ids": ["first", "second"]})))
                .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                    "threads": returned.iter().enumerate().map(|(index, thread_id)| json!({
                        "thread_id": thread_id,
                        "estimated_usage_credits_micros": if index == 0 { Some(1) } else { second_amount }
                    })).collect::<Vec<_>>()
                })))
                .expect(/*r*/ 1)
                .mount(&server)
                .await;
            let client = Client::new(
                server.uri(),
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            );
            let error = client
                .get_threads_usage(&["first", "second"])
                .await
                .unwrap_err();
            assert!(error.to_string().contains("unexpected threads"));
        }
    }
}

#[test]
fn thread_usage_preserves_unknown_and_zero_native_group_amounts() {
    let usage: ThreadUsage = serde_json::from_value(json!({
        "thread_id": "thread-123",
        "estimated_usage_credits_micros": 0,
        "native_usage_usd_micros": null,
        "groups": [
            { "model": "legacy", "estimated_usage_credits_micros": 0 },
            {
                "model": "unknown",
                "estimated_usage_credits_micros": 0,
                "native_usage_usd_micros": null
            },
            {
                "model": "zero",
                "estimated_usage_credits_micros": 0,
                "native_usage_usd_micros": 0
            }
        ]
    }))
    .expect("decode native groups from legacy and current backend responses");

    assert_eq!(
        usage,
        ThreadUsage {
            thread_id: "thread-123".to_string(),
            estimated_usage_credits_micros: 0,
            estimated_usage_usd_micros: None,
            native_usage_usd_micros: None,
            groups: [("legacy", None), ("unknown", None), ("zero", Some(0))]
                .into_iter()
                .map(
                    |(model, native_usage_usd_micros)| ThreadUsageBreakdownGroup {
                        model: Some(model.to_string()),
                        reasoning_effort: None,
                        speed: None,
                        estimated_usage_credits_micros: 0,
                        native_usage_usd_micros,
                        net_new_input_tokens: None,
                        cached_input_tokens: None,
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                    }
                )
                .collect(),
        }
    );
}

#[tokio::test]
async fn get_threads_usage_preserves_native_amounts_and_absence() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/codex/usage/thread_usage/query"))
        .and(body_json(json!({ "thread_ids": ["priced", "unknown"] })))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "threads": [
                {
                    "thread_id": "priced",
                    "estimated_usage_credits_micros": 46_000_000,
                    "estimated_usage_usd_micros": 2_505_000,
                    "native_usage_usd_micros": 3_250_000,
                    "groups": [
                        {
                            "model": "test-model-a",
                            "reasoning_effort": "medium",
                            "speed": "standard",
                            "estimated_usage_credits_micros": 12_000_000,
                            "native_usage_usd_micros": 750_000
                        },
                        {
                            "model": "test-model-b",
                            "reasoning_effort": "high",
                            "speed": "fast",
                            "estimated_usage_credits_micros": 34_000_000,
                            "native_usage_usd_micros": 2_500_000
                        }
                    ]
                },
                {
                    "thread_id": "unknown",
                    "estimated_usage_credits_micros": 7_000_000,
                    "groups": [{
                        "model": "legacy-model",
                        "estimated_usage_credits_micros": 7_000_000
                    }]
                }
            ]
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
            .get_threads_usage(&["priced", "unknown"])
            .await
            .expect("read native and legacy usage in one batch"),
        vec![
            ThreadUsage {
                thread_id: "priced".to_string(),
                estimated_usage_credits_micros: 46_000_000,
                estimated_usage_usd_micros: Some(2_505_000),
                native_usage_usd_micros: Some(3_250_000),
                groups: vec![
                    ThreadUsageBreakdownGroup {
                        model: Some("test-model-a".to_string()),
                        reasoning_effort: Some("medium".to_string()),
                        speed: Some("standard".to_string()),
                        estimated_usage_credits_micros: 12_000_000,
                        native_usage_usd_micros: Some(750_000),
                        net_new_input_tokens: None,
                        cached_input_tokens: None,
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                    },
                    ThreadUsageBreakdownGroup {
                        model: Some("test-model-b".to_string()),
                        reasoning_effort: Some("high".to_string()),
                        speed: Some("fast".to_string()),
                        estimated_usage_credits_micros: 34_000_000,
                        native_usage_usd_micros: Some(2_500_000),
                        net_new_input_tokens: None,
                        cached_input_tokens: None,
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                    },
                ],
            },
            ThreadUsage {
                thread_id: "unknown".to_string(),
                estimated_usage_credits_micros: 7_000_000,
                estimated_usage_usd_micros: None,
                native_usage_usd_micros: None,
                groups: vec![ThreadUsageBreakdownGroup {
                    model: Some("legacy-model".to_string()),
                    reasoning_effort: None,
                    speed: None,
                    estimated_usage_credits_micros: 7_000_000,
                    native_usage_usd_micros: None,
                    net_new_input_tokens: None,
                    cached_input_tokens: None,
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                }],
            },
        ]
    );
}
