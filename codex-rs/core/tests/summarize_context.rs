#![expect(clippy::unwrap_used)]

use codex_core::Codex;
use codex_core::CodexSpawnOk;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_login::CodexAuth;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// End‑to‑end: request → summarize → next request uses only the summary in history.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn summarize_context_three_requests_and_instructions() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Set up a mock server that we can inspect after the run.
    let server = MockServer::start().await;

    // SSE 1: normal assistant reply so it is recorded in history.
    let sse1 = {
        let ev1 = serde_json::json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "id": "m1",
                "content": [{"type": "output_text", "text": "FIRST_REPLY"}]
            }
        });
        let ev2 = serde_json::json!({
            "type": "response.completed",
            "response": {
                "id": "r1",
                "usage": {"input_tokens":0,"input_tokens_details":null,"output_tokens":0,"output_tokens_details":null,"total_tokens":0}
            }
        });
        let mut out = String::new();
        for ev in [ev1, ev2] {
            out.push_str(&format!(
                "event: {}\n",
                ev.get("type").unwrap().as_str().unwrap()
            ));
            out.push_str(&format!("data: {ev}\n\n"));
        }
        out
    };

    // SSE 2: summarizer returns a summary message.
    let summary_text = "SUMMARY_ONLY_CONTEXT";
    let sse2 = {
        let ev1 = serde_json::json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "id": "m2",
                "content": [{"type": "output_text", "text": summary_text}]
            }
        });
        let ev2 = serde_json::json!({
            "type": "response.completed",
            "response": {
                "id": "r2",
                "usage": {"input_tokens":0,"input_tokens_details":null,"output_tokens":0,"output_tokens_details":null,"total_tokens":0}
            }
        });
        let mut out = String::new();
        for ev in [ev1, ev2] {
            out.push_str(&format!(
                "event: {}\n",
                ev.get("type").unwrap().as_str().unwrap()
            ));
            out.push_str(&format!("data: {ev}\n\n"));
        }
        out
    };

    // SSE 3: can be minimal completed; we only need to capture the request body.
    let sse3 = {
        let ev = serde_json::json!({
            "type": "response.completed",
            "response": {
                "id": "r3",
                "usage": {"input_tokens":0,"input_tokens_details":null,"output_tokens":0,"output_tokens_details":null,"total_tokens":0}
            }
        });
        format!(
            "event: {}\ndata: {}\n\n",
            ev.get("type").unwrap().as_str().unwrap(),
            ev
        )
    };

    // Mount three expectations, one per request, matched by body content.
    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"hello world\"")
            && !body.contains("\"text\":\"Start Summarization\"")
    };
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(first_matcher)
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse1, "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"Start Summarization\"")
    };
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(second_matcher)
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse2, "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"next turn\"")
    };
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(third_matcher)
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse3, "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Build config pointing to the mock server.
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;

    // Spawn Codex with a dummy API key so auth header is set.
    let ctrl_c = std::sync::Arc::new(tokio::sync::Notify::new());
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        Some(CodexAuth::from_api_key("dummy".to_string())),
        ctrl_c.clone(),
    )
    .await
    .unwrap();

    // 1) Normal user input – should hit server once.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello world".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // 2) Summarize – second hit with summarization instructions.
    codex.submit(Op::SummarizeContext).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // 3) Next user input – third hit; history should include only the summary.
    let third_user_msg = "next turn";
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: third_user_msg.into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Inspect the three captured requests.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3, "expected exactly three requests");

    let req1 = &requests[0];
    let req2 = &requests[1];
    let req3 = &requests[2];

    let body1 = req1.body_json::<serde_json::Value>().unwrap();
    let body2 = req2.body_json::<serde_json::Value>().unwrap();
    let body3 = req3.body_json::<serde_json::Value>().unwrap();

    // System instructions should change for the summarization turn.
    let instr1 = body1.get("instructions").and_then(|v| v.as_str()).unwrap();
    let instr2 = body2.get("instructions").and_then(|v| v.as_str()).unwrap();
    assert_ne!(
        instr1, instr2,
        "summarization should override base instructions"
    );
    assert!(
        instr2.contains("You are a summarization assistant"),
        "summarization instructions not applied"
    );

    // The summarization request should include the injected user input marker.
    let input2 = body2.get("input").and_then(|v| v.as_array()).unwrap();
    // The last item is the user message created from the injected input.
    let last2 = input2.last().unwrap();
    assert_eq!(last2.get("type").unwrap().as_str().unwrap(), "message");
    assert_eq!(last2.get("role").unwrap().as_str().unwrap(), "user");
    let text2 = last2["content"][0]["text"].as_str().unwrap();
    assert!(text2.contains("Start Summarization"));

    // Third request must contain only the summary from step 2 as prior history plus new user msg.
    let input3 = body3.get("input").and_then(|v| v.as_array()).unwrap();
    println!("third request body: {body3}");
    assert!(
        input3.len() >= 2,
        "expected summary + new user message in third request"
    );

    // Helper: collect all (role, text) message tuples.
    let mut messages: Vec<(String, String)> = Vec::new();
    for item in input3 {
        if item["type"].as_str() == Some("message") {
            let role = item["role"].as_str().unwrap_or_default().to_string();
            let text = item["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            messages.push((role, text));
        }
    }

    // Exactly one assistant message should remain after compaction.
    let assistant_count = messages.iter().filter(|(r, _)| r == "assistant").count();
    assert_eq!(
        assistant_count, 1,
        "exactly one assistant message should remain after compaction"
    );
    assert!(
        messages
            .iter()
            .any(|(r, t)| r == "user" && t == third_user_msg),
        "third request should include the new user message"
    );
    // The pre-compaction user prompt and summarize trigger should not be present anymore.
    assert!(
        !messages.iter().any(|(_, t)| t.contains("hello world")),
        "third request should not include the original user input"
    );
    assert!(
        !messages
            .iter()
            .any(|(_, t)| t.contains("Start Summarization")),
        "third request should not include the summarize trigger"
    );
}
