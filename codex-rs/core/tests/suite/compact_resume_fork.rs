#![allow(clippy::expect_used)]

//! Integration tests that cover compacting, resuming, and forking conversations.
//!
//! Each test sets up a mocked SSE conversation and drives the conversation through
//! a specific sequence of operations. After every operation we capture the
//! request payload that Codex would send to the model and assert that the
//! model-visible history matches the expected sequence of messages.

use super::compact::FIRST_REPLY;
use super::compact::SUMMARY_TEXT;
use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::codex::compact::SUMMARIZATION_PROMPT;
use codex_core::config::Config;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::seed_global_agents_context;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use wiremock::MockServer;

const AFTER_SECOND_RESUME: &str = "AFTER_SECOND_RESUME";

fn network_disabled() -> bool {
    std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok()
}

fn filter_out_ghost_snapshot_entries(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .filter(|item| !is_ghost_snapshot_message(item))
        .cloned()
        .collect()
}

fn is_ghost_snapshot_message(item: &Value) -> bool {
    if item.get("type").and_then(Value::as_str) != Some("message") {
        return false;
    }
    if item.get("role").and_then(Value::as_str) != Some("user") {
        return false;
    }
    item.get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|entry| entry.get("text"))
        .and_then(Value::as_str)
        .is_some_and(|text| text.trim_start().starts_with("<ghost_snapshot>"))
}

fn find_message_text_with_prefix(request: &Value, prefix: &str) -> String {
    request
        .get("input")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                item.get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| content.first())
                    .and_then(|entry| entry.get("text"))
                    .and_then(Value::as_str)
                    .filter(|text| text.starts_with(prefix))
            })
        })
        .unwrap_or_else(|| {
            panic!("expected request to include message starting with `{prefix}`: {request:?}")
        })
        .to_string()
}

fn session_prefix(request: &Value) -> Vec<Value> {
    request
        .get("input")
        .and_then(Value::as_array)
        .expect("input array")
        .iter()
        .take(3)
        .cloned()
        .collect()
}

fn tail_messages(request: &Value) -> Vec<(String, String)> {
    request
        .get("input")
        .and_then(Value::as_array)
        .expect("input array")
        .iter()
        .skip(3)
        .map(|item| {
            let role = item["role"]
                .as_str()
                .expect("response item role should be text")
                .to_string();
            let text = item["content"][0]["text"]
                .as_str()
                .expect("response item text expected in content[0]")
                .to_string();
            (role, text)
        })
        .collect()
}

fn user_tail(text: impl Into<String>) -> (String, String) {
    ("user".to_string(), text.into())
}

fn assistant_tail(text: impl Into<String>) -> (String, String) {
    ("assistant".to_string(), text.into())
}

