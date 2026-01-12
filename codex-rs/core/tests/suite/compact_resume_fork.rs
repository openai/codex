#![allow(clippy::expect_used)]

//! Integration tests that cover compacting, resuming, and forking conversations.
//!
//! Each test sets up a mocked SSE conversation and drives the conversation through
//! a specific sequence of operations. After every operation we capture the
//! request payload that Codex would send to the model and assert that the
//! model-visible history matches the expected sequence of messages.

use super::compact::COMPACT_WARNING_MESSAGE;
use super::compact::FIRST_REPLY;
use super::compact::SUMMARY_TEXT;
use codex_core::CodexAuth;
use codex_core::CodexThread;
use codex_core::ModelProviderInfo;
use codex_core::NewThread;
use codex_core::ThreadManager;
use codex_core::built_in_model_providers;
use codex_core::compact::SUMMARIZATION_PROMPT;
use codex_core::config::Config;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::WarningEvent;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
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

fn body_contains_text(body: &str, text: &str) -> bool {
    body.contains(&json_fragment(text))
}

fn json_fragment(text: &str) -> String {
    serde_json::to_string(text)
        .expect("serialize text to JSON")
        .trim_matches('"')
        .to_string()
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

fn normalize_line_endings_str(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_string()
    }
}

fn extract_summary_message(request: &Value, summary_text: &str) -> Value {
    request
        .get("input")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("type").and_then(Value::as_str) == Some("message")
                    && item.get("role").and_then(Value::as_str) == Some("user")
                    && item
                        .get("content")
                        .and_then(Value::as_array)
                        .and_then(|arr| arr.first())
                        .and_then(|entry| entry.get("text"))
                        .and_then(Value::as_str)
                        .map(|text| text.contains(summary_text))
                        .unwrap_or(false)
            })
        })
        .cloned()
        .unwrap_or_else(|| panic!("expected summary message {summary_text}"))
}

