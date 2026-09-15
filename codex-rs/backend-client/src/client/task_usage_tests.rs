//! Exact decimal preservation and sorting for consumer task credits.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn credits_preserve_precision_and_order_signed_decimal_strings() {
    let mut amounts: Vec<TaskCredits> = serde_json::from_value(json!([
        "1.000000000000000002",
        "-0.00025",
        "0",
        "1.000000000000000001",
        "1e-20",
        "-1E+3"
    ]))
    .unwrap();
    amounts.sort();
    assert_eq!(
        amounts.iter().map(TaskCredits::as_str).collect::<Vec<_>>(),
        vec![
            "-1E+3",
            "-0.00025",
            "0",
            "1e-20",
            "1.000000000000000001",
            "1.000000000000000002"
        ]
    );
    for invalid in ["NaN", "inf", "1..0", "", "1e999999999999"] {
        assert!(serde_json::from_value::<TaskCredits>(json!(invalid)).is_err());
    }
}

#[tokio::test]
async fn task_queries_validate_request_groups_and_response_ownership() {
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    let server = MockServer::start().await;
    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    let query = TaskUsageThread {
        thread_id: "root".into(),
        created_at: None,
        descendant_thread_ids: vec!["child".into()],
    };
    let task = json!({"thread_id":"root", "data_status":"available", "usage_source":"plan_and_credits",
    "five_hour_limit_percent":150.0, "weekly_limit_percent":0.0,
    "balance_usage_credits":"-0.000000000000000001", "groups":[{
        "product_experience":"codex", "model":"gpt-5.5", "reasoning_effort":"high", "speed":"standard",
        "five_hour_limit_percent":150.25, "weekly_limit_percent":0.125, "balance_usage_credits":"0"
    }]});
    for ids in [vec!["root"], vec!["other"], vec!["root", "root"]] {
        server.reset().await;
        let response = json!({"data_as_of":null, "threads":ids.iter().map(|id| {
            let mut task=task.clone();task["thread_id"]=json!(id);task
        }).collect::<Vec<_>>()});
        Mock::given(method("POST"))
            .and(path("/api/codex/usage/thread_usage/query_v2"))
            .respond_with(move |request: &wiremock::Request| {
                assert_eq!(
                    request.body_json::<serde_json::Value>().unwrap(),
                    json!({"threads":[{
                        "thread_id":"root", "created_at":null, "descendant_thread_ids":["child"]
                    }]})
                );
                ResponseTemplate::new(/*s*/ 200).set_body_json(&response)
            })
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let result = client.get_task_usage(std::slice::from_ref(&query)).await;
        if ids == ["root"] {
            let response = result.unwrap();
            assert_eq!((response.data_as_of, response.threads.len()), (None, 1));
            let row = &response.threads[0];
            assert_eq!(
                (
                    row.thread_id.as_str(),
                    row.data_status,
                    row.amounts.five_hour_limit_percent,
                    row.amounts.weekly_limit_percent,
                    row.amounts
                        .balance_usage_credits
                        .as_ref()
                        .map(TaskCredits::as_str)
                ),
                (
                    "root",
                    TaskUsageStatus::Available,
                    Some(150.0),
                    Some(0.0),
                    Some("-0.000000000000000001")
                )
            );
            assert_eq!(
                (
                    row.groups[0].amounts.five_hour_limit_percent,
                    row.groups[0].amounts.weekly_limit_percent
                ),
                (Some(150.25), Some(0.125))
            );
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("Invalid task usage response.")
            );
        }
    }
    server.reset().await;
    for queries in [
        vec![],
        vec![query.clone(), query.clone()],
        vec![TaskUsageThread {
            thread_id: "root".into(),
            created_at: None,
            descendant_thread_ids: (0..1_000).map(|n| format!("child-{n}")).collect(),
        }],
    ] {
        assert!(client.get_task_usage(&queries).await.is_err());
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}