fn conversation_summary(seed: &str) -> String {
    format!(
        "You were originally given instructions from a user over one or more turns. Here were the user messages:\n\n{seed}\n\nAnother language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:\n\n{SUMMARY_TEXT}"
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
/// Scenario: compact an initial conversation, resume it, fork one turn back, and
/// ensure the model-visible history matches expectations at each request.
async fn compact_resume_and_fork_preserve_model_history_view() {
    if network_disabled() {
        println!("Skipping test because network is disabled in this sandbox");
        return;
    }

    // 1. Arrange mocked SSE responses for the initial compact/resume/fork flow.
    let server = MockServer::start().await;
    mount_initial_flow(&server).await;

    // 2. Start a new conversation and drive it through the compact/resume/fork steps.
    let (_home, config, manager, base) = start_test_conversation(&server).await;

    user_turn(&base, "hello world").await;
    compact_conversation(&base).await;
    user_turn(&base, "AFTER_COMPACT").await;
    let base_path = fetch_conversation_path(&base).await;
    assert!(
        base_path.exists(),
        "compact+resume test expects base path {base_path:?} to exist",
    );

    let resumed = resume_conversation(&manager, &config, base_path).await;
    user_turn(&resumed, "AFTER_RESUME").await;
    let resumed_path = fetch_conversation_path(&resumed).await;
    assert!(
        resumed_path.exists(),
        "compact+resume test expects resumed path {resumed_path:?} to exist",
    );

    let forked = fork_conversation(&manager, &config, resumed_path, 2).await;
    user_turn(&forked, "AFTER_FORK").await;

    // 3. Capture the requests to the model and validate the history slices.
    let requests = gather_request_bodies(&server).await;

    assert_eq!(requests.len(), 5);

    let prefix = session_prefix(&requests[0]);
    let instructions = requests[0]["instructions"].clone();
    let first_cache_key = requests[0]["prompt_cache_key"].clone();

    for (idx, request) in requests.iter().enumerate() {
        assert_eq!(
            request["instructions"], instructions,
            "instructions changed at request {idx}"
        );

        if idx < requests.len() - 1 {
            assert_eq!(
                request["prompt_cache_key"], first_cache_key,
                "prompt cache key changed at request {idx}"
            );
        }

        let input = request["input"].as_array().expect("input array");
        assert!(
            input.len() >= prefix.len(),
            "request {idx} is missing session prefix entries"
        );
        assert_eq!(
            &input[..prefix.len()],
            &prefix[..],
            "session prefix mismatch at request {idx}"
        );
    }

    let summary_block = conversation_summary("hello world");
    let expected_tails: Vec<Vec<(String, String)>> = vec![
        vec![user_tail("hello world")],
        vec![
            user_tail("hello world"),
            assistant_tail(FIRST_REPLY),
            user_tail(SUMMARIZATION_PROMPT),
        ],
        vec![user_tail(summary_block.clone()), user_tail("AFTER_COMPACT")],
        vec![
            user_tail(summary_block.clone()),
            user_tail("AFTER_COMPACT"),
            assistant_tail("AFTER_COMPACT_REPLY"),
            user_tail("AFTER_RESUME"),
        ],
        vec![
            user_tail(summary_block),
            user_tail("AFTER_COMPACT"),
            assistant_tail("AFTER_COMPACT_REPLY"),
            user_tail("AFTER_FORK"),
        ],
    ];

    for (idx, (request, expected_tail)) in requests.iter().zip(expected_tails).enumerate() {
        let tail = tail_messages(request);
        assert_eq!(tail, expected_tail, "tail mismatch at request {idx}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
/// Scenario: after the forked branch is compacted, resuming again should reuse
/// the compacted history and only append the new user message.
async fn compact_resume_after_second_compaction_preserves_history() {
    if network_disabled() {
        println!("Skipping test because network is disabled in this sandbox");
        return;
    }

    // 1. Arrange mocked SSE responses for the initial flow plus the second compact.
    let server = MockServer::start().await;
    mount_initial_flow(&server).await;
    mount_second_compact_flow(&server).await;

    // 2. Drive the conversation through compact -> resume -> fork -> compact -> resume.
    let (_home, config, manager, base) = start_test_conversation(&server).await;

    user_turn(&base, "hello world").await;
    compact_conversation(&base).await;
    user_turn(&base, "AFTER_COMPACT").await;
    let base_path = fetch_conversation_path(&base).await;
    assert!(
        base_path.exists(),
        "second compact test expects base path {base_path:?} to exist",
    );

    let resumed = resume_conversation(&manager, &config, base_path).await;
    user_turn(&resumed, "AFTER_RESUME").await;
    let resumed_path = fetch_conversation_path(&resumed).await;
    assert!(
        resumed_path.exists(),
        "second compact test expects resumed path {resumed_path:?} to exist",
    );

    let forked = fork_conversation(&manager, &config, resumed_path, 3).await;
    user_turn(&forked, "AFTER_FORK").await;

    compact_conversation(&forked).await;
    user_turn(&forked, "AFTER_COMPACT_2").await;
    let forked_path = fetch_conversation_path(&forked).await;
    assert!(
        forked_path.exists(),
        "second compact test expects forked path {forked_path:?} to exist",
    );

    let resumed_again = resume_conversation(&manager, &config, forked_path).await;
    user_turn(&resumed_again, AFTER_SECOND_RESUME).await;

    let requests = gather_request_bodies(&server).await;
    let input_after_compact = json!(requests[requests.len() - 2]["input"]);
    let input_after_resume = json!(requests[requests.len() - 1]["input"]);

    // test input after compact before resume is the same as input after resume
    let compact_input_array = input_after_compact
        .as_array()
        .expect("input after compact should be an array");
    let resume_input_array = input_after_resume
        .as_array()
        .expect("input after resume should be an array");
    let compact_filtered = filter_out_ghost_snapshot_entries(compact_input_array);
    let resume_filtered = filter_out_ghost_snapshot_entries(resume_input_array);
    assert!(
        compact_filtered.len() <= resume_filtered.len(),
        "after-resume input should have at least as many items as after-compact"
    );
    assert_eq!(
        compact_filtered.as_slice(),
        &resume_filtered[..compact_filtered.len()]
    );
    // hard coded test
    let prompt = requests[0]["instructions"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let user_instructions = find_message_text_with_prefix(&requests[0], "<user_instructions>");
    let agents_context = find_message_text_with_prefix(&requests[0], "<agents_context>");
    let environment_instructions =
        find_message_text_with_prefix(&requests[0], "<environment_context>");

    let mut expected = json!([
      {
        "instructions": prompt,
        "input": [
          {
            "type": "message",
            "role": "user",
            "content": [
              {
                "type": "input_text",
                "text": user_instructions
              }
            ]
          },
          {
            "type": "message",
            "role": "user",
            "content": [
              {
                "type": "input_text",
                "text": agents_context
              }
            ]
          },
          {
            "type": "message",
            "role": "user",
            "content": [
              {
                "type": "input_text",
                "text": environment_instructions
              }
            ]
          },
          {
            "type": "message",
            "role": "user",
            "content": [
              {
                "type": "input_text",
                "text": "You were originally given instructions from a user over one or more turns. Here were the user messages:\n\nAFTER_FORK\n\nAnother language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:\n\nSUMMARY_ONLY_CONTEXT"
              }
            ]
          },
          {
            "type": "message",
            "role": "user",
            "content": [
              {
                "type": "input_text",
                "text": "AFTER_COMPACT_2"
              }
            ]
          },
          {
            "type": "message",
            "role": "user",
            "content": [
              {
                "type": "input_text",
                "text": "AFTER_SECOND_RESUME"
              }
            ]
          }
        ],
      }
    ]);
    normalize_line_endings(&mut expected);
    let last_request_after_2_compacts = json!([{
        "instructions": requests[requests.len() -1]["instructions"],
        "input": requests[requests.len() -1]["input"],
    }]);
    assert_eq!(expected, last_request_after_2_compacts);
}

fn normalize_line_endings(value: &mut Value) {
    match value {
        Value::String(text) => {
            if text.contains('\r') {
                *text = text.replace("\r\n", "\n").replace('\r', "\n");
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_line_endings(item);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                normalize_line_endings(item);
            }
        }
        _ => {}
    }
}

async fn gather_request_bodies(server: &MockServer) -> Vec<Value> {
    server
        .received_requests()
        .await
        .expect("mock server should not fail")
        .into_iter()
        .map(|req| {
            let mut value = req.body_json::<Value>().expect("valid JSON body");
            normalize_line_endings(&mut value);
            value
        })
        .collect()
}

async fn mount_initial_flow(server: &MockServer) {
    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    let sse2 = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed("r2"),
    ]);
    let sse3 = sse(vec![
        ev_assistant_message("m3", "AFTER_COMPACT_REPLY"),
        ev_completed("r3"),
    ]);
    let sse4 = sse(vec![ev_completed("r4")]);
    let sse5 = sse(vec![ev_completed("r5")]);

    let match_first = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"hello world\"")
            && !body.contains("You have exceeded the maximum number of tokens")
            && !body.contains(&format!("\"text\":\"{SUMMARY_TEXT}\""))
            && !body.contains("\"text\":\"AFTER_COMPACT\"")
            && !body.contains("\"text\":\"AFTER_RESUME\"")
            && !body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once_match(server, match_first, sse1).await;

    let match_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once_match(server, match_compact, sse2).await;

    let match_after_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_COMPACT\"")
            && !body.contains("\"text\":\"AFTER_RESUME\"")
            && !body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once_match(server, match_after_compact, sse3).await;

    let match_after_resume = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_RESUME\"")
    };
    mount_sse_once_match(server, match_after_resume, sse4).await;

    let match_after_fork = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once_match(server, match_after_fork, sse5).await;
}