fn normalize_compact_prompts(requests: &mut [Value]) {
    let normalized_summary_prompt = normalize_line_endings_str(SUMMARIZATION_PROMPT);
    for request in requests {
        if let Some(input) = request.get_mut("input").and_then(Value::as_array_mut) {
            input.retain(|item| {
                if item.get("type").and_then(Value::as_str) != Some("message")
                    || item.get("role").and_then(Value::as_str) != Some("user")
                {
                    return true;
                }
                let content = item
                    .get("content")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if let Some(first) = content.first() {
                    let text = first
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let normalized_text = normalize_line_endings_str(text);
                    !(text.is_empty() || normalized_text == normalized_summary_prompt)
                } else {
                    false
                }
            });
        }
    }
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
    let request_log = mount_initial_flow(&server).await;
    let expected_model = "gpt-5.1-codex";
    // 2. Start a new conversation and drive it through the compact/resume/fork steps.
    let (_home, config, manager, base) =
        start_test_conversation(&server, Some(expected_model)).await;

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

    let forked = fork_thread(&manager, &config, resumed_path, 2).await;
    user_turn(&forked, "AFTER_FORK").await;

    // 3. Capture the requests to the model and validate the history slices.
    let mut requests = gather_request_bodies(&request_log);
    normalize_compact_prompts(&mut requests);

    // input after compact is a prefix of input after resume/fork
    let input_after_compact = json!(requests[requests.len() - 3]["input"]);
    let input_after_resume = json!(requests[requests.len() - 2]["input"]);
    let input_after_fork = json!(requests[requests.len() - 1]["input"]);

    let compact_arr = input_after_compact
        .as_array()
        .expect("input after compact should be an array");
    let resume_arr = input_after_resume
        .as_array()
        .expect("input after resume should be an array");
    let fork_arr = input_after_fork
        .as_array()
        .expect("input after fork should be an array");

    assert!(
        compact_arr.len() <= resume_arr.len(),
        "after-resume input should have at least as many items as after-compact",
    );
    assert_eq!(compact_arr.as_slice(), &resume_arr[..compact_arr.len()]);

    assert!(
        compact_arr.len() <= fork_arr.len(),
        "after-fork input should have at least as many items as after-compact",
    );
    assert_eq!(
        &compact_arr.as_slice()[..compact_arr.len()],
        &fork_arr[..compact_arr.len()]
    );

    let expected_model = requests[0]["model"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let prompt = requests[0]["instructions"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let permissions_message = requests[0]["input"][0].clone();
    let user_instructions = requests[0]["input"][1]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let environment_context = requests[0]["input"][2]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let tool_calls = json!(requests[0]["tools"].as_array());
    let prompt_cache_key = requests[0]["prompt_cache_key"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let fork_prompt_cache_key = requests[requests.len() - 1]["prompt_cache_key"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let summary_after_compact = extract_summary_message(&requests[2], SUMMARY_TEXT);
    let summary_after_resume = extract_summary_message(&requests[3], SUMMARY_TEXT);
    let summary_after_fork = extract_summary_message(&requests[4], SUMMARY_TEXT);
    let user_turn_1 = json!(
    {
      "model": expected_model,
      "instructions": prompt,
      "input": [
        permissions_message,
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
              "text": environment_context
            }
          ]
        },
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "hello world"
            }
          ]
        }
      ],
      "tools": tool_calls,
      "tool_choice": "auto",
      "parallel_tool_calls": false,
      "reasoning": {
        "summary": "auto"
      },
      "store": false,
      "stream": true,
      "include": [
        "reasoning.encrypted_content"
      ],
      "prompt_cache_key": prompt_cache_key
    });
    let compact_1 = json!(
    {
      "model": expected_model,
      "instructions": prompt,
      "input": [
        permissions_message,
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
              "text": environment_context
            }
          ]
        },
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "hello world"
            }
          ]
        },
        {
          "type": "message",
          "role": "assistant",
          "content": [
            {
              "type": "output_text",
              "text": "FIRST_REPLY"
            }
          ]
        },
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": SUMMARIZATION_PROMPT
            }
          ]
        }
      ],
      "tools": [],
      "tool_choice": "auto",
      "parallel_tool_calls": false,
      "reasoning": {
        "summary": "auto"
      },
      "store": false,
      "stream": true,
      "include": [
        "reasoning.encrypted_content"
      ],
      "prompt_cache_key": prompt_cache_key
    });
    let user_turn_2_after_compact = json!(
    {
      "model": expected_model,
      "instructions": prompt,
      "input": [
        permissions_message,
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
              "text": environment_context
            }
          ]
        },
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "hello world"
            }
          ]
        },
        summary_after_compact,
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "AFTER_COMPACT"
            }
          ]
        }
      ],
      "tools": tool_calls,
      "tool_choice": "auto",
      "parallel_tool_calls": false,
      "reasoning": {
        "summary": "auto"
      },
      "store": false,
      "stream": true,
      "include": [
        "reasoning.encrypted_content"
      ],
      "prompt_cache_key": prompt_cache_key
    });
    let usert_turn_3_after_resume = json!(
    {
      "model": expected_model,
      "instructions": prompt,
      "input": [
        permissions_message,
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
              "text": environment_context
            }
          ]
        },
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "hello world"
            }
          ]
        },
        summary_after_resume,
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "AFTER_COMPACT"
            }
          ]
        },
        {
          "type": "message",
          "role": "assistant",
          "content": [
            {
              "type": "output_text",
              "text": "AFTER_COMPACT_REPLY"
            }
          ]
        },
        permissions_message,
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
              "text": environment_context
            }
          ]
        },
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "AFTER_RESUME"
            }
          ]
        }
      ],
      "tools": tool_calls,
      "tool_choice": "auto",
      "parallel_tool_calls": false,
      "reasoning": {
        "summary": "auto"
      },
      "store": false,
      "stream": true,
      "include": [
        "reasoning.encrypted_content"
      ],
      "prompt_cache_key": prompt_cache_key
    });
    let user_turn_3_after_fork = json!(
    {
      "model": expected_model,
      "instructions": prompt,
      "input": [
        permissions_message,
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
              "text": environment_context
            }
          ]
        },
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "hello world"
            }
          ]
        },
        summary_after_fork,
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "AFTER_COMPACT"
            }
          ]
        },
        {
          "type": "message",
          "role": "assistant",
          "content": [
            {
              "type": "output_text",
              "text": "AFTER_COMPACT_REPLY"
            }
          ]
        },
        permissions_message,
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
              "text": environment_context
            }
          ]
        },
        permissions_message,
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
              "text": environment_context
            }
          ]
        },
        {
          "type": "message",
          "role": "user",
          "content": [
            {
              "type": "input_text",
              "text": "AFTER_FORK"
            }
          ]
        }
      ],
      "tools": tool_calls,
      "tool_choice": "auto",
      "parallel_tool_calls": false,
      "reasoning": {
        "summary": "auto"
      },
      "store": false,
      "stream": true,
      "include": [
        "reasoning.encrypted_content"
      ],
      "prompt_cache_key": fork_prompt_cache_key
    });
    let mut expected = json!([
        user_turn_1,
        compact_1,
        user_turn_2_after_compact,
        usert_turn_3_after_resume,
        user_turn_3_after_fork
    ]);
    normalize_line_endings(&mut expected);
    if let Some(arr) = expected.as_array_mut() {
        normalize_compact_prompts(arr);
    }
    assert_eq!(requests.len(), 5);
    assert_eq!(json!(requests), expected);
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
    let mut request_log = mount_initial_flow(&server).await;
    request_log.extend(mount_second_compact_flow(&server).await);

    // 2. Drive the conversation through compact -> resume -> fork -> compact -> resume.
    let (_home, config, manager, base) = start_test_conversation(&server, None).await;

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

    let forked = fork_thread(&manager, &config, resumed_path, 3).await;
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

    let mut requests = gather_request_bodies(&request_log);
    normalize_compact_prompts(&mut requests);
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
    let permissions_message = requests[0]["input"][0].clone();
    let user_instructions = requests[0]["input"][1]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let environment_instructions = requests[0]["input"][2]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    // Build expected final request input: initial context + forked user message +
    // compacted summary + post-compact user message + resumed user message.
    let summary_after_second_compact =
        extract_summary_message(&requests[requests.len() - 3], SUMMARY_TEXT);

    let mut expected = json!([
      {
        "instructions": prompt,
        "input": [
          permissions_message,
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
                "text": "AFTER_FORK"
              }
            ]
          },
          summary_after_second_compact,
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
          permissions_message,
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
                "text": "AFTER_SECOND_RESUME"
              }
            ]
          }
        ],
      }
    ]);
    normalize_line_endings(&mut expected);
    let mut last_request_after_2_compacts = json!([{
        "instructions": requests[requests.len() -1]["instructions"],
        "input": requests[requests.len() -1]["input"],
    }]);
    if let Some(arr) = expected.as_array_mut() {
        normalize_compact_prompts(arr);
    }
    if let Some(arr) = last_request_after_2_compacts.as_array_mut() {
        normalize_compact_prompts(arr);
    }
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

