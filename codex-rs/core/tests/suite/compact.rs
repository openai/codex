use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::protocol::ErrorEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use codex_core::protocol::WarningEvent;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use std::collections::VecDeque;
use tempfile::TempDir;

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
use serde_json::json;
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
const POST_AUTO_USER_MSG: &str = "post auto follow-up";
const COMPACT_PROMPT_MARKER: &str =
    "You are performing a CONTEXT CHECKPOINT COMPACTION for a tool.";
pub(super) const TEST_COMPACT_PROMPT: &str =
    "You are performing a CONTEXT CHECKPOINT COMPACTION for a tool.\nTest-only compact prompt.";

pub(super) const COMPACT_WARNING_MESSAGE: &str = "Heads up: Long conversations and multiple compactions can cause the model to be less accurate. Start new a new conversation when possible to keep conversations small and targeted.";

fn auto_summary(summary: &str) -> String {
    summary.to_string()
}

fn drop_call_id(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(obj) => {
            obj.retain(|k, _| k != "call_id");
            for v in obj.values_mut() {
                drop_call_id(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                drop_call_id(v);
            }
        }
        _ => {}
    }
}

fn set_test_compact_prompt(config: &mut Config) {
    config.compact_prompt = Some(TEST_COMPACT_PROMPT.to_string());
}

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
        body.contains("\"text\":\"hello world\"") && !body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, first_matcher, sse1).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(COMPACT_PROMPT_MARKER)
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
    set_test_compact_prompt(&mut config);
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
            items: vec![UserInput::Text {
                text: "hello world".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // 2) Summarize – second hit should include the summarization prompt.
    codex.submit(Op::Compact).await.unwrap();
    let warning_event = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning_event else {
        panic!("expected warning event after compact");
    };
    assert_eq!(message, COMPACT_WARNING_MESSAGE);
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // 3) Next user input – third hit; history should include only the summary.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
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
        text2, TEST_COMPACT_PROMPT,
        "expected summarize trigger, got `{text2}`"
    );

    // Third request must contain the refreshed instructions, compacted user history, and new user message.
    let input3 = body3.get("input").and_then(|v| v.as_array()).unwrap();

    assert!(
        input3.len() >= 3,
        "expected refreshed context and new user message in third request"
    );

    let mut messages: Vec<(String, String)> = Vec::new();

    for item in input3 {
        if let Some("message") = item.get("type").and_then(|v| v.as_str()) {
            let role = item
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let text = item
                .get("content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|entry| entry.get("text"))
                .and_then(|v| v.as_str())
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
    assert!(
        messages
            .iter()
            .any(|(r, t)| r == "user" && t == "hello world"),
        "third request should include the original user message"
    );
    assert!(
        messages
            .iter()
            .any(|(r, t)| r == "user" && t == SUMMARY_TEXT),
        "third request should include the summary message"
    );
    assert!(
        !messages
            .iter()
            .any(|(_, text)| text.contains(TEST_COMPACT_PROMPT)),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compact_uses_custom_prompt() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse_stream = sse(vec![ev_completed("r1")]);
    mount_sse_once(&server, sse_stream).await;

    let custom_prompt = "Use this compact prompt instead";

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    config.compact_prompt = Some(custom_prompt.to_string());

    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create conversation")
        .conversation;

    codex.submit(Op::Compact).await.expect("trigger compact");
    let warning_event = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning_event else {
        panic!("expected warning event after compact");
    };
    assert_eq!(message, COMPACT_WARNING_MESSAGE);
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("collect requests");
    let body = requests
        .iter()
        .find_map(|req| req.body_json::<serde_json::Value>().ok())
        .expect("summary request body");

    let input = body
        .get("input")
        .and_then(|v| v.as_array())
        .expect("input array");
    let mut found_custom_prompt = false;
    let mut found_default_prompt = false;

    for item in input {
        if item["type"].as_str() != Some("message") {
            continue;
        }
        let text = item["content"][0]["text"].as_str().unwrap_or_default();
        if text == custom_prompt {
            found_custom_prompt = true;
        }
        if text == TEST_COMPACT_PROMPT {
            found_default_prompt = true;
        }
    }

    assert!(found_custom_prompt, "custom prompt should be injected");
    assert!(!found_default_prompt, "default prompt should be replaced");
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
    let sse_resume = sse(vec![ev_completed("r3-resume")]);
    let sse4 = sse(vec![
        ev_assistant_message("m4", FINAL_REPLY),
        ev_completed_with_tokens("r4", 120),
    ]);

    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(FIRST_AUTO_MSG)
            && !body.contains(SECOND_AUTO_MSG)
            && !body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, first_matcher, sse1).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(SECOND_AUTO_MSG)
            && body.contains(FIRST_AUTO_MSG)
            && !body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, second_matcher, sse2).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, third_matcher, sse3).await;

    let resume_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(AUTO_SUMMARY_TEXT)
            && !body.contains(COMPACT_PROMPT_MARKER)
            && !body.contains(POST_AUTO_USER_MSG)
    };
    mount_sse_once_match(&server, resume_matcher, sse_resume).await;

    let fourth_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(POST_AUTO_USER_MSG) && !body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, fourth_matcher, sse4).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    set_test_compact_prompt(&mut config);
    config.model_auto_compact_token_limit = Some(200_000);
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: FIRST_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: SECOND_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: POST_AUTO_USER_MSG.into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        5,
        "expected user turns, a compaction request, a resumed turn, and the follow-up turn; got {}",
        requests.len()
    );
    let is_auto_compact = |req: &wiremock::Request| {
        std::str::from_utf8(&req.body)
            .unwrap_or("")
            .contains(COMPACT_PROMPT_MARKER)
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

    let resume_index = requests
        .iter()
        .enumerate()
        .find_map(|(idx, req)| {
            let body = std::str::from_utf8(&req.body).unwrap_or("");
            (body.contains(AUTO_SUMMARY_TEXT)
                && !body.contains(COMPACT_PROMPT_MARKER)
                && !body.contains(POST_AUTO_USER_MSG))
            .then_some(idx)
        })
        .expect("resume request missing after compaction");

    let follow_up_index = requests
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, req)| {
            let body = std::str::from_utf8(&req.body).unwrap_or("");
            (body.contains(POST_AUTO_USER_MSG) && !body.contains(COMPACT_PROMPT_MARKER))
                .then_some(idx)
        })
        .expect("follow-up request missing");
    assert_eq!(follow_up_index, 4, "follow-up request should be last");

    let body_first = requests[0].body_json::<serde_json::Value>().unwrap();
    let body_auto = requests[auto_compact_index]
        .body_json::<serde_json::Value>()
        .unwrap();
    let body_resume = requests[resume_index]
        .body_json::<serde_json::Value>()
        .unwrap();
    let body_follow_up = requests[follow_up_index]
        .body_json::<serde_json::Value>()
        .unwrap();
    let instructions = body_auto
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

    let input_auto = body_auto.get("input").and_then(|v| v.as_array()).unwrap();
    let last_auto = input_auto
        .last()
        .expect("auto compact request should append a user message");
    assert_eq!(
        last_auto.get("type").and_then(|v| v.as_str()),
        Some("message")
    );
    assert_eq!(last_auto.get("role").and_then(|v| v.as_str()), Some("user"));
    let last_text = last_auto
        .get("content")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or_default();
    assert_eq!(
        last_text, TEST_COMPACT_PROMPT,
        "auto compact should send the summarization prompt as a user message",
    );

    let input_resume = body_resume.get("input").and_then(|v| v.as_array()).unwrap();
    assert!(
        input_resume.iter().any(|item| {
            item.get("type").and_then(|v| v.as_str()) == Some("message")
                && item.get("role").and_then(|v| v.as_str()) == Some("user")
                && item
                    .get("content")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|entry| entry.get("text"))
                    .and_then(|v| v.as_str())
                    == Some(AUTO_SUMMARY_TEXT)
        }),
        "resume request should include compacted history"
    );

    let input_follow_up = body_follow_up
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap();
    let user_texts: Vec<String> = input_follow_up
        .iter()
        .filter(|item| item.get("type").and_then(|v| v.as_str()) == Some("message"))
        .filter(|item| item.get("role").and_then(|v| v.as_str()) == Some("user"))
        .filter_map(|item| {
            item.get("content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|entry| entry.get("text"))
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string)
        })
        .collect();
    assert!(
        user_texts.iter().any(|text| text == FIRST_AUTO_MSG),
        "auto compact follow-up request should include the first user message"
    );
    assert!(
        user_texts.iter().any(|text| text == SECOND_AUTO_MSG),
        "auto compact follow-up request should include the second user message"
    );
    assert!(
        user_texts.iter().any(|text| text == POST_AUTO_USER_MSG),
        "auto compact follow-up request should include the new user message"
    );
    assert!(
        user_texts.iter().any(|text| text == AUTO_SUMMARY_TEXT),
        "auto compact follow-up request should include the summary message"
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

    let auto_summary_payload = auto_summary(AUTO_SUMMARY_TEXT);
    let sse3 = sse(vec![
        ev_assistant_message("m3", &auto_summary_payload),
        ev_completed_with_tokens("r3", 200),
    ]);

    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(FIRST_AUTO_MSG)
            && !body.contains(SECOND_AUTO_MSG)
            && !body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, first_matcher, sse1).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(SECOND_AUTO_MSG)
            && body.contains(FIRST_AUTO_MSG)
            && !body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, second_matcher, sse2).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, third_matcher, sse3).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    set_test_compact_prompt(&mut config);
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: codex,
        session_configured,
        ..
    } = conversation_manager.new_conversation(config).await.unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: FIRST_AUTO_MSG.into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
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

    let summary_payload = auto_summary(SUMMARY_TEXT);
    let sse2 = sse(vec![
        ev_assistant_message("m2", &summary_payload),
        ev_completed_with_tokens("r2", 50),
    ]);

    let sse3 = sse(vec![
        ev_assistant_message("m3", STILL_TOO_BIG_REPLY),
        ev_completed_with_tokens("r3", 500),
    ]);

    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(FIRST_AUTO_MSG) && !body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, first_matcher, sse1.clone()).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(COMPACT_PROMPT_MARKER)
    };
    mount_sse_once_match(&server, second_matcher, sse2.clone()).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        !body.contains(COMPACT_PROMPT_MARKER) && body.contains(SUMMARY_TEXT)
    };
    mount_sse_once_match(&server, third_matcher, sse3.clone()).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;
    set_test_compact_prompt(&mut config);
    config.model_auto_compact_token_limit = Some(200);
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
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
                .map(|text| text == TEST_COMPACT_PROMPT)
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
    set_test_compact_prompt(&mut config);
    config.model_auto_compact_token_limit = Some(200_000);
    let codex = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"))
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
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
    let warning_event = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning_event else {
        panic!("expected warning event after compact retry");
    };
    assert_eq!(message, COMPACT_WARNING_MESSAGE);
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
        Some(TEST_COMPACT_PROMPT),
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
        Some(TEST_COMPACT_PROMPT),
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
async fn manual_compact_twice_preserves_latest_user_messages() {
    skip_if_no_network!();

    let first_user_message = "first manual turn";
    let second_user_message = "second manual turn";
    let final_user_message = "post compact follow-up";
    let first_summary = "FIRST_MANUAL_SUMMARY";
    let second_summary = "SECOND_MANUAL_SUMMARY";

    let server = start_mock_server().await;

    let first_turn = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    let first_compact_summary = auto_summary(first_summary);
    let first_compact = sse(vec![
        ev_assistant_message("m2", &first_compact_summary),
        ev_completed("r2"),
    ]);
    let second_turn = sse(vec![
        ev_assistant_message("m3", SECOND_LARGE_REPLY),
        ev_completed("r3"),
    ]);
    let second_compact_summary = auto_summary(second_summary);
    let second_compact = sse(vec![
        ev_assistant_message("m4", &second_compact_summary),
        ev_completed("r4"),
    ]);
    let final_turn = sse(vec![
        ev_assistant_message("m5", FINAL_REPLY),
        ev_completed("r5"),
    ]);

    let responses_mock = mount_sse_sequence(
        &server,
        vec![
            first_turn,
            first_compact,
            second_turn,
            second_compact,
            final_turn,
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
    set_test_compact_prompt(&mut config);
    let codex = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"))
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: first_user_message.into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex.submit(Op::Compact).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: second_user_message.into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex.submit(Op::Compact).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: final_user_message.into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = responses_mock.requests();
    assert_eq!(
        requests.len(),
        5,
        "expected exactly 5 requests (user turn, compact, user turn, compact, final turn)"
    );
    let contains_user_text = |input: &[serde_json::Value], expected: &str| -> bool {
        input.iter().any(|item| {
            item.get("type").and_then(|v| v.as_str()) == Some("message")
                && item.get("role").and_then(|v| v.as_str()) == Some("user")
                && item
                    .get("content")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter().any(|entry| {
                            entry.get("text").and_then(|v| v.as_str()) == Some(expected)
                        })
                    })
                    .unwrap_or(false)
        })
    };

    let first_turn_input = requests[0].input();
    assert!(
        contains_user_text(&first_turn_input, first_user_message),
        "first turn request missing first user message"
    );
    assert!(
        !contains_user_text(&first_turn_input, TEST_COMPACT_PROMPT),
        "first turn request should not include summarization prompt"
    );

    let first_compact_input = requests[1].input();
    assert!(
        contains_user_text(&first_compact_input, TEST_COMPACT_PROMPT),
        "first compact request should include summarization prompt"
    );
    assert!(
        contains_user_text(&first_compact_input, first_user_message),
        "first compact request should include history before compaction"
    );

    let second_turn_input = requests[2].input();
    assert!(
        contains_user_text(&second_turn_input, second_user_message),
        "second turn request missing second user message"
    );
    assert!(
        contains_user_text(&second_turn_input, first_user_message),
        "second turn request should include the compacted user history"
    );

    let second_compact_input = requests[3].input();
    assert!(
        contains_user_text(&second_compact_input, TEST_COMPACT_PROMPT),
        "second compact request should include summarization prompt"
    );
    assert!(
        contains_user_text(&second_compact_input, second_user_message),
        "second compact request should include latest history"
    );

    let mut final_output = requests
        .last()
        .unwrap_or_else(|| panic!("final turn request missing for {final_user_message}"))
        .input()
        .into_iter()
        .collect::<VecDeque<_>>();

    // System prompt
    final_output.pop_front();
    // Developer instructions
    final_output.pop_front();

    let _ = final_output
        .iter_mut()
        .map(drop_call_id)
        .collect::<Vec<_>>();

    let expected = vec![
        json!({
            "content": vec![json!({
                "text": first_user_message,
                "type": "input_text",
            })],
            "role": "user",
            "type": "message",
        }),
        json!({
            "content": vec![json!({
                "text": first_summary,
                "type": "input_text",
            })],
            "role": "user",
            "type": "message",
        }),
        json!({
            "content": vec![json!({
                "text": second_user_message,
                "type": "input_text",
            })],
            "role": "user",
            "type": "message",
        }),
        json!({
            "content": vec![json!({
                "text": second_summary,
                "type": "input_text",
            })],
            "role": "user",
            "type": "message",
        }),
        json!({
            "content": vec![json!({
                "text": final_user_message,
                "type": "input_text",
            })],
            "role": "user",
            "type": "message",
        }),
    ];
    assert_eq!(final_output, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_allows_multiple_attempts_when_interleaved_with_other_turn_events() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 500),
    ]);
    let first_summary_payload = auto_summary(FIRST_AUTO_SUMMARY);
    let sse2 = sse(vec![
        ev_assistant_message("m2", &first_summary_payload),
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
    let second_summary_payload = auto_summary(SECOND_AUTO_SUMMARY);
    let sse5 = sse(vec![
        ev_assistant_message("m5", &second_summary_payload),
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
    set_test_compact_prompt(&mut config);
    config.model_auto_compact_token_limit = Some(200);
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
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
        request_bodies[1].contains(COMPACT_PROMPT_MARKER),
        "first auto compact request should include the summarization prompt"
    );
    assert!(
        request_bodies[3].contains(&format!("unsupported call: {DUMMY_FUNCTION_NAME}")),
        "function call output should be sent before the second auto compact"
    );
    assert!(
        request_bodies[4].contains(COMPACT_PROMPT_MARKER),
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
    let auto_summary_payload = auto_summary(AUTO_SUMMARY_TEXT);
    let auto_compact_turn = sse(vec![
        ev_assistant_message("m3", &auto_summary_payload),
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
    set_test_compact_prompt(&mut config);
    config.model_context_window = Some(context_window);
    config.model_auto_compact_token_limit = Some(limit);

    let codex = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"))
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
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
        auto_compact_body.contains(COMPACT_PROMPT_MARKER),
        "auto compact request should include the summarization prompt after exceeding 95% (limit {limit})"
    );
}
