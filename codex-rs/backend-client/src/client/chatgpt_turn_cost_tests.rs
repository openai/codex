//! Exercises both authenticated SiWC routes and preserves nullable/omitted estimate fields.

use super::*;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::CodexAuth;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn chatgpt_turn_costs_use_workspace_auth_and_chatgpt_path() {
    check_query(
        "/backend-api",
        "/backend-api/wham/usage/thread-estimates/query",
    )
    .await;
}

#[tokio::test]
async fn chatgpt_turn_costs_use_workspace_auth_and_codex_path() {
    check_query("", "/api/codex/usage/thread-estimates/query").await;
}

async fn check_query(base_path: &str, endpoint: &str) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(endpoint))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(header("content-type", "application/json"))
        .and(body_json(serde_json::json!({
            "threads": [
                {"thread_id": "thread-a", "turn_ids": ["turn-a"]},
                {"thread_id": "thread-b", "turn_ids": ["turn-b", "turn-c"]}
            ],
            "include_settled_response_ids": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "threads": [
                {"thread_id": "thread-a", "turns": [{
                    "turn_id": "turn-a", "model": "gpt-5.6",
                    "estimated_usage_usd_micros": 1250001, "native_usage_usd_micros": 750000,
                    "settled_response_ids": ["resp-a", "resp-b"]
                }]},
                {"thread_id": "thread-b", "turns": [
                    {"turn_id": "turn-b", "estimated_usage_usd_micros": null, "native_usage_usd_micros": null,
                     "settled_response_ids": null},
                    {"turn_id": "turn-c", "estimated_usage_usd_micros": 0}
                ]}
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = Client::from_auth(
        format!("{}{base_path}", server.uri()),
        &CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    let result = client
        .query_chatgpt_turn_costs(&BTreeMap::from([
            ("thread-a".to_string(), vec!["turn-a".to_string()]),
            (
                "thread-b".to_string(),
                vec!["turn-b".to_string(), "turn-c".to_string()],
            ),
        ]))
        .await
        .expect("ChatGPT turn costs");
    assert_eq!(
        result,
        vec![
            ChatgptThreadTurnCosts {
                thread_id: "thread-a".to_string(),
                turns: vec![ChatgptTurnCost {
                    turn_id: "turn-a".to_string(),
                    model: Some("gpt-5.6".to_string()),
                    estimated_usage_usd_micros: Some(1250001),
                    native_usage_usd_micros: Some(750000),
                    settled_response_ids: Some(vec!["resp-a".to_string(), "resp-b".to_string()]),
                }],
            },
            ChatgptThreadTurnCosts {
                thread_id: "thread-b".to_string(),
                turns: vec![
                    ChatgptTurnCost {
                        turn_id: "turn-b".to_string(),
                        model: None,
                        estimated_usage_usd_micros: None,
                        native_usage_usd_micros: None,
                        settled_response_ids: None,
                    },
                    ChatgptTurnCost {
                        turn_id: "turn-c".to_string(),
                        model: None,
                        estimated_usage_usd_micros: Some(0),
                        native_usage_usd_micros: Some(0),
                        settled_response_ids: None,
                    },
                ],
            },
        ]
    );
    server.verify().await;
}

#[test]
fn visible_turn_cost_requires_both_dollar_components() {
    for (converted, native, expected) in [
        (Some(0), Some(4_668), Some(4_668)),
        (Some(120), Some(35), Some(155)),
        (Some(120), Some(0), Some(120)),
        (Some(0), Some(0), Some(0)),
        (None, Some(35), None),
        (Some(120), None, None),
        (Some(-1), Some(35), None),
        (Some(120), Some(-1), None),
        (Some(i64::MAX), Some(1), None),
    ] {
        let cost: ChatgptTurnCost = serde_json::from_value(serde_json::json!({
            "turn_id": "turn-a",
            "estimated_usage_usd_micros": converted,
            "native_usage_usd_micros": native,
        }))
        .expect("valid turn cost response");
        assert_eq!(
            cost.visible_total_usd_micros(),
            expected,
            "converted={converted:?}, native={native:?}"
        );
    }
}

#[test]
fn settled_turn_cost_distinguishes_omitted_native_usd_from_null() {
    let mut turn = serde_json::json!({
        "turn_id": "turn-a",
        "estimated_usage_usd_micros": 120,
        "settled_response_ids": ["resp-a"],
    });
    let legacy: ChatgptTurnCost = serde_json::from_value(turn.clone()).unwrap();
    turn["native_usage_usd_micros"] = serde_json::Value::Null;
    let unavailable: ChatgptTurnCost = serde_json::from_value(turn).unwrap();

    assert_eq!(legacy.visible_total_usd_micros(), Some(120));
    assert_eq!(unavailable.visible_total_usd_micros(), None);
}
