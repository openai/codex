#![allow(clippy::expect_used)]

//! Integration tests that cover compacting, resuming, and forking conversations.
//!
//! Each test sets up a mocked SSE conversation and drives the conversation through
//! a specific sequence of operations. After every operation we capture the
//! request payload that Codex would send to the model and assert that the
//! model-visible history matches the expected sequence of messages.

use super::compact::FIRST_REPLY;
use super::compact::SUMMARIZE_TRIGGER;
use super::compact::SUMMARY_TEXT;
use super::compact::ev_assistant_message;
use super::compact::ev_completed;
use super::compact::mount_sse_once;
use super::compact::sse;
use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::project_doc;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_core::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::USER_INSTRUCTIONS_CLOSE_TAG;
use codex_core::protocol::USER_INSTRUCTIONS_OPEN_TAG;
use codex_core::shell;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::config_types::SandboxMode;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use wiremock::MockServer;

const PROJECT_DOC_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";
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
    let initial_context = initial_context_messages(&config).await;

    user_turn(&base, "hello world").await;
    compact_conversation(&base).await;
    user_turn(&base, "AFTER_COMPACT").await;
    let base_path = fetch_conversation_path(&base, "base conversation").await;
    assert!(
        base_path.exists(),
        "compact+resume test expects base path {:?} to exist",
        base_path
    );

    let resumed = resume_conversation(&manager, &config, base_path).await;
    user_turn(&resumed, "AFTER_RESUME").await;
    let resumed_path = fetch_conversation_path(&resumed, "resumed conversation").await;
    assert!(
        resumed_path.exists(),
        "compact+resume test expects resumed path {:?} to exist",
        resumed_path
    );

    let forked = fork_conversation(&manager, &config, resumed_path, 1).await;
    user_turn(&forked, "AFTER_FORK").await;

    // 3. Capture the requests to the model and validate the history slices.
    let requests = gather_request_bodies(&server).await;
    let bridge_after_compact = history_bridge_text(&["hello world"], SUMMARY_TEXT);
    let after_compact_history = request_history_for_user_suffix(
        &requests,
        &[bridge_after_compact.as_str(), "AFTER_COMPACT"],
    );
    let after_resume_history = request_history_for_user_suffix(
        &requests,
        &[
            bridge_after_compact.as_str(),
            "AFTER_COMPACT",
            "AFTER_RESUME",
        ],
    );
    let after_fork_history = request_history_for_user_suffix(
        &requests,
        &[bridge_after_compact.as_str(), "AFTER_COMPACT", "AFTER_FORK"],
    );

    let instruction_text = entry_text(&initial_context[0]);
    let environment_text = entry_text(&initial_context[1]);

    let after_compact_users = user_messages(&after_compact_history);
    assert_eq!(
        after_compact_users,
        vec![
            instruction_text.clone(),
            environment_text.clone(),
            bridge_after_compact.clone(),
            "AFTER_COMPACT".to_string(),
        ]
    );

    let after_resume_users = user_messages(&after_resume_history);
    assert_eq!(
        after_resume_users,
        vec![
            instruction_text.clone(),
            environment_text.clone(),
            bridge_after_compact.clone(),
            "AFTER_COMPACT".to_string(),
            "AFTER_RESUME".to_string(),
        ]
    );

    let after_fork_users = user_messages(&after_fork_history);
    assert_eq!(
        after_fork_users,
        vec![
            instruction_text,
            environment_text,
            bridge_after_compact,
            "AFTER_COMPACT".to_string(),
            "AFTER_FORK".to_string(),
        ]
    );
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
    let initial_context = initial_context_messages(&config).await;

    user_turn(&base, "hello world").await;
    compact_conversation(&base).await;
    user_turn(&base, "AFTER_COMPACT").await;
    let base_path = fetch_conversation_path(&base, "base conversation").await;
    assert!(
        base_path.exists(),
        "second compact test expects base path {:?} to exist",
        base_path
    );

    let resumed = resume_conversation(&manager, &config, base_path).await;
    user_turn(&resumed, "AFTER_RESUME").await;
    let resumed_path = fetch_conversation_path(&resumed, "resumed conversation").await;
    assert!(
        resumed_path.exists(),
        "second compact test expects resumed path {:?} to exist",
        resumed_path
    );

    let forked = fork_conversation(&manager, &config, resumed_path, 1).await;
    user_turn(&forked, "AFTER_FORK").await;

    compact_conversation(&forked).await;
    let forked_path = fetch_conversation_path(&forked, "forked conversation").await;
    assert!(
        forked_path.exists(),
        "second compact test expects forked path {:?} to exist",
        forked_path
    );

    let resumed_again = resume_conversation(&manager, &config, forked_path).await;
    user_turn(&resumed_again, AFTER_SECOND_RESUME).await;

    // 3. Capture the requests and verify the compacted histories.
    let requests = gather_request_bodies(&server).await;
    let bridge_after_compact = history_bridge_text(&["hello world"], SUMMARY_TEXT);
    let after_compact_history = request_history_for_user_suffix(
        &requests,
        &[bridge_after_compact.as_str(), "AFTER_COMPACT"],
    );
    let after_resume_history = request_history_for_user_suffix(
        &requests,
        &[
            bridge_after_compact.as_str(),
            "AFTER_COMPACT",
            "AFTER_RESUME",
        ],
    );
    let after_fork_history = request_history_for_user_suffix(
        &requests,
        &[bridge_after_compact.as_str(), "AFTER_COMPACT", "AFTER_FORK"],
    );
    let after_second_resume_history =
        request_history_for_user_suffix(&requests, &[AFTER_SECOND_RESUME]);

    let instruction_text = entry_text(&initial_context[0]);
    let environment_text = entry_text(&initial_context[1]);

    let after_compact_users = user_messages(&after_compact_history);
    assert_eq!(
        after_compact_users,
        vec![
            instruction_text.clone(),
            environment_text.clone(),
            bridge_after_compact.clone(),
            "AFTER_COMPACT".to_string(),
        ]
    );

    let after_resume_users = user_messages(&after_resume_history);
    assert_eq!(
        after_resume_users,
        vec![
            instruction_text.clone(),
            environment_text.clone(),
            bridge_after_compact.clone(),
            "AFTER_COMPACT".to_string(),
            "AFTER_RESUME".to_string(),
        ]
    );

    let after_fork_users = user_messages(&after_fork_history);
    assert_eq!(
        after_fork_users,
        vec![
            instruction_text.clone(),
            environment_text.clone(),
            bridge_after_compact,
            "AFTER_COMPACT".to_string(),
            "AFTER_FORK".to_string(),
        ]
    );

    let after_second_resume_users = user_messages(&after_second_resume_history);
    assert_eq!(after_second_resume_users.len(), 4);
    assert_eq!(after_second_resume_users[0], instruction_text);
    assert_eq!(after_second_resume_users[1], environment_text);
    assert!(after_second_resume_users[2].starts_with("You were originally given instructions"));
    assert_eq!(after_second_resume_users[3], AFTER_SECOND_RESUME);
}

