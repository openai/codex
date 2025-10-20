use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::protocol::ErrorEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use core_test_support::load_default_config_for_test;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use tempfile::TempDir;

use codex_core::codex::compact::SUMMARIZATION_PROMPT;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_failed;
use core_test_support::responses::start_mock_server;
use pretty_assertions::assert_eq;
// --- Test helpers -----------------------------------------------------------

pub(super) const FIRST_REPLY: &str = "FIRST_REPLY";
pub(super) const SUMMARY_TEXT: &str = "SUMMARY_ONLY_CONTEXT";
const THIRD_USER_MSG: &str = "next turn";
const AUTO_SUMMARY_TEXT: &str = "AUTO_SUMMARY";
const FIRST_AUTO_MSG: &str = "token limit start";
const SECOND_AUTO_MSG: &str = "token limit push";
const STILL_TOO_BIG_REPLY: &str = "STILL_TOO_BIG";
const MULTI_AUTO_MSG: &str = "multi auto";
const SECOND_LARGE_REPLY: &str = "SECOND_LARGE_REPLY";
const FIRST_AUTO_SUMMARY: &str = "FIRST_AUTO_SUMMARY";
const SECOND_AUTO_SUMMARY: &str = "SECOND_AUTO_SUMMARY";
const FINAL_REPLY: &str = "FINAL_REPLY";
const CONTEXT_LIMIT_MESSAGE: &str =
    "Your input exceeds the context window of this model. Please adjust your input and try again.";
