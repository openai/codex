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
use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_once;
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
    let base_path = fetch_conversation_path(&base, "base conversation").await;
    assert!(
        base_path.exists(),
        "compact+resume test expects base path {base_path:?} to exist",
    );

    let resumed = resume_conversation(&manager, &config, base_path).await;
    user_turn(&resumed, "AFTER_RESUME").await;
    let resumed_path = fetch_conversation_path(&resumed, "resumed conversation").await;
    assert!(
        resumed_path.exists(),
        "compact+resume test expects resumed path {resumed_path:?} to exist",
    );

    let forked = fork_conversation(&manager, &config, resumed_path, 2).await;
    user_turn(&forked, "AFTER_FORK").await;

    // 3. Capture the requests to the model and validate the history slices.
    let requests = gather_request_bodies(&server).await;

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

    let prompt = requests[0]["instructions"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let user_instructions = requests[0]["input"][0]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let environment_context = requests[0]["input"][1]["content"][0]["text"]
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
    let user_turn_1 = json!(
    {
      "model": "gpt-5-codex",
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
      "model": "gpt-5-codex",
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
      "model": "gpt-5-codex",
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
              "text": "You were originally given instructions from a user over one or more turns. Here were the user messages:

hello world

Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:

SUMMARY_ONLY_CONTEXT"
            }
          ]
        },
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
      "model": "gpt-5-codex",
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
              "text": "You were originally given instructions from a user over one or more turns. Here were the user messages:

hello world

Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:

SUMMARY_ONLY_CONTEXT"
            }
          ]
        },
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
      "model": "gpt-5-codex",
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
              "text": "You were originally given instructions from a user over one or more turns. Here were the user messages:

hello world

Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:

SUMMARY_ONLY_CONTEXT"
            }
          ]
        },
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
    mount_initial_flow(&server).await;
    mount_second_compact_flow(&server).await;

    // 2. Drive the conversation through compact -> resume -> fork -> compact -> resume.
    let (_home, config, manager, base) = start_test_conversation(&server).await;

    user_turn(&base, "hello world").await;
    compact_conversation(&base).await;
    user_turn(&base, "AFTER_COMPACT").await;
    let base_path = fetch_conversation_path(&base, "base conversation").await;
    assert!(
        base_path.exists(),
        "second compact test expects base path {base_path:?} to exist",
    );

    let resumed = resume_conversation(&manager, &config, base_path).await;
    user_turn(&resumed, "AFTER_RESUME").await;
    let resumed_path = fetch_conversation_path(&resumed, "resumed conversation").await;
    assert!(
        resumed_path.exists(),
        "second compact test expects resumed path {resumed_path:?} to exist",
    );

    let forked = fork_conversation(&manager, &config, resumed_path, 3).await;
    user_turn(&forked, "AFTER_FORK").await;

    compact_conversation(&forked).await;
    user_turn(&forked, "AFTER_COMPACT_2").await;
    let forked_path = fetch_conversation_path(&forked, "forked conversation").await;
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
    assert!(
        compact_input_array.len() <= resume_input_array.len(),
        "after-resume input should have at least as many items as after-compact"
    );
    assert_eq!(
        compact_input_array.as_slice(),
        &resume_input_array[..compact_input_array.len()]
    );
    // hard coded test
    let prompt = requests[0]["instructions"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let user_instructions = requests[0]["input"][0]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let environment_instructions = requests[0]["input"][1]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();

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
    mount_sse_once(server, match_first, sse1).await;

    let match_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("You have exceeded the maximum number of tokens")
    };
    mount_sse_once(server, match_compact, sse2).await;

    let match_after_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_COMPACT\"")
            && !body.contains("\"text\":\"AFTER_RESUME\"")
            && !body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once(server, match_after_compact, sse3).await;

    let match_after_resume = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_RESUME\"")
    };
    mount_sse_once(server, match_after_resume, sse4).await;

    let match_after_fork = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once(server, match_after_fork, sse5).await;
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
    mount_sse_once(server, match_second_compact, sse6).await;

    let match_after_second_resume = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{AFTER_SECOND_RESUME}\""))
    };
    mount_sse_once(server, match_after_second_resume, sse7).await;
}

async fn start_test_conversation(
    server: &MockServer,
) -> (TempDir, Config, ConversationManager, Arc<CodexConversation>) {
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().expect("create temp dir");
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
            items: vec![InputItem::Text { text: text.into() }],
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

async fn fetch_conversation_path(
    conversation: &Arc<CodexConversation>,
    context: &str,
) -> std::path::PathBuf {
    conversation
        .submit(Op::GetPath)
        .await
        .expect("request conversation path");
    match wait_for_event(conversation, |ev| {
        matches!(ev, EventMsg::ConversationPath(_))
    })
    .await
    {
        EventMsg::ConversationPath(ConversationPathResponseEvent { path, .. }) => path,
        _ => panic!("expected ConversationPath event for {context}"),
    }
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