/// Returns the instruction and environment context messages that every
/// conversation starts with in these tests.
async fn initial_context_messages(config: &Config) -> Vec<Value> {
    let instructions_text = match project_doc::read_project_docs(config).await {
        Ok(Some(doc)) => match &config.user_instructions {
            Some(existing) => Some(format!("{existing}{PROJECT_DOC_SEPARATOR}{doc}")),
            None => Some(doc),
        },
        Ok(None) | Err(_) => config.user_instructions.clone(),
    };

    let mut messages = Vec::new();
    if let Some(text) = instructions_text {
        messages.push(message_value(
            "user",
            "input_text",
            format!("{USER_INSTRUCTIONS_OPEN_TAG}\n\n{text}\n\n{USER_INSTRUCTIONS_CLOSE_TAG}"),
        ));
    }

    let shell = shell::default_user_shell().await;
    messages.push(message_value(
        "user",
        "input_text",
        format_environment_context(
            Some(config.cwd.clone()),
            Some(config.approval_policy),
            Some(config.sandbox_policy.clone()),
            Some(shell),
        ),
    ));
    messages
}

fn message_value(role: &str, content_type: &str, text: String) -> Value {
    json!({
        "type": "message",
        "role": role,
        "content": [{"type": content_type, "text": text}],
    })
}

fn history_bridge_text(user_messages: &[&str], summary_text: &str) -> String {
    let user_messages_text = if user_messages.is_empty() {
        "(none)".to_string()
    } else {
        user_messages.join("\n\n")
    };
    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        summary_text.to_string()
    };
    format!(
        "You were originally given instructions from a user over one or more turns. Here were the user messages:\n\n{user_messages_text}\n\nAnother language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:\n\n{summary_text}"
    )
}