async fn mount_second_compact_flow(server: &MockServer) {
    let sse6 = sse(vec![
        ev_assistant_message("m4", SUMMARY_TEXT),
        ev_completed("r6"),
    ]);
    let sse7 = sse(vec![ev_completed("r7")]);

    let match_second_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("You have exceeded the maximum number of tokens")
            && body.contains("AFTER_FORK")
    };
    mount_sse_once_match(server, match_second_compact, sse6).await;

    let match_after_second_resume = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{AFTER_SECOND_RESUME}\""))
    };
    mount_sse_once_match(server, match_after_second_resume, sse7).await;
}

async fn start_test_conversation(
    server: &MockServer,
) -> (TempDir, Config, ConversationManager, Arc<CodexConversation>) {
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().expect("create temp dir");
    let _ = seed_global_agents_context(&home, "guide.md", "Global memo.");
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider;

    let manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let NewConversation { conversation, .. } = manager
        .new_conversation(config.clone())
        .await
        .expect("create conversation");

    (home, config, manager, conversation)
}

async fn user_turn(conversation: &Arc<CodexConversation>, text: &str) {
    conversation
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: text.into() }],
        })
        .await
        .expect("submit user turn");
    wait_for_event(conversation, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

async fn compact_conversation(conversation: &Arc<CodexConversation>) {
    conversation
        .submit(Op::Compact)
        .await
        .expect("compact conversation");
    wait_for_event(conversation, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

async fn fetch_conversation_path(conversation: &Arc<CodexConversation>) -> std::path::PathBuf {
    conversation.rollout_path()
}

async fn resume_conversation(
    manager: &ConversationManager,
    config: &Config,
    path: std::path::PathBuf,
) -> Arc<CodexConversation> {
    let auth_manager =
        codex_core::AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
    let NewConversation { conversation, .. } = manager
        .resume_conversation_from_rollout(config.clone(), path, auth_manager)
        .await
        .expect("resume conversation");
    conversation
}

#[cfg(test)]
async fn fork_conversation(
    manager: &ConversationManager,
    config: &Config,
    path: std::path::PathBuf,
    nth_user_message: usize,
) -> Arc<CodexConversation> {
    let NewConversation { conversation, .. } = manager
        .fork_conversation(nth_user_message, config.clone(), path)
        .await
        .expect("fork conversation");
    conversation
}
