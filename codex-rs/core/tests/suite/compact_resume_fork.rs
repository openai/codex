use super::compact::FIRST_REPLY;
use super::compact::SUMMARIZE_TRIGGER;
use super::compact::SUMMARY_TEXT;
use super::compact::ev_assistant_message;
use super::compact::ev_completed;
use super::compact::mount_sse_once;
use super::compact::sse;
use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::project_doc;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::shell;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_protocol::protocol::USER_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::USER_INSTRUCTIONS_OPEN_TAG;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use wiremock::MockServer;

const PROJECT_DOC_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

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

fn user_text_message(text: &str) -> Value {
    message_value("user", "input_text", text.to_string())
}

fn assistant_text_message(text: &str) -> Value {
    message_value("assistant", "output_text", text.to_string())
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
                    codex_protocol::config_types::SandboxMode::DangerFullAccess
                ));
                lines.push("  <network_access>enabled</network_access>".to_string());
            }
            SandboxPolicy::ReadOnly => {
                lines.push(format!(
                    "  <sandbox_mode>{}</sandbox_mode>",
                    codex_protocol::config_types::SandboxMode::ReadOnly
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
                    codex_protocol::config_types::SandboxMode::WorkspaceWrite
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compact_resume_and_fork_preserve_model_history_view() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Start a mock server and mount five sequential expectations.
    let server = MockServer::start().await;

    // 1) Normal assistant reply so it is recorded in history.
    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    // 2) Summarizer returns a summary message (this is what compaction keeps).
    let sse2 = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed("r2"),
    ]);
    // 3) After the post-compact user message, return an assistant reply so the
    //    subsequent history includes: compact msg + user msg + response msg.
    let sse3 = sse(vec![
        ev_assistant_message("m3", "AFTER_COMPACT_REPLY"),
        ev_completed("r3"),
    ]);
    let sse4 = sse(vec![ev_completed("r4")]);
    let sse5 = sse(vec![ev_completed("r5")]);

    // Mount expectations with distinct matchers so they are consumed in order.
    let match_first = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"hello world\"")
            && !body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\""))
    };
    mount_sse_once(&server, match_first, sse1).await;

    let match_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\""))
    };
    mount_sse_once(&server, match_compact, sse2).await;

    let match_after_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_COMPACT\"")
            && !body.contains("\"text\":\"AFTER_RESUME\"")
            && !body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once(&server, match_after_compact, sse3).await;

    let match_after_resume = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_RESUME\"")
    };
    mount_sse_once(&server, match_after_resume, sse4).await;

    let match_after_fork = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once(&server, match_after_fork, sse5).await;

    // Build config pointing to the mock server and spawn Codex.
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider.clone();

    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: base, ..
    } = conversation_manager
        .new_conversation(config.clone())
        .await
        .expect("create conversation");

    // Start -> msg -> response.
    base.submit(Op::UserInput {
        items: vec![InputItem::Text {
            text: "hello world".into(),
        }],
    })
    .await
    .unwrap();
    wait_for_event(&base, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // compact
    base.submit(Op::Compact).await.unwrap();
    wait_for_event(&base, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // -> msg (post-compact)
    base.submit(Op::UserInput {
        items: vec![InputItem::Text {
            text: "AFTER_COMPACT".into(),
        }],
    })
    .await
    .unwrap();
    wait_for_event(&base, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Capture rollout path from base conversation (flushes on GetPath).
    base.submit(Op::GetPath).await.unwrap();
    let base_path =
        match wait_for_event(&base, |ev| matches!(ev, EventMsg::ConversationPath(_))).await {
            EventMsg::ConversationPath(ConversationPathResponseEvent { path, .. }) => path.clone(),
            _ => panic!("expected ConversationPath"),
        };

    // resume -> msg
    let auth_manager =
        codex_core::AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: resumed,
        ..
    } = conversation_manager
        .resume_conversation_from_rollout(config.clone(), base_path.clone(), auth_manager)
        .await
        .expect("resume conversation");

    resumed
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "AFTER_RESUME".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&resumed, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // response -> fork 1 message back
    resumed.submit(Op::GetPath).await.unwrap();
    let resumed_path =
        match wait_for_event(&resumed, |ev| matches!(ev, EventMsg::ConversationPath(_))).await {
            EventMsg::ConversationPath(ConversationPathResponseEvent { path, .. }) => path.clone(),
            _ => panic!("expected ConversationPath after resume"),
        };

    let NewConversation {
        conversation: forked,
        ..
    } = conversation_manager
        .fork_conversation(1, config.clone(), resumed_path.clone())
        .await
        .expect("fork conversation");

    forked
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "AFTER_FORK".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&forked, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Verify five requests were made.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 5, "expected exactly five requests");

    // Extract the model-visible history (input without the trailing user message) for
    // requests 3 (after compact), 4 (after resume), and 5 (after fork).
    let req3_body_after_compact = requests[2].body_json::<serde_json::Value>().unwrap();
    let req4_body_after_resume = requests[3].body_json::<serde_json::Value>().unwrap();
    let req5_body_after_fork = requests[4].body_json::<serde_json::Value>().unwrap();

    let prior_history_after_compact = req3_body_after_compact
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap()
        .clone();
    let prior_history_after_resume = req4_body_after_resume
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap()
        .clone();
    let prior_history_after_fork = req5_body_after_fork
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap()
        .clone();

    let initial_context = initial_context_messages(&config).await;

    let expected_prior_history_after_compact = initial_context
        .iter()
        .cloned()
        .chain(vec![
            assistant_text_message(SUMMARY_TEXT),
            user_text_message("AFTER_COMPACT"),
        ])
        .collect::<Vec<_>>();

    let expected_prior_history_after_resume = initial_context
        .iter()
        .cloned()
        .chain(vec![
            assistant_text_message(SUMMARY_TEXT),
            user_text_message("AFTER_COMPACT"),
            assistant_text_message("AFTER_COMPACT_REPLY"),
            user_text_message("AFTER_RESUME"),
        ])
        .collect::<Vec<_>>();

    let expected_prior_history_after_fork = initial_context
        .iter()
        .cloned()
        .chain(vec![
            assistant_text_message(SUMMARY_TEXT),
            user_text_message("AFTER_COMPACT"),
            assistant_text_message("AFTER_COMPACT_REPLY"),
            user_text_message("AFTER_FORK"),
        ])
        .collect::<Vec<_>>();

    assert_eq!(
        prior_history_after_compact,
        expected_prior_history_after_compact
    );
    assert_eq!(
        prior_history_after_resume,
        expected_prior_history_after_resume
    );
    assert_eq!(prior_history_after_fork, expected_prior_history_after_fork);
}