fn format_environment_context(
    cwd: Option<std::path::PathBuf>,
    approval_policy: Option<AskForApproval>,
    sandbox_policy: Option<SandboxPolicy>,
    shell: Option<shell::Shell>,
) -> String {
    let mut lines = vec![ENVIRONMENT_CONTEXT_OPEN_TAG.to_string()];
    if let Some(cwd) = cwd {
        lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
    }
    if let Some(policy) = approval_policy {
        lines.push(format!("  <approval_policy>{policy}</approval_policy>"));
    }
    if let Some(policy) = sandbox_policy {
        match policy {
            SandboxPolicy::DangerFullAccess => {
                lines.push(format!(
                    "  <sandbox_mode>{}</sandbox_mode>",
                    SandboxMode::DangerFullAccess
                ));
                lines.push("  <network_access>enabled</network_access>".to_string());
            }
            SandboxPolicy::ReadOnly => {
                lines.push(format!(
                    "  <sandbox_mode>{}</sandbox_mode>",
                    SandboxMode::ReadOnly
                ));
                lines.push("  <network_access>restricted</network_access>".to_string());
            }
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                ..
            } => {
                lines.push(format!(
                    "  <sandbox_mode>{}</sandbox_mode>",
                    SandboxMode::WorkspaceWrite
                ));
                lines.push(format!(
                    "  <network_access>{}</network_access>",
                    if network_access {
                        "enabled"
                    } else {
                        "restricted"
                    }
                ));
                if !writable_roots.is_empty() {
                    lines.push("  <writable_roots>".to_string());
                    for root in writable_roots {
                        lines.push(format!("    <root>{}</root>", root.to_string_lossy()));
                    }
                    lines.push("  </writable_roots>".to_string());
                }
            }
        }
    }
    if let Some(shell_name) = shell.and_then(|s| s.name()) {
        lines.push(format!("  <shell>{shell_name}</shell>"));
    }
    lines.push(ENVIRONMENT_CONTEXT_CLOSE_TAG.to_string());
    lines.join("\n")
}

async fn gather_request_bodies(server: &MockServer) -> Vec<serde_json::Value> {
    server
        .received_requests()
        .await
        .expect("mock server should not fail")
        .into_iter()
        .map(|req| {
            req.body_json::<serde_json::Value>()
                .expect("valid JSON body")
        })
        .collect()
}

fn user_messages(history: &[Value]) -> Vec<String> {
    history
        .iter()
        .filter_map(|entry| {
            if entry.get("role").and_then(|v| v.as_str()) == Some("user") {
                entry
                    .get("content")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                    })
            } else {
                None
            }
        })
        .flatten()
        .collect()
}

fn entry_text(entry: &Value) -> String {
    entry
        .get("content")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or_default()
        .to_string()
}

fn request_history_for_user_suffix(
    requests: &[serde_json::Value],
    expected_suffix: &[&str],
) -> Vec<Value> {
    let expected_vec: Vec<String> = expected_suffix.iter().map(|s| s.to_string()).collect();
    let (idx, value) = requests
        .iter()
        .enumerate()
        .find(|(_, req)| {
            let history = req.get("input").and_then(|v| v.as_array()).cloned();
            history
                .map(|hist| user_messages(&hist))
                .map_or(false, |users| {
                    users.len() >= expected_vec.len()
                        && users[users.len() - expected_vec.len()..] == expected_vec[..]
                })
        })
        .unwrap_or_else(|| {
            panic!(
                "no request found with user message suffix {:?}",
                expected_suffix
            )
        });

    value
        .get("input")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_else(|| panic!("request {idx} missing input array"))
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
            && !body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\""))
            && !body.contains("\"text\":\"AFTER_COMPACT\"")
            && !body.contains("\"text\":\"AFTER_RESUME\"")
            && !body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once(server, match_first, sse1).await;

    let match_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\""))
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
        body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\"")) && body.contains("AFTER_FORK")
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

async fn fork_conversation(
    manager: &ConversationManager,
    config: &Config,
    path: std::path::PathBuf,
    back_steps: usize,
) -> Arc<CodexConversation> {
    let NewConversation { conversation, .. } = manager
        .fork_conversation(back_steps, config.clone(), path)
        .await
        .expect("fork conversation");
    conversation
}
