#![expect(clippy::unwrap_used)]

use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use pretty_assertions::assert_eq;

// --- Test helpers -----------------------------------------------------------

/// Build an SSE stream body from a list of JSON events.
fn sse(events: Vec<Value>) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for ev in events {
        let kind = ev.get("type").and_then(|v| v.as_str()).unwrap();
        writeln!(&mut out, "event: {kind}").unwrap();
        if !ev.as_object().map(|o| o.len() == 1).unwrap_or(false) {
            write!(&mut out, "data: {ev}\n\n").unwrap();
        } else {
            out.push('\n');
        }
    }
    out
}

/// Convenience: SSE event for a completed response with a specific id.
fn ev_completed(id: &str) -> Value {
    serde_json::json!({
        "type": "response.completed",
        "response": {
            "id": id,
            "usage": {"input_tokens":0,"input_tokens_details":null,"output_tokens":0,"output_tokens_details":null,"total_tokens":0}
        }
    })
}

/// Convenience: SSE event for a single assistant message output item.
fn ev_assistant_message(id: &str, text: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": id,
            "content": [{"type": "output_text", "text": text}]
        }
    })
}

fn sse_response(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(body, "text/event-stream")
}

async fn mount_sse_once<M>(server: &MockServer, matcher: M, body: String)
where
    M: wiremock::Match + Send + Sync + 'static,
{
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(matcher)
        .respond_with(sse_response(body))
        .expect(1)
        .mount(server)
        .await;
}

const FIRST_REPLY: &str = "FIRST_REPLY";
const SUMMARY_TEXT: &str = "SUMMARY_ONLY_CONTEXT";
const SUMMARIZE_TRIGGER: &str = "Start Summarization";
const THIRD_USER_MSG: &str = "next turn";

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

    // SSE 1: assistant replies normally so it is recorded in history.
    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);

    // SSE 2: summarizer returns a summary message.
    let sse2 = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed("r2"),
    ]);

    // SSE 3: minimal completed; we only need to capture the request body.
    let sse3 = sse(vec![ev_completed("r3")]);

    // Mount three expectations, one per request, matched by body content.
    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"hello world\"")
            && !body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\""))
    };
    mount_sse_once(&server, first_matcher, sse1).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\""))
    };
    mount_sse_once(&server, second_matcher, sse2).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{THIRD_USER_MSG}\""))
    };
    mount_sse_once(&server, third_matcher, sse3).await;

    // Build config pointing to the mock server and spawn Codex.
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: codex,
        session_configured,
        ..
    } = conversation_manager.new_conversation(config).await.unwrap();
    let rollout_path = session_configured.rollout_path;

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
    codex.submit(Op::Compact).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // 3) Next user input – third hit; history should include only the summary.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: THIRD_USER_MSG.into(),
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
    assert!(text2.contains(SUMMARIZE_TRIGGER));

    // Third request must contain only the summary from step 2 as prior history plus new user msg.
    let input3 = body3.get("input").and_then(|v| v.as_array()).unwrap();
    println!("third request body: {body3}");
    assert!(
        input3.len() >= 2,
        "expected summary + new user message in third request"
    );

    // Collect all (role, text) message tuples.
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

    // Exactly one assistant message should remain after compaction and the new user message is present.
    let assistant_count = messages.iter().filter(|(r, _)| r == "assistant").count();
    assert_eq!(
        assistant_count, 1,
        "exactly one assistant message should remain after compaction"
    );
    assert!(
        messages
            .iter()
            .any(|(r, t)| r == "user" && t == THIRD_USER_MSG),
        "third request should include the new user message"
    );
    assert!(
        !messages.iter().any(|(_, t)| t.contains("hello world")),
        "third request should not include the original user input"
    );
    assert!(
        !messages.iter().any(|(_, t)| t.contains(SUMMARIZE_TRIGGER)),
        "third request should not include the summarize trigger"
    );

    // Shut down Codex to flush rollout entries before inspecting the file.
    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    // Verify rollout contains APITurn entries for each API call and a Compacted entry.
    let text = std::fs::read_to_string(&rollout_path).unwrap_or_else(|e| {
        panic!(
            "failed to read rollout file {}: {e}",
            rollout_path.display()
        )
    });
    let mut api_turn_count = 0usize;
    let mut saw_compacted_summary = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry): Result<RolloutLine, _> = serde_json::from_str(trimmed) else {
            continue;
        };
        match entry.item {
            RolloutItem::TurnContext(_) => {
                api_turn_count += 1;
            }
            RolloutItem::Compacted(ci) => {
                if ci.message == SUMMARY_TEXT {
                    saw_compacted_summary = true;
                }
            }
            _ => {}
        }
    }

    assert!(
        api_turn_count == 3,
        "expected three APITurn entries in rollout"
    );
    assert!(
        saw_compacted_summary,
        "expected a Compacted entry containing the summarizer output"
    );
}