const DUMMY_FUNCTION_NAME: &str = "unsupported_tool";
const DUMMY_CALL_ID: &str = "call-multi-auto";
const FUNCTION_CALL_LIMIT_MSG: &str = "function call limit push";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn summarize_context_three_requests_and_instructions() {
    skip_if_no_network!();

    // Set up a mock server that we can inspect after the run.
    let server = start_mock_server().await;

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
            && !body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, first_matcher, sse1).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, second_matcher, sse2).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{THIRD_USER_MSG}\""))
    };
    mount_sse_once_match(&server, third_matcher, sse3).await;

    // Build config pointing to the mock server and spawn Codex.
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    config.model_auto_compact_token_limit = Some(200_000);
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

    // 2) Summarize – second hit should include the summarization prompt.
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

    // Manual compact should keep the baseline developer instructions.
    let instr1 = body1.get("instructions").and_then(|v| v.as_str()).unwrap();
    let instr2 = body2.get("instructions").and_then(|v| v.as_str()).unwrap();
    assert_eq!(
        instr1, instr2,
        "manual compact should keep the standard developer instructions"
    );

    // The summarization request should include the injected user input marker.
    let input2 = body2.get("input").and_then(|v| v.as_array()).unwrap();
    // The last item is the user message created from the injected input.
    let last2 = input2.last().unwrap();
    assert_eq!(last2.get("type").unwrap().as_str().unwrap(), "message");
    assert_eq!(last2.get("role").unwrap().as_str().unwrap(), "user");
    let text2 = last2["content"][0]["text"].as_str().unwrap();
    assert_eq!(
        text2, SUMMARIZATION_PROMPT,
        "expected summarize trigger, got `{text2}`"
    );

    // Third request must contain the refreshed instructions, bridge summary message and new user msg.
    let input3 = body3.get("input").and_then(|v| v.as_array()).unwrap();

    assert!(
        input3.len() >= 3,
        "expected refreshed context and new user message in third request"
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

    // No previous assistant messages should remain and the new user message is present.
    let assistant_count = messages.iter().filter(|(r, _)| r == "assistant").count();
    assert_eq!(assistant_count, 0, "assistant history should be cleared");
    assert!(
        messages
            .iter()
            .any(|(r, t)| r == "user" && t == THIRD_USER_MSG),
        "third request should include the new user message"
    );
    let Some((_, bridge_text)) = messages.iter().find(|(role, text)| {
        role == "user"
            && (text.contains("Here were the user messages")
                || text.contains("Here are all the user messages"))
            && text.contains(SUMMARY_TEXT)
    }) else {
        panic!("expected a bridge message containing the summary");
    };
    assert!(
        bridge_text.contains("hello world"),
        "bridge should capture earlier user messages"
    );
    assert!(
        !bridge_text.contains(SUMMARIZATION_PROMPT),
        "bridge text should not echo the summarize trigger"
    );
    assert!(
        !messages
            .iter()
            .any(|(_, text)| text.contains(SUMMARIZATION_PROMPT)),
        "third request should not include the summarize trigger"
    );

    // Shut down Codex to flush rollout entries before inspecting the file.
    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    // Verify rollout contains APITurn entries for each API call and a Compacted entry.
    println!("rollout path: {}", rollout_path.display());
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

// Windows CI only: bump to 4 workers to prevent SSE/event starvation and test timeouts.
#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn auto_compact_runs_after_token_limit_hit() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 70_000),
    ]);

    let sse2 = sse(vec![
        ev_assistant_message("m2", "SECOND_REPLY"),
        ev_completed_with_tokens("r2", 330_000),
    ]);

    let sse3 = sse(vec![
        ev_assistant_message("m3", AUTO_SUMMARY_TEXT),
        ev_completed_with_tokens("r3", 200),
    ]);

    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(FIRST_AUTO_MSG)
            && !body.contains(SECOND_AUTO_MSG)
            && !body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, first_matcher, sse1).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(SECOND_AUTO_MSG)
            && body.contains(FIRST_AUTO_MSG)
            && !body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, second_matcher, sse2).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, third_matcher, sse3).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    config.model_auto_compact_token_limit = Some(200_000);
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: FIRST_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: SECOND_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
    // wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert!(
        requests.len() >= 3,
        "auto compact should add at least a third request, got {}",
        requests.len()
    );
    let is_auto_compact = |req: &wiremock::Request| {
        std::str::from_utf8(&req.body)
            .unwrap_or("")
            .contains("You have exceeded the maximum number of tokens")
    };
    let auto_compact_count = requests.iter().filter(|req| is_auto_compact(req)).count();
    assert_eq!(
        auto_compact_count, 1,
        "expected exactly one auto compact request"
    );
    let auto_compact_index = requests
        .iter()
        .enumerate()
        .find_map(|(idx, req)| is_auto_compact(req).then_some(idx))
        .expect("auto compact request missing");
    assert_eq!(
        auto_compact_index, 2,
        "auto compact should add a third request"
    );

    let body_first = requests[0].body_json::<serde_json::Value>().unwrap();
    let body3 = requests[auto_compact_index]
        .body_json::<serde_json::Value>()
        .unwrap();
    let instructions = body3
        .get("instructions")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let baseline_instructions = body_first
        .get("instructions")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        instructions, baseline_instructions,
        "auto compact should keep the standard developer instructions",
    );

    let input3 = body3.get("input").and_then(|v| v.as_array()).unwrap();
    let last3 = input3
        .last()
        .expect("auto compact request should append a user message");
    assert_eq!(last3.get("type").and_then(|v| v.as_str()), Some("message"));
    assert_eq!(last3.get("role").and_then(|v| v.as_str()), Some("user"));
    let last_text = last3
        .get("content")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or_default();
    assert_eq!(
        last_text, SUMMARIZATION_PROMPT,
        "auto compact should send the summarization prompt as a user message",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_persists_rollout_entries() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 70_000),
    ]);

    let sse2 = sse(vec![
        ev_assistant_message("m2", "SECOND_REPLY"),
        ev_completed_with_tokens("r2", 330_000),
    ]);

    let sse3 = sse(vec![
        ev_assistant_message("m3", AUTO_SUMMARY_TEXT),
        ev_completed_with_tokens("r3", 200),
    ]);

    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(FIRST_AUTO_MSG)
            && !body.contains(SECOND_AUTO_MSG)
            && !body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, first_matcher, sse1).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(SECOND_AUTO_MSG)
            && body.contains(FIRST_AUTO_MSG)
            && !body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, second_matcher, sse2).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, third_matcher, sse3).await;

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

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: FIRST_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: SECOND_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    let rollout_path = session_configured.rollout_path;
    let text = std::fs::read_to_string(&rollout_path).unwrap_or_else(|e| {
        panic!(
            "failed to read rollout file {}: {e}",
            rollout_path.display()
        )
    });

    let mut turn_context_count = 0usize;
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
                turn_context_count += 1;
            }
            RolloutItem::Compacted(_) => {}
            _ => {}
        }
    }

    assert!(
        turn_context_count >= 2,
        "expected at least two turn context entries, got {turn_context_count}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_stops_after_failed_attempt() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 500),
    ]);

    let sse2 = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed_with_tokens("r2", 50),
    ]);

    let sse3 = sse(vec![
        ev_assistant_message("m3", STILL_TOO_BIG_REPLY),
        ev_completed_with_tokens("r3", 500),
    ]);

    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(FIRST_AUTO_MSG)
            && !body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, first_matcher, sse1.clone()).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(&server, second_matcher, sse2.clone()).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        !body.contains("You have exceeded the maximum number of tokens")
            && body.contains(SUMMARY_TEXT)
    };
    mount_sse_once_match(&server, third_matcher, sse3.clone()).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    config.model_auto_compact_token_limit = Some(200);
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: FIRST_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();

    let error_event = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Error(_))).await;
    let EventMsg::Error(ErrorEvent { message }) = error_event else {
        panic!("expected error event");
    };
    assert!(
        message.contains("limit"),
        "error message should include limit information: {message}"
    );
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        3,
        "auto compact should attempt at most one summarization before erroring"
    );

    let last_body = requests[2].body_json::<serde_json::Value>().unwrap();
    let input = last_body
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("unexpected request format: {last_body}"));
    let contains_prompt = input.iter().any(|item| {
        item.get("type").and_then(|v| v.as_str()) == Some("message")
            && item.get("role").and_then(|v| v.as_str()) == Some("user")
            && item
                .get("content")
                .and_then(|v| v.as_array())
                .and_then(|items| items.first())
                .and_then(|entry| entry.get("text"))
                .and_then(|text| text.as_str())
                .map(|text| text == SUMMARIZATION_PROMPT)
                .unwrap_or(false)
    });
    assert!(
        !contains_prompt,
        "third request should be the follow-up turn, not another summarization",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compact_retries_after_context_window_error() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let user_turn = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    let compact_failed = sse_failed(
        "resp-fail",
        "context_length_exceeded",
        CONTEXT_LIMIT_MESSAGE,
    );
    let compact_succeeds = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed("r2"),
    ]);

    let request_log = mount_sse_sequence(
        &server,
        vec![
            user_turn.clone(),
            compact_failed.clone(),
            compact_succeeds.clone(),
        ],
    )
    .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    config.model_auto_compact_token_limit = Some(200_000);
    let codex = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"))
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "first turn".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex.submit(Op::Compact).await.unwrap();

    let EventMsg::BackgroundEvent(event) =
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::BackgroundEvent(_))).await
    else {
        panic!("expected background event after compact retry");
    };
    assert!(
        event.message.contains("Trimmed 1 older conversation item"),
        "background event should mention trimmed item count: {}",
        event.message
    );
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(
        requests.len(),
        3,
        "expected user turn and two compact attempts"
    );

    let compact_attempt = requests[1].body_json();
    let retry_attempt = requests[2].body_json();

    let compact_input = compact_attempt["input"]
        .as_array()
        .unwrap_or_else(|| panic!("compact attempt missing input array: {compact_attempt}"));
    let retry_input = retry_attempt["input"]
        .as_array()
        .unwrap_or_else(|| panic!("retry attempt missing input array: {retry_attempt}"));
    assert_eq!(
        compact_input
            .last()
            .and_then(|item| item.get("content"))
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
            .and_then(|entry| entry.get("text"))
            .and_then(|text| text.as_str()),
        Some(SUMMARIZATION_PROMPT),
        "compact attempt should include summarization prompt"
    );
    assert_eq!(
        retry_input
            .last()
            .and_then(|item| item.get("content"))
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
            .and_then(|entry| entry.get("text"))
            .and_then(|text| text.as_str()),
        Some(SUMMARIZATION_PROMPT),
        "retry attempt should include summarization prompt"
    );
    assert_eq!(
        retry_input.len(),
        compact_input.len().saturating_sub(1),
        "retry should drop exactly one history item (before {} vs after {})",
        compact_input.len(),
        retry_input.len()
    );
    if let (Some(first_before), Some(first_after)) = (compact_input.first(), retry_input.first()) {
        assert_ne!(
            first_before, first_after,
            "retry should drop the oldest conversation item"
        );
    } else {
        panic!("expected non-empty compact inputs");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_allows_multiple_attempts_when_interleaved_with_other_turn_events() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 500),
    ]);
    let sse2 = sse(vec![
        ev_assistant_message("m2", FIRST_AUTO_SUMMARY),
        ev_completed_with_tokens("r2", 50),
    ]);
    let sse3 = sse(vec![
        ev_function_call(DUMMY_CALL_ID, DUMMY_FUNCTION_NAME, "{}"),
        ev_completed_with_tokens("r3", 150),
    ]);
    let sse4 = sse(vec![
        ev_assistant_message("m4", SECOND_LARGE_REPLY),
        ev_completed_with_tokens("r4", 450),
    ]);
    let sse5 = sse(vec![
        ev_assistant_message("m5", SECOND_AUTO_SUMMARY),
        ev_completed_with_tokens("r5", 60),
    ]);
    let sse6 = sse(vec![
        ev_assistant_message("m6", FINAL_REPLY),
        ev_completed_with_tokens("r6", 120),
    ]);

    mount_sse_sequence(&server, vec![sse1, sse2, sse3, sse4, sse5, sse6]).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    config.model_auto_compact_token_limit = Some(200);
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: MULTI_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();

    let mut auto_compact_lifecycle_events = Vec::new();
    loop {
        let event = codex.next_event().await.unwrap();
        if event.id.starts_with("auto-compact-")
            && matches!(
                event.msg,
                EventMsg::TaskStarted(_) | EventMsg::TaskComplete(_)
            )
        {
            auto_compact_lifecycle_events.push(event);
            continue;
        }
        if let EventMsg::TaskComplete(_) = &event.msg
            && !event.id.starts_with("auto-compact-")
        {
            break;
        }
    }

    assert!(
        auto_compact_lifecycle_events.is_empty(),
        "auto compact should not emit task lifecycle events"
    );

    let request_bodies: Vec<String> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .map(|request| String::from_utf8(request.body).unwrap_or_default())
        .collect();
    assert_eq!(
        request_bodies.len(),
        6,
        "expected six requests including two auto compactions"
    );
    assert!(
        request_bodies[0].contains(MULTI_AUTO_MSG),
        "first request should contain the user input"
    );
    assert!(
        request_bodies[1].contains("You have exceeded the maximum number of tokens"),
        "first auto compact request should include the summarization prompt"
    );
    assert!(
        request_bodies[3].contains(&format!("unsupported call: {DUMMY_FUNCTION_NAME}")),
        "function call output should be sent before the second auto compact"
    );
    assert!(
        request_bodies[4].contains("You have exceeded the maximum number of tokens"),
        "second auto compact request should include the summarization prompt"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_triggers_after_function_call_over_95_percent_usage() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let context_window = 100;
    let limit = context_window * 90 / 100;
    let over_limit_tokens = context_window * 95 / 100 + 1;

    let first_turn = sse(vec![
        ev_function_call(DUMMY_CALL_ID, DUMMY_FUNCTION_NAME, "{}"),
        ev_completed_with_tokens("r1", 50),
    ]);
    let function_call_follow_up = sse(vec![
        ev_assistant_message("m2", FINAL_REPLY),
        ev_completed_with_tokens("r2", over_limit_tokens),
    ]);
    let auto_compact_turn = sse(vec![
        ev_assistant_message("m3", AUTO_SUMMARY_TEXT),
        ev_completed_with_tokens("r3", 10),
    ]);
    let post_auto_compact_turn = sse(vec![ev_completed_with_tokens("r4", 10)]);

    // Mount responses in order and keep mocks only for the ones we assert on.
    let first_turn_mock = mount_sse_once(&server, first_turn).await;
    let follow_up_mock = mount_sse_once(&server, function_call_follow_up).await;
    let auto_compact_mock = mount_sse_once(&server, auto_compact_turn).await;
    // We don't assert on the post-compact request, so no need to keep its mock.
    mount_sse_once(&server, post_auto_compact_turn).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    config.model_context_window = Some(context_window);
    config.model_auto_compact_token_limit = Some(limit);

    let codex = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"))
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: FUNCTION_CALL_LIMIT_MSG.into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |msg| matches!(msg, EventMsg::TaskComplete(_))).await;

    // Assert first request captured expected user message that triggers function call.
    let first_request = first_turn_mock.single_request().input();
    assert!(
        first_request.iter().any(|item| {
            item.get("type").and_then(|value| value.as_str()) == Some("message")
                && item
                    .get("content")
                    .and_then(|content| content.as_array())
                    .and_then(|entries| entries.first())
                    .and_then(|entry| entry.get("text"))
                    .and_then(|value| value.as_str())
                    == Some(FUNCTION_CALL_LIMIT_MSG)
        }),
        "first request should include the user message that triggers the function call"
    );

    let function_call_output = follow_up_mock
        .single_request()
        .function_call_output(DUMMY_CALL_ID);
    let output_text = function_call_output
        .get("output")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    assert!(
        output_text.contains(DUMMY_FUNCTION_NAME),
        "function call output should be sent before auto compact"
    );

    let auto_compact_body = auto_compact_mock.single_request().body_json().to_string();
    assert!(
        auto_compact_body.contains("You have exceeded the maximum number of tokens"),
        "auto compact request should include the summarization prompt after exceeding 95% (limit {limit})"
    );
}