fn gather_request_bodies(request_log: &[ResponseMock]) -> Vec<Value> {
    let mut bodies = request_log
        .iter()
        .flat_map(ResponseMock::requests)
        .map(|request| request.body_json())
        .collect::<Vec<_>>();
    for body in &mut bodies {
        normalize_line_endings(body);
    }
    bodies
}

async fn mount_initial_flow(server: &MockServer) -> Vec<ResponseMock> {
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
            && !body.contains(&format!("\"text\":\"{SUMMARY_TEXT}\""))
            && !body.contains("\"text\":\"AFTER_COMPACT\"")
            && !body.contains("\"text\":\"AFTER_RESUME\"")
            && !body.contains("\"text\":\"AFTER_FORK\"")
    };
    let first = mount_sse_once_match(server, match_first, sse1).await;

    let match_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body_contains_text(body, SUMMARIZATION_PROMPT) || body.contains(&json_fragment(FIRST_REPLY))
    };
    let compact = mount_sse_once_match(server, match_compact, sse2).await;

    let match_after_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_COMPACT\"")
            && !body.contains("\"text\":\"AFTER_RESUME\"")
            && !body.contains("\"text\":\"AFTER_FORK\"")
    };
    let after_compact = mount_sse_once_match(server, match_after_compact, sse3).await;

    let match_after_resume = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_RESUME\"")
    };
    let after_resume = mount_sse_once_match(server, match_after_resume, sse4).await;

    let match_after_fork = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_FORK\"")
    };
    let after_fork = mount_sse_once_match(server, match_after_fork, sse5).await;

    vec![first, compact, after_compact, after_resume, after_fork]
}

async fn mount_second_compact_flow(server: &MockServer) -> Vec<ResponseMock> {
    let sse6 = sse(vec![
        ev_assistant_message("m4", SUMMARY_TEXT),
        ev_completed("r6"),
    ]);
    let sse7 = sse(vec![ev_completed("r7")]);

    let match_second_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("AFTER_FORK")
    };
    let second_compact = mount_sse_once_match(server, match_second_compact, sse6).await;

    let match_after_second_resume = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{AFTER_SECOND_RESUME}\""))
    };
    let after_second_resume = mount_sse_once_match(server, match_after_second_resume, sse7).await;

    vec![second_compact, after_second_resume]
}

async fn start_test_conversation(
    server: &MockServer,
    model: Option<&str>,
) -> (TempDir, Config, ThreadManager, Arc<CodexThread>) {
    let model_provider = ModelProviderInfo {
        name: "Non-OpenAI Model provider".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&home).await;
    config.model_provider = model_provider;
    config.compact_prompt = Some(SUMMARIZATION_PROMPT.to_string());
    if let Some(model) = model {
        config.model = Some(model.to_string());
    }
    let manager = ThreadManager::with_models_provider(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
    );
    let NewThread { thread, .. } = manager
        .start_thread(config.clone())
        .await
        .expect("create conversation");

    (home, config, manager, thread)
}

async fn user_turn(conversation: &Arc<CodexThread>, text: &str) {
    conversation
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: text.into() }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit user turn");
    wait_for_event(conversation, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
}

async fn compact_conversation(conversation: &Arc<CodexThread>) {
    conversation
        .submit(Op::Compact)
        .await
        .expect("compact conversation");
    let warning_event = wait_for_event(conversation, |ev| matches!(ev, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning_event else {
        panic!("expected warning event after compact");
    };
    assert_eq!(message, COMPACT_WARNING_MESSAGE);
    wait_for_event(conversation, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
}

async fn fetch_conversation_path(conversation: &Arc<CodexThread>) -> std::path::PathBuf {
    conversation.rollout_path()
}

async fn resume_conversation(
    manager: &ThreadManager,
    config: &Config,
    path: std::path::PathBuf,
) -> Arc<CodexThread> {
    let auth_manager =
        codex_core::AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
    let NewThread { thread, .. } = manager
        .resume_thread_from_rollout(config.clone(), path, auth_manager)
        .await
        .expect("resume conversation");
    thread
}

#[cfg(test)]
async fn fork_thread(
    manager: &ThreadManager,
    config: &Config,
    path: std::path::PathBuf,
    nth_user_message: usize,
) -> Arc<CodexThread> {
    let NewThread { thread, .. } = manager
        .fork_thread(nth_user_message, config.clone(), path)
        .await
        .expect("fork conversation");
    thread
}
