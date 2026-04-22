use super::*;
use crate::config::Config;
use crate::config::ConfigOverrides;
use crate::config::Constrained;
use crate::config::ManagedFeatures;
use crate::config::NetworkProxySpec;
use crate::config::test_config;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::FeatureRequirementsToml;
use crate::config_loader::NetworkConstraints;
use crate::config_loader::NetworkDomainPermissionToml;
use crate::config_loader::NetworkDomainPermissionsToml;
use crate::config_loader::RequirementSource;
use crate::config_loader::Sourced;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::test_support;
use codex_analytics::GuardianApprovalRequestSource;
use codex_config::config_toml::ConfigToml;
use codex_config::types::McpServerConfig;
use codex_exec_server::LOCAL_FS;
use codex_features::Feature;
use codex_model_provider::create_model_provider;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::ThreadId;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::PathBufExt;
use core_test_support::TempDirExt;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_path_buf;
use insta::Settings;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

fn fixed_guardian_parent_session_id() -> ThreadId {
    ThreadId::from_string("11111111-1111-4111-8111-111111111111")
        .expect("fixed parent session id should be a valid UUID")
}

async fn guardian_test_session_and_turn(
    server: &wiremock::MockServer,
) -> (Arc<Session>, Arc<TurnContext>) {
    guardian_test_session_and_turn_with_base_url(server.uri().as_str()).await
}

async fn guardian_test_session_and_turn_with_base_url(
    base_url: &str,
) -> (Arc<Session>, Arc<TurnContext>) {
    let (mut session, mut turn) = crate::session::tests::make_session_and_context().await;
    session.conversation_id = fixed_guardian_parent_session_id();
    let mut config = (*turn.config).clone();
    config.model_provider.base_url = Some(format!("{base_url}/v1"));
    config.user_instructions = None;
    let config = Arc::new(config);
    let models_manager = Arc::new(test_support::models_manager_with_provider(
        config.codex_home.to_path_buf(),
        Arc::clone(&session.services.auth_manager),
        config.model_provider.clone(),
    ));
    session.services.models_manager = models_manager;
    turn.config = Arc::clone(&config);
    turn.provider = create_model_provider(config.model_provider.clone(), turn.auth_manager.clone());
    turn.user_instructions = None;

    (Arc::new(session), Arc::new(turn))
}

async fn seed_guardian_parent_history(session: &Arc<Session>, turn: &Arc<TurnContext>) {
    session
        .record_into_history(
            &[
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "Please check the repo visibility and push the docs fix if needed."
                            .to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::FunctionCall {
                    id: None,
                    name: "gh_repo_view".to_string(),
                    namespace: None,
                    arguments: "{\"repo\":\"openai/codex\"}".to_string(),
                    call_id: "call-1".to_string(),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "call-1".to_string(),
                    output: codex_protocol::models::FunctionCallOutputPayload::from_text(
                        "repo visibility: public".to_string(),
                    ),
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "The repo is public; I now need approval to push the docs fix."
                            .to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
            ],
            turn.as_ref(),
        )
        .await;
}

fn rollout_item_contains_message_text(item: &RolloutItem, needle: &str) -> bool {
    let RolloutItem::ResponseItem(response_item) = item else {
        return false;
    };
    response_item_contains_message_text(response_item, needle)
}

fn response_item_contains_message_text(item: &ResponseItem, needle: &str) -> bool {
    let ResponseItem::Message { content, .. } = item else {
        return false;
    };
    content.iter().any(|item| match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text } => text.contains(needle),
        ContentItem::InputImage { .. } => false,
    })
}

fn guardian_snapshot_options() -> ContextSnapshotOptions {
    ContextSnapshotOptions::default()
        .strip_capability_instructions()
        .strip_agents_md_user_context()
}

fn normalize_guardian_snapshot_paths(text: String) -> String {
    let platform_path = test_path_buf("/repo/codex-rs/core").display().to_string();
    if platform_path == "/repo/codex-rs/core" {
        return text;
    }

    let escaped_platform_path = serde_json::to_string(&platform_path)
        .expect("test path should serialize")
        .trim_matches('"')
        .to_string();
    text.replace(&escaped_platform_path, "/repo/codex-rs/core")
        .replace(&platform_path, "/repo/codex-rs/core")
}

fn guardian_prompt_text(items: &[codex_protocol::user_input::UserInput]) -> String {
    items
        .iter()
        .map(|item| match item {
            codex_protocol::user_input::UserInput::Text { text, .. } => text.as_str(),
            _ => "",
        })
        .collect::<String>()
}

fn last_user_message_text_from_body(body: &serde_json::Value) -> String {
    body["input"]
        .as_array()
        .expect("request input array")
        .iter()
        .filter(|item| item.get("role").and_then(serde_json::Value::as_str) == Some("user"))
        .filter_map(|item| item.get("content").and_then(serde_json::Value::as_array))
        .next_back()
        .expect("user message content")
        .iter()
        .filter(|span| span.get("type").and_then(serde_json::Value::as_str) == Some("input_text"))
        .filter_map(|span| span.get("text").and_then(serde_json::Value::as_str))
        .collect::<String>()
}

#[test]
fn build_guardian_transcript_keeps_original_numbering() {
    let entries = [
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::User,
            text: "first".to_string(),
        },
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::Assistant,
            text: "second".to_string(),
        },
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::Assistant,
            text: "third".to_string(),
        },
    ];

    let (transcript, omission) = render_guardian_transcript_entries(&entries[..2]);

    assert_eq!(
        transcript,
        vec![
            "[1] user: first".to_string(),
            "[2] assistant: second".to_string()
        ]
    );
    assert!(omission.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn build_guardian_prompt_full_mode_preserves_initial_review_format() -> anyhow::Result<()> {
    let (session, turn) = guardian_test_session_and_turn_with_base_url("http://localhost").await;
    seed_guardian_parent_history(&session, &turn).await;

    let prompt = build_guardian_prompt_items(
        session.as_ref(),
        Some("Sandbox denied outbound git push to github.com.".to_string()),
        GuardianApprovalRequest::Shell {
            id: "shell-1".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the reviewed docs fix.".to_string()),
        },
        GuardianPromptMode::Full,
    )
    .await?;

    let text = guardian_prompt_text(&prompt.items);
    assert!(text.contains("whose request action you are assessing"));
    assert!(text.contains(">>> TRANSCRIPT START\n"));
    assert!(text.contains(">>> TRANSCRIPT END\n"));
    assert!(text.contains("The Codex agent has requested the following action:\n"));
    assert!(!text.contains("TRANSCRIPT DELTA"));
    assert_eq!(prompt.transcript_cursor.transcript_entry_count, 4);

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn build_guardian_prompt_delta_mode_preserves_original_numbering() -> anyhow::Result<()> {
    let (session, turn) = guardian_test_session_and_turn_with_base_url("http://localhost").await;
    seed_guardian_parent_history(&session, &turn).await;
    session
        .record_into_history(
            &[
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "Please also push the second docs fix.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "I need approval for the second push.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
            ],
            turn.as_ref(),
        )
        .await;

    let prompt = build_guardian_prompt_items(
        session.as_ref(),
        /*retry_reason*/ None,
        GuardianApprovalRequest::Shell {
            id: "shell-2".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the second docs fix.".to_string()),
        },
        GuardianPromptMode::Delta {
            cursor: GuardianTranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 4,
            },
        },
    )
    .await?;

    let text = guardian_prompt_text(&prompt.items);
    assert!(text.contains("added since your last approval assessment"));
    assert!(text.contains(">>> TRANSCRIPT DELTA START\n"));
    assert!(text.contains("[5] user: Please also push the second docs fix."));
    assert!(text.contains("[6] assistant: I need approval for the second push."));
    assert!(text.contains(">>> TRANSCRIPT DELTA END\n"));
    assert!(text.contains("The Codex agent has requested the following next action:\n"));
    assert!(!text.contains("[1] user: Please check the repo visibility"));
    assert_eq!(prompt.transcript_cursor.transcript_entry_count, 6);

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn build_guardian_prompt_delta_mode_handles_empty_delta() -> anyhow::Result<()> {
    let (session, turn) = guardian_test_session_and_turn_with_base_url("http://localhost").await;
    seed_guardian_parent_history(&session, &turn).await;

    let prompt = build_guardian_prompt_items(
        session.as_ref(),
        /*retry_reason*/ None,
        GuardianApprovalRequest::Shell {
            id: "shell-2".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the second docs fix.".to_string()),
        },
        GuardianPromptMode::Delta {
            cursor: GuardianTranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 4,
            },
        },
    )
    .await?;

    let text = guardian_prompt_text(&prompt.items);
    assert!(text.contains(">>> TRANSCRIPT DELTA START\n"));
    assert!(text.contains("<no retained transcript delta entries>"));
    assert!(text.contains(">>> TRANSCRIPT DELTA END\n"));
    assert_eq!(prompt.transcript_cursor.transcript_entry_count, 4);

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn build_guardian_prompt_stale_delta_cursor_falls_back_to_full_prompt() -> anyhow::Result<()>
{
    let (session, turn) = guardian_test_session_and_turn_with_base_url("http://localhost").await;
    seed_guardian_parent_history(&session, &turn).await;

    let prompt = build_guardian_prompt_items(
        session.as_ref(),
        /*retry_reason*/ None,
        GuardianApprovalRequest::Shell {
            id: "shell-3".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the docs fix.".to_string()),
        },
        GuardianPromptMode::Delta {
            cursor: GuardianTranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 99,
            },
        },
    )
    .await?;

    let text = guardian_prompt_text(&prompt.items);
    assert!(text.contains("whose request action you are assessing"));
    assert!(text.contains(">>> TRANSCRIPT START\n"));
    assert!(!text.contains("TRANSCRIPT DELTA"));
    assert_eq!(prompt.transcript_cursor.transcript_entry_count, 4);

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn build_guardian_prompt_stale_delta_version_falls_back_to_full_prompt() -> anyhow::Result<()>
{
    let (session, turn) = guardian_test_session_and_turn_with_base_url("http://localhost").await;
    seed_guardian_parent_history(&session, &turn).await;
    session
        .replace_history(
            vec![
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "Compacted retained user request.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "Compacted summary of earlier guardian context.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
            ],
            /*reference_context_item*/ None,
        )
        .await;
    session
        .record_into_history(
            &[
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "Please push after the compaction.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "I need approval for the post-compaction push.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
            ],
            turn.as_ref(),
        )
        .await;

    let prompt = build_guardian_prompt_items(
        session.as_ref(),
        /*retry_reason*/ None,
        GuardianApprovalRequest::Shell {
            id: "shell-4".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push after the compaction.".to_string()),
        },
        GuardianPromptMode::Delta {
            cursor: GuardianTranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 4,
            },
        },
    )
    .await?;

    let text = guardian_prompt_text(&prompt.items);
    assert!(text.contains("whose request action you are assessing"));
    assert!(text.contains(">>> TRANSCRIPT START\n"));
    assert!(!text.contains("TRANSCRIPT DELTA"));
    assert!(text.contains("[3] user: Please push after the compaction."));
    assert!(text.contains("[4] assistant: I need approval for the post-compaction push."));
    assert_eq!(prompt.transcript_cursor.parent_history_version, 1);
    assert_eq!(prompt.transcript_cursor.transcript_entry_count, 4);

    Ok(())
}

#[test]
fn collect_guardian_transcript_entries_skips_contextual_user_messages() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "hello".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
    ];

    let entries = collect_guardian_transcript_entries(&items);

    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0],
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::Assistant,
            text: "hello".to_string(),
        }
    );
}

#[test]
fn collect_guardian_transcript_entries_includes_recent_tool_calls_and_output() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "check the repo".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "read_file".to_string(),
            namespace: None,
            arguments: "{\"path\":\"README.md\"}".to_string(),
            call_id: "call-1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: codex_protocol::models::FunctionCallOutputPayload::from_text(
                "repo is public".to_string(),
            ),
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "I need to push a fix".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
    ];

    let entries = collect_guardian_transcript_entries(&items);

    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries[1],
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::Tool("tool read_file call".to_string()),
            text: "{\"path\":\"README.md\"}".to_string(),
        }
    );
    assert_eq!(
        entries[2],
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::Tool("tool read_file result".to_string()),
            text: "repo is public".to_string(),
        }
    );
}

#[test]
fn guardian_truncate_text_keeps_prefix_suffix_and_xml_marker() {
    let content = "prefix ".repeat(200) + &" suffix".repeat(200);

    let (truncated, was_truncated) = guardian_truncate_text(&content, /*token_cap*/ 20);

    assert!(truncated.starts_with("prefix"));
    assert!(truncated.contains("<truncated omitted_approx_tokens=\""));
    assert!(truncated.ends_with("suffix"));
    assert!(was_truncated);
}

#[test]
fn format_guardian_action_pretty_truncates_large_string_fields() -> serde_json::Result<()> {
    let patch = "line\n".repeat(100_000);
    let action = GuardianApprovalRequest::ApplyPatch {
        id: "patch-1".to_string(),
        cwd: test_path_buf("/tmp").abs(),
        files: Vec::new(),
        patch: patch.clone(),
    };

    let rendered = format_guardian_action_pretty(&action)?;

    assert!(rendered.text.contains("\"tool\": \"apply_patch\""));
    assert!(rendered.text.contains("<truncated omitted_approx_tokens="));
    assert!(rendered.text.len() < patch.len());
    assert!(rendered.truncated);
    Ok(())
}

#[test]
fn format_guardian_action_pretty_reports_no_truncation_for_small_payload() -> serde_json::Result<()>
{
    let action = GuardianApprovalRequest::ApplyPatch {
        id: "patch-1".to_string(),
        cwd: test_path_buf("/tmp").abs(),
        files: Vec::new(),
        patch: "line\n".to_string(),
    };

    let rendered = format_guardian_action_pretty(&action)?;

    assert!(rendered.text.contains("\"tool\": \"apply_patch\""));
    assert!(!rendered.truncated);
    Ok(())
}

#[test]
fn guardian_approval_request_to_json_renders_mcp_tool_call_shape() -> serde_json::Result<()> {
    let action = GuardianApprovalRequest::McpToolCall {
        id: "call-1".to_string(),
        server: "mcp_server".to_string(),
        tool_name: "browser_navigate".to_string(),
        arguments: Some(serde_json::json!({
            "url": "https://example.com",
        })),
        connector_id: None,
        connector_name: Some("Playwright".to_string()),
        connector_description: None,
        tool_title: Some("Navigate".to_string()),
        tool_description: None,
        annotations: Some(GuardianMcpAnnotations {
            destructive_hint: Some(true),
            open_world_hint: None,
            read_only_hint: Some(false),
        }),
    };

    assert_eq!(
        guardian_approval_request_to_json(&action)?,
        serde_json::json!({
            "tool": "mcp_tool_call",
            "server": "mcp_server",
            "tool_name": "browser_navigate",
            "arguments": {
                "url": "https://example.com",
            },
            "connector_name": "Playwright",
            "tool_title": "Navigate",
            "annotations": {
                "destructive_hint": true,
                "read_only_hint": false,
            },
        })
    );
    Ok(())
}

#[test]
fn guardian_assessment_action_redacts_apply_patch_patch_text() {
    let cwd = test_path_buf("/tmp").abs();
    let file = test_path_buf("/tmp/guardian.txt").abs();
    let action = GuardianApprovalRequest::ApplyPatch {
        id: "patch-1".to_string(),
        cwd: cwd.clone(),
        files: vec![file.clone()],
        patch: "*** Begin Patch\n*** Update File: guardian.txt\n@@\n+secret\n*** End Patch"
            .to_string(),
    };

    assert_eq!(
        serde_json::to_value(guardian_assessment_action(&action)).expect("serialize action"),
        serde_json::json!({
            "type": "apply_patch",
            "cwd": cwd,
            "files": [file],
        }),
    );
}

#[test]
fn guardian_request_turn_id_prefers_network_access_owner_turn() {
    let network_access = GuardianApprovalRequest::NetworkAccess {
        id: "network-1".to_string(),
        turn_id: "owner-turn".to_string(),
        target: "https://example.com:443".to_string(),
        host: "example.com".to_string(),
        protocol: NetworkApprovalProtocol::Https,
        port: 443,
    };
    let apply_patch = GuardianApprovalRequest::ApplyPatch {
        id: "patch-1".to_string(),
        cwd: test_path_buf("/tmp").abs(),
        files: vec![test_path_buf("/tmp/guardian.txt").abs()],
        patch: "*** Begin Patch\n*** Update File: guardian.txt\n@@\n+hello\n*** End Patch"
            .to_string(),
    };

    assert_eq!(
        guardian_request_turn_id(&network_access, "fallback-turn"),
        "owner-turn"
    );
    assert_eq!(
        guardian_request_turn_id(&apply_patch, "fallback-turn"),
        "fallback-turn"
    );
}

#[tokio::test]
async fn cancelled_guardian_review_emits_terminal_abort_without_warning() {
    let (session, turn, rx) = crate::session::tests::make_session_and_context_with_rx().await;
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let decision = review_approval_request_with_cancel(
        &session,
        &turn,
        "review-cancelled-guardian".to_string(),
        GuardianApprovalRequest::ApplyPatch {
            id: "patch-1".to_string(),
            cwd: test_path_buf("/tmp").abs(),
            files: vec![test_path_buf("/tmp/guardian.txt").abs()],
            patch: "*** Begin Patch\n*** Update File: guardian.txt\n@@\n+hello\n*** End Patch"
                .to_string(),
        },
        /*retry_reason*/ None,
        GuardianApprovalRequestSource::MainTurn,
        cancel_token,
    )
    .await;

    assert_eq!(decision, ReviewDecision::Abort);

    let mut guardian_statuses = Vec::new();
    let mut warnings = Vec::new();
    while let Ok(event) = rx.try_recv() {
        match event.msg {
            EventMsg::GuardianAssessment(event) => guardian_statuses.push(event.status),
            EventMsg::Warning(event) => warnings.push(event.message),
            _ => {}
        }
    }

    assert_eq!(
        guardian_statuses,
        vec![
            GuardianAssessmentStatus::InProgress,
            GuardianAssessmentStatus::Aborted,
        ]
    );
    assert!(warnings.is_empty());
}

#[test]
fn guardian_timeout_message_distinguishes_timeout_from_policy_denial() {
    let message = guardian_timeout_message();
    assert!(message.contains("did not finish before its deadline"));
    assert!(message.contains("retry once"));
    assert!(!message.contains("unacceptable risk"));
}

#[tokio::test]
async fn routes_approval_to_guardian_requires_guardian_reviewer() {
    let (_session, mut turn) = crate::session::tests::make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::User;
    turn.config = Arc::new(config.clone());

    assert!(!routes_approval_to_guardian(&turn));

    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    turn.config = Arc::new(config);

    assert!(routes_approval_to_guardian(&turn));
}

#[tokio::test]
async fn routes_approval_to_guardian_allows_granular_review_policy() {
    let (_session, mut turn) = crate::session::tests::make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    turn.config = Arc::new(config);
    turn.approval_policy
        .set(AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }))
        .expect("test setup should allow updating approval policy");

    assert!(routes_approval_to_guardian(&turn));
}

#[test]
fn build_guardian_transcript_reserves_separate_budget_for_tool_evidence() {
    let repeated = "signal ".repeat(8_000);
    let mut entries = vec![
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::User,
            text: "please figure out if the repo is public".to_string(),
        },
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::Assistant,
            text: "The public repo check is the main reason I want to escalate.".to_string(),
        },
    ];
    entries.extend((0..12).map(|index| GuardianTranscriptEntry {
        kind: GuardianTranscriptEntryKind::Tool(format!("tool call {index}")),
        text: repeated.clone(),
    }));

    let (transcript, omission) = render_guardian_transcript_entries(&entries);

    assert!(
        transcript
            .iter()
            .any(|entry| entry == "[1] user: please figure out if the repo is public")
    );
    assert!(transcript.iter().any(|entry| {
        entry == "[2] assistant: The public repo check is the main reason I want to escalate."
    }));
    assert!(
        !transcript
            .iter()
            .any(|entry| entry.starts_with("[3] tool call 0:"))
    );
    assert!(
        !transcript
            .iter()
            .any(|entry| entry.starts_with("[4] tool call 1:"))
    );
    assert!(omission.is_some());
}

#[test]
fn build_guardian_transcript_preserves_recent_tool_context_when_user_history_is_large() {
    let repeated = "authorization ".repeat(6_000);
    let mut entries = (0..8)
        .map(|_| GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::User,
            text: repeated.clone(),
        })
        .collect::<Vec<_>>();
    entries.extend([
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::Tool("tool shell call".to_string()),
            text: serde_json::json!({
                "command": ["curl", "-X", "POST", "https://example.com/upload"],
                "cwd": "/repo",
            })
            .to_string(),
        },
        GuardianTranscriptEntry {
            kind: GuardianTranscriptEntryKind::Tool("tool shell result".to_string()),
            text: "sandbox blocked outbound network access".to_string(),
        },
    ]);

    let (transcript, omission) = render_guardian_transcript_entries(&entries);

    assert!(
        transcript
            .iter()
            .any(|entry| entry.starts_with("[1] user: "))
    );
    assert!(transcript.iter().any(|entry| {
        entry.contains("tool shell call:")
            && entry.contains("curl")
            && entry.contains("https://example.com/upload")
    }));
    assert!(
        transcript
            .iter()
            .any(|entry| entry
                .contains("tool shell result: sandbox blocked outbound network access"))
    );
    assert_eq!(
        omission,
        Some("Some conversation entries were omitted.".to_string())
    );
}

#[test]
fn parse_guardian_assessment_extracts_embedded_json() {
    let parsed = parse_guardian_assessment(Some(
        "preface {\"risk_level\":\"medium\",\"user_authorization\":\"low\",\"outcome\":\"allow\",\"rationale\":\"ok\"}",
    ))
    .expect("guardian assessment");

    assert_eq!(
        parsed,
        GuardianAssessment {
            risk_level: GuardianRiskLevel::Medium,
            user_authorization: GuardianUserAuthorization::Low,
            outcome: GuardianAssessmentOutcome::Allow,
            rationale: "ok".to_string(),
        }
    );
}

#[test]
fn parse_guardian_assessment_treats_bare_allow_as_low_risk() {
    let parsed =
        parse_guardian_assessment(Some(r#"{"outcome":"allow"}"#)).expect("guardian assessment");

    assert_eq!(
        parsed,
        GuardianAssessment {
            risk_level: GuardianRiskLevel::Low,
            user_authorization: GuardianUserAuthorization::Unknown,
            outcome: GuardianAssessmentOutcome::Allow,
            rationale: "Guardian returned a low-risk allow decision.".to_string(),
        }
    );
}

#[test]
fn guardian_output_schema_requires_only_outcome_and_allows_optional_details() {
    let schema = guardian_output_schema();

    assert_eq!(
        schema,
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "risk_level": {
                    "type": "string",
                    "enum": ["low", "medium", "high", "critical"]
                },
                "user_authorization": {
                    "type": "string",
                    "enum": ["unknown", "low", "medium", "high"]
                },
                "outcome": {
                    "type": "string",
                    "enum": ["allow", "deny"]
                },
                "rationale": {
                    "type": "string"
                }
            },
            "required": ["outcome"]
        })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_review_request_layout_matches_model_visible_request_snapshot()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let guardian_assessment = serde_json::json!({
        "risk_level": "medium",
        "user_authorization": "high",
        "outcome": "allow",
        "rationale": "The user explicitly requested pushing the reviewed branch to the known remote.",
    })
    .to_string();
    let request_log = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-guardian"),
            ev_assistant_message("msg-guardian", &guardian_assessment),
            ev_completed("resp-guardian"),
        ]),
    )
    .await;

    let (mut session, mut turn) = crate::session::tests::make_session_and_context().await;
    session.conversation_id = fixed_guardian_parent_session_id();
    let temp_cwd = TempDir::new()?;
    let mut config = (*turn.config).clone();
    config.cwd = temp_cwd.abs();
    config.model_provider.base_url = Some(format!("{}/v1", server.uri()));
    let config = Arc::new(config);
    let models_manager = Arc::new(test_support::models_manager_with_provider(
        config.codex_home.to_path_buf(),
        Arc::clone(&session.services.auth_manager),
        config.model_provider.clone(),
    ));
    session.services.models_manager = models_manager;
    turn.config = Arc::clone(&config);
    turn.provider = create_model_provider(config.model_provider.clone(), turn.auth_manager.clone());
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    seed_guardian_parent_history(&session, &turn).await;

    let request = GuardianApprovalRequest::Shell {
        id: "shell-1".to_string(),
        command: vec![
            "git".to_string(),
            "push".to_string(),
            "origin".to_string(),
            "guardian-approval-mvp".to_string(),
        ],
        cwd: test_path_buf("/repo/codex-rs/core").abs(),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: Some("Need to push the reviewed docs fix to the repo remote.".to_string()),
    };

    let outcome = run_guardian_review_session_for_test(
        Arc::clone(&session),
        Arc::clone(&turn),
        request,
        Some("Sandbox denied outbound git push to github.com.".to_string()),
        guardian_output_schema(),
        /*external_cancel*/ None,
    )
    .await;
    let (GuardianReviewOutcome::Completed(assessment), metadata) = outcome else {
        panic!("expected guardian assessment");
    };
    let guardian_thread_id = metadata
        .guardian_thread_id
        .as_deref()
        .expect("guardian thread id");
    assert_eq!(assessment.outcome, GuardianAssessmentOutcome::Allow);
    assert_ne!(guardian_thread_id, session.conversation_id.to_string());
    ThreadId::from_string(guardian_thread_id).expect("guardian thread id should be a valid UUID");
    assert!(matches!(
        metadata.guardian_session_kind,
        Some(codex_analytics::GuardianReviewSessionKind::TrunkNew)
    ));
    let request = request_log.single_request();
    let request_body = request.body_json();
    assert_eq!(
        request_body.pointer("/text/format/strict"),
        Some(&serde_json::json!(false))
    );
    assert_eq!(
        request_body.pointer("/text/format/schema"),
        Some(&serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "risk_level": {
                    "type": "string",
                    "enum": ["low", "medium", "high", "critical"]
                },
                "user_authorization": {
                    "type": "string",
                    "enum": ["unknown", "low", "medium", "high"]
                },
                "outcome": {
                    "type": "string",
                    "enum": ["allow", "deny"]
                },
                "rationale": {
                    "type": "string"
                }
            },
            "required": ["outcome"]
        }))
    );
    let request_model = request_body
        .get("model")
        .and_then(|value| value.as_str())
        .expect("guardian request should include a model");
    let request_reasoning_effort = request_body
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(|value| value.as_str());
    assert_eq!(metadata.guardian_model.as_deref(), Some(request_model));
    assert_eq!(
        metadata.guardian_reasoning_effort.as_deref(),
        request_reasoning_effort
    );
    assert_eq!(metadata.had_prior_review_context, Some(false));
    assert!(
        metadata.time_to_first_token_ms.is_some(),
        "guardian review metadata should capture TTFT when the nested turn completes"
    );

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("snapshots");
    settings.set_prepend_module_to_snapshot(false);
    settings.bind(|| {
        assert_snapshot!(
            "codex_core__guardian__tests__guardian_review_request_layout",
            normalize_guardian_snapshot_paths(context_snapshot::format_labeled_requests_snapshot(
                "Guardian review request layout",
                &[("Guardian Review Request", &request)],
                &guardian_snapshot_options(),
            ))
        );
    });

    Ok(())
}

#[tokio::test]
async fn build_guardian_prompt_items_includes_parent_session_id() -> anyhow::Result<()> {
    let (session, _) = crate::session::tests::make_session_and_context().await;
    let prompt = build_guardian_prompt_items(
        &session,
        /*retry_reason*/ None,
        GuardianApprovalRequest::Shell {
            id: "shell-1".to_string(),
            command: vec!["git".to_string(), "status".to_string()],
            cwd: test_path_buf("/repo").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: None,
        },
        GuardianPromptMode::Full,
    )
    .await?;
    let prompt_text = prompt
        .items
        .into_iter()
        .map(|item| match item {
            codex_protocol::user_input::UserInput::Text { text, .. } => text,
            codex_protocol::user_input::UserInput::Image { .. } => String::new(),
            _ => String::new(),
        })
        .collect::<String>();

    assert!(
        prompt_text.contains(&format!(
            ">>> TRANSCRIPT END\nReviewed Codex session id: {}\n",
            session.conversation_id
        )),
        "guardian prompt should expose the parent session id immediately after the transcript end"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_reuses_prompt_cache_key_and_appends_prior_reviews() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_rationale = "first guardian rationale from the prior review";
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-guardian-1"),
                ev_assistant_message(
                    "msg-guardian-1",
                    &format!(
                        "{{\"risk_level\":\"low\",\"user_authorization\":\"high\",\"outcome\":\"allow\",\"rationale\":\"{first_rationale}\"}}"
                    ),
                ),
                ev_completed("resp-guardian-1"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-2"),
                ev_assistant_message(
                    "msg-guardian-2",
                    "{\"risk_level\":\"low\",\"user_authorization\":\"high\",\"outcome\":\"allow\",\"rationale\":\"second guardian rationale\"}",
                ),
                ev_completed("resp-guardian-2"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-3"),
                ev_assistant_message(
                    "msg-guardian-3",
                    "{\"risk_level\":\"low\",\"user_authorization\":\"high\",\"outcome\":\"allow\",\"rationale\":\"third guardian rationale\"}",
                ),
                ev_completed("resp-guardian-3"),
            ]),
        ],
    )
    .await;

    let (session, turn) = guardian_test_session_and_turn(&server).await;
    seed_guardian_parent_history(&session, &turn).await;

    let first_request = GuardianApprovalRequest::Shell {
        id: "shell-1".to_string(),
        command: vec!["git".to_string(), "push".to_string()],
        cwd: test_path_buf("/repo/codex-rs/core").abs(),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: Some("Need to push the first docs fix.".to_string()),
    };
    let first_outcome = run_guardian_review_session_for_test(
        Arc::clone(&session),
        Arc::clone(&turn),
        first_request,
        Some("First retry reason".to_string()),
        guardian_output_schema(),
        /*external_cancel*/ None,
    )
    .await;
    session
        .record_into_history(
            &[
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "Please push the second docs fix too.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "I need approval for the second docs fix.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
            ],
            turn.as_ref(),
        )
        .await;
    let second_request = GuardianApprovalRequest::Shell {
        id: "shell-2".to_string(),
        command: vec![
            "git".to_string(),
            "push".to_string(),
            "--force-with-lease".to_string(),
        ],
        cwd: test_path_buf("/repo/codex-rs/core").abs(),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: Some("Need to push the second docs fix.".to_string()),
    };
    let second_outcome = run_guardian_review_session_for_test(
        Arc::clone(&session),
        Arc::clone(&turn),
        second_request,
        Some("Second retry reason".to_string()),
        guardian_output_schema(),
        /*external_cancel*/ None,
    )
    .await;
    session
        .record_into_history(
            &[
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "Please push the third docs fix too.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "I need approval for the third docs fix.".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
            ],
            turn.as_ref(),
        )
        .await;
    let third_request = GuardianApprovalRequest::Shell {
        id: "shell-3".to_string(),
        command: vec!["git".to_string(), "push".to_string()],
        cwd: test_path_buf("/repo/codex-rs/core").abs(),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: Some("Need to push the third docs fix.".to_string()),
    };
    let third_outcome = run_guardian_review_session_for_test(
        Arc::clone(&session),
        Arc::clone(&turn),
        third_request,
        Some("Third retry reason".to_string()),
        guardian_output_schema(),
        /*external_cancel*/ None,
    )
    .await;

    let (GuardianReviewOutcome::Completed(first_assessment), first_metadata) = first_outcome else {
        panic!("expected first guardian assessment");
    };
    let (GuardianReviewOutcome::Completed(second_assessment), second_metadata) = second_outcome
    else {
        panic!("expected second guardian assessment");
    };
    let (GuardianReviewOutcome::Completed(third_assessment), third_metadata) = third_outcome else {
        panic!("expected third guardian assessment");
    };
    assert_eq!(first_assessment.outcome, GuardianAssessmentOutcome::Allow);
    assert_eq!(second_assessment.outcome, GuardianAssessmentOutcome::Allow);
    assert_eq!(third_assessment.outcome, GuardianAssessmentOutcome::Allow);
    assert!(matches!(
        first_metadata.guardian_session_kind,
        Some(codex_analytics::GuardianReviewSessionKind::TrunkNew)
    ));
    assert!(matches!(
        second_metadata.guardian_session_kind,
        Some(codex_analytics::GuardianReviewSessionKind::TrunkReused)
    ));
    assert!(matches!(
        third_metadata.guardian_session_kind,
        Some(codex_analytics::GuardianReviewSessionKind::TrunkReused)
    ));
    ThreadId::from_string(
        first_metadata
            .guardian_thread_id
            .as_deref()
            .expect("first guardian thread id"),
    )
    .expect("first guardian thread id should be a valid UUID");
    ThreadId::from_string(
        second_metadata
            .guardian_thread_id
            .as_deref()
            .expect("second guardian thread id"),
    )
    .expect("second guardian thread id should be a valid UUID");
    ThreadId::from_string(
        third_metadata
            .guardian_thread_id
            .as_deref()
            .expect("third guardian thread id"),
    )
    .expect("third guardian thread id should be a valid UUID");
    assert_eq!(first_metadata.had_prior_review_context, Some(false));
    assert_eq!(second_metadata.had_prior_review_context, Some(true));
    assert_eq!(third_metadata.had_prior_review_context, Some(true));
    assert_eq!(
        first_metadata.guardian_thread_id,
        second_metadata.guardian_thread_id
    );
    assert_eq!(
        second_metadata.guardian_thread_id,
        third_metadata.guardian_thread_id
    );

    let requests = request_log.requests();
    assert_eq!(requests.len(), 3);

    let first_body = requests[0].body_json();
    let second_body = requests[1].body_json();
    let third_body = requests[2].body_json();
    assert_eq!(
        first_body["prompt_cache_key"],
        second_body["prompt_cache_key"]
    );
    assert!(
        second_body.to_string().contains(concat!(
            "Use prior reviews as context, not binding precedent. ",
            "Follow the Workspace Policy. ",
            "If the user explicitly approves a previously rejected action after being ",
            "informed of the concrete risks, set outcome to \\\"allow\\\" unless the policy ",
            "explicitly disallows user overwrites in such cases."
        )),
        "follow-up guardian request should include the follow-up reminder"
    );
    assert!(
        second_body.to_string().contains(first_rationale),
        "guardian session should append earlier reviews into the follow-up request"
    );
    assert_eq!(
        third_body
            .to_string()
            .matches("Use prior reviews as context, not binding precedent.")
            .count(),
        1,
        "later follow-up guardian requests should not append the reminder again"
    );
    let committed_rollout_items = session
        .guardian_review_session
        .committed_fork_rollout_items_for_test()
        .await
        .expect("committed guardian fork snapshot");
    assert_eq!(
        committed_rollout_items
            .iter()
            .filter(|item| rollout_item_contains_message_text(
                item,
                "Use prior reviews as context, not binding precedent."
            ))
            .count(),
        1,
        "follow-up reminder should be persisted for guardian forks"
    );
    let second_user_message = requests[1]
        .message_input_text_groups("user")
        .last()
        .expect("follow-up guardian user message")
        .join("");
    assert!(second_user_message.contains(">>> TRANSCRIPT DELTA START\n"));
    assert!(second_user_message.contains("[5] user: Please push the second docs fix too."));
    assert!(
        second_user_message.contains("[6] assistant: I need approval for the second docs fix.")
    );
    assert!(!second_user_message.contains("[1] user: Please check the repo visibility"));

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("snapshots");
    settings.set_prepend_module_to_snapshot(false);
    settings.bind(|| {
        assert_snapshot!(
            "codex_core__guardian__tests__guardian_followup_review_request_layout",
            format!(
                "{}\n\nshared_prompt_cache_key: {}\nfollowup_contains_first_rationale: {}",
                normalize_guardian_snapshot_paths(
                    context_snapshot::format_labeled_requests_snapshot(
                        "Guardian follow-up review request layout",
                        &[
                            ("Initial Guardian Review Request", &requests[0]),
                            ("Follow-up Guardian Review Request", &requests[1]),
                        ],
                        &guardian_snapshot_options(),
                    )
                ),
                first_body["prompt_cache_key"] == second_body["prompt_cache_key"],
                second_body.to_string().contains(first_rationale),
            )
        );
    });

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_review_surfaces_responses_api_errors_in_rejection_reason() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let error_message =
        "Item 'rs_test' of type 'reasoning' was provided without its required following item.";
    let _request_log = mount_response_once(
        &server,
        wiremock::ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": {
                "message": error_message,
                "type": "invalid_request_error",
                "param": "input"
            }
        })),
    )
    .await;

    let (mut session, mut turn, rx) =
        crate::session::tests::make_session_and_context_with_rx().await;
    let mut config = (*turn.config).clone();
    config.model_provider.base_url = Some(format!("{}/v1", server.uri()));
    config.user_instructions = None;
    let config = Arc::new(config);
    let models_manager = Arc::new(test_support::models_manager_with_provider(
        config.codex_home.to_path_buf(),
        Arc::clone(&session.services.auth_manager),
        config.model_provider.clone(),
    ));
    Arc::get_mut(&mut session)
        .expect("session should be uniquely owned")
        .services
        .models_manager = models_manager;
    let turn_mut = Arc::get_mut(&mut turn).expect("turn should be uniquely owned");
    turn_mut.config = Arc::clone(&config);
    turn_mut.provider =
        create_model_provider(config.model_provider.clone(), turn_mut.auth_manager.clone());
    turn_mut.user_instructions = None;

    seed_guardian_parent_history(&session, &turn).await;

    let decision = review_approval_request(
        &session,
        &turn,
        "review-shell-guardian-error".to_string(),
        GuardianApprovalRequest::Shell {
            id: "shell-guardian-error".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the reviewed docs fix.".to_string()),
        },
        /*retry_reason*/ None,
    )
    .await;

    assert_eq!(decision, ReviewDecision::Denied);

    let mut warnings = Vec::new();
    let mut denial_rationales = Vec::new();
    while let Ok(event) = rx.try_recv() {
        match event.msg {
            EventMsg::Warning(event) => warnings.push(event.message),
            EventMsg::GuardianAssessment(event)
                if event.status == GuardianAssessmentStatus::Denied =>
            {
                denial_rationales.push(event.rationale)
            }
            _ => {}
        }
    }

    assert!(
        warnings
            .iter()
            .any(|message| message.contains(error_message)),
        "warning should include the underlying responses api error"
    );
    assert!(
        denial_rationales
            .iter()
            .flatten()
            .any(|message| message.contains(error_message)),
        "denial rationale should include the underlying responses api error"
    );
    assert!(
        denial_rationales.iter().flatten().all(|message| {
            !message.contains("guardian review completed without an assessment payload")
        }),
        "denial rationale should not fall back to the generic missing payload error"
    );
    {
        let rationales = session.services.guardian_rejections.lock().await;
        assert!(rationales.contains_key("review-shell-guardian-error"));
        assert!(!rationales.contains_key("shell-guardian-error"));
    }
    let rejection_message =
        guardian_rejection_message(session.as_ref(), "review-shell-guardian-error").await;
    assert!(
        rejection_message.contains("Reason: Automatic approval review failed:")
            && rejection_message.contains(error_message),
        "rejection message should include guardian rationale: {rejection_message}"
    );

    Ok(())
}

#[tokio::test]
async fn guardian_parallel_reviews_fork_from_last_committed_trunk_history() -> anyhow::Result<()> {
    const TEST_STACK_SIZE_BYTES: usize = 2 * 1024 * 1024;

    let handle =
        std::thread::Builder::new()
            .name("guardian_parallel_reviews_fork_from_last_committed_trunk_history".to_string())
            .stack_size(TEST_STACK_SIZE_BYTES)
            .spawn(|| -> anyhow::Result<()> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(Box::pin(async {
        let first_assessment = serde_json::json!({
            "risk_level": "low",
            "user_authorization": "high",
            "outcome": "allow",
            "rationale": "first guardian rationale",
        })
        .to_string();
        let second_assessment = serde_json::json!({
            "risk_level": "low",
            "user_authorization": "high",
            "outcome": "allow",
            "rationale": "second guardian rationale",
        })
        .to_string();
        let third_assessment = serde_json::json!({
            "risk_level": "low",
            "user_authorization": "high",
            "outcome": "allow",
            "rationale": "third guardian rationale",
        })
        .to_string();
        let (gate_tx, gate_rx) = tokio::sync::oneshot::channel();
        let (server, _) = start_streaming_sse_server(vec![
            vec![StreamingSseChunk {
                gate: None,
                body: sse(vec![
                    ev_response_created("resp-guardian-1"),
                    ev_assistant_message("msg-guardian-1", &first_assessment),
                    ev_completed("resp-guardian-1"),
                ]),
            }],
            vec![
                StreamingSseChunk {
                    gate: None,
                    body: sse(vec![ev_response_created("resp-guardian-2")]),
                },
                StreamingSseChunk {
                    gate: Some(gate_rx),
                    body: sse(vec![
                        ev_assistant_message("msg-guardian-2", &second_assessment),
                        ev_completed("resp-guardian-2"),
                    ]),
                },
            ],
            vec![StreamingSseChunk {
                gate: None,
                body: sse(vec![
                    ev_response_created("resp-guardian-3"),
                    ev_assistant_message("msg-guardian-3", &third_assessment),
                    ev_completed("resp-guardian-3"),
                ]),
            }],
        ])
        .await;

        let (session, turn) = guardian_test_session_and_turn_with_base_url(server.uri()).await;
        seed_guardian_parent_history(&session, &turn).await;

        let initial_request = GuardianApprovalRequest::Shell {
            id: "shell-guardian-1".to_string(),
            command: vec!["git".to_string(), "status".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Inspect repo state before proceeding.".to_string()),
        };
        assert_eq!(
            review_approval_request(
                &session,
                &turn,
                "review-shell-guardian-1".to_string(),
                initial_request,
                /*retry_reason*/ None
            )
            .await,
            ReviewDecision::Approved
        );
        session
            .record_into_history(
                &[
                    ResponseItem::Message {
                        id: None,
                        role: "user".to_string(),
                        content: vec![ContentItem::InputText {
                            text: "Please inspect pending changes before pushing.".to_string(),
                        }],
                        end_turn: None,
                        phase: None,
                    },
                    ResponseItem::Message {
                        id: None,
                        role: "assistant".to_string(),
                        content: vec![ContentItem::OutputText {
                            text: "I need approval to run git diff.".to_string(),
                        }],
                        end_turn: None,
                        phase: None,
                    },
                ],
                turn.as_ref(),
            )
            .await;

        let second_request = GuardianApprovalRequest::Shell {
            id: "shell-guardian-2".to_string(),
            command: vec!["git".to_string(), "diff".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Inspect pending changes before proceeding.".to_string()),
        };
        let third_request = GuardianApprovalRequest::Shell {
            id: "shell-guardian-3".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: test_path_buf("/repo/codex-rs/core").abs(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Inspect whether pushing is safe before proceeding.".to_string()),
        };

        let session_for_second = Arc::clone(&session);
        let turn_for_second = Arc::clone(&turn);
        let mut second_review = tokio::spawn(async move {
            review_approval_request(
                &session_for_second,
                &turn_for_second,
                "review-shell-guardian-2".to_string(),
                second_request,
                Some("trunk follow-up".to_string()),
            )
            .await
        });

        let second_request_observed = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if server.requests().await.len() >= 2 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(
            second_request_observed.is_ok(),
            "second guardian request was not observed"
        );
        session
            .record_into_history(
                &[
                    ResponseItem::Message {
                        id: None,
                        role: "user".to_string(),
                        content: vec![ContentItem::InputText {
                            text: "Now inspect whether pushing is safe.".to_string(),
                        }],
                        end_turn: None,
                        phase: None,
                    },
                    ResponseItem::Message {
                        id: None,
                        role: "assistant".to_string(),
                        content: vec![ContentItem::OutputText {
                            text: "I need approval to push after the diff check.".to_string(),
                        }],
                        end_turn: None,
                        phase: None,
                    },
                ],
                turn.as_ref(),
            )
            .await;

        let third_decision = review_approval_request(
            &session,
            &turn,
            "review-shell-guardian-3".to_string(),
            third_request,
            Some("parallel follow-up".to_string()),
        )
        .await;
        assert_eq!(third_decision, ReviewDecision::Approved);
        let requests = server.requests().await;
        assert_eq!(requests.len(), 3);
        let third_request_body = serde_json::from_slice::<serde_json::Value>(&requests[2])?;
        let third_request_body_text = third_request_body.to_string();
        assert!(
            third_request_body_text.contains("first guardian rationale"),
            "forked guardian review should include the last committed trunk assessment"
        );
        let third_user_message = last_user_message_text_from_body(&third_request_body);
        assert!(third_user_message.contains(">>> TRANSCRIPT DELTA START\n"));
        assert!(
            third_user_message.contains("[5] user: Please inspect pending changes before pushing.")
        );
        assert!(third_user_message.contains("[7] user: Now inspect whether pushing is safe."));
        assert!(!third_user_message.contains("[1] user: Please check the repo visibility"));
        assert!(
            !third_request_body_text.contains("second guardian rationale"),
            "forked guardian review should not include the still in-flight trunk assessment"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut second_review)
                .await
                .is_err(),
            "the trunk guardian review should still be blocked on its gated response"
        );

        gate_tx
            .send(())
            .expect("second guardian review gate should still be open");
        assert_eq!(second_review.await?, ReviewDecision::Approved);
        server.shutdown().await;

        Ok(())
                }))
            })?;

    match handle.join() {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!(
            "guardian_parallel_reviews_fork_from_last_committed_trunk_history thread panicked"
        )),
    }
}
#[tokio::test]
async fn guardian_review_session_config_preserves_parent_network_proxy() {
    let mut parent_config = test_config().await;
    let network = NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig::default(),
        Some(NetworkConstraints {
            enabled: Some(true),
            domains: Some(NetworkDomainPermissionsToml {
                entries: std::collections::BTreeMap::from([(
                    "github.com".to_string(),
                    NetworkDomainPermissionToml::Allow,
                )]),
            }),
            ..Default::default()
        }),
        parent_config.permissions.sandbox_policy.get(),
    )
    .expect("network proxy spec");
    parent_config.permissions.network = Some(network.clone());

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        /*live_network_config*/ None,
        "parent-active-model",
        Some(codex_protocol::openai_models::ReasoningEffort::Low),
    )
    .expect("guardian config");

    assert_eq!(guardian_config.permissions.network, Some(network));
    assert_eq!(
        guardian_config.model,
        Some("parent-active-model".to_string())
    );
    assert_eq!(
        guardian_config.model_reasoning_effort,
        Some(codex_protocol::openai_models::ReasoningEffort::Low)
    );
    assert_eq!(
        guardian_config.permissions.approval_policy,
        Constrained::allow_only(AskForApproval::Never)
    );
    assert_eq!(
        guardian_config.permissions.sandbox_policy,
        Constrained::allow_only(SandboxPolicy::new_read_only_policy())
    );
}

#[tokio::test]
async fn guardian_review_session_config_overrides_parent_developer_instructions() {
    let mut parent_config = test_config().await;
    parent_config.developer_instructions =
        Some("parent or managed config should not replace guardian policy".to_string());

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
    )
    .expect("guardian config");

    assert_eq!(
        guardian_config.developer_instructions,
        Some(guardian_policy_prompt())
    );
}

#[tokio::test]
async fn guardian_review_session_config_uses_live_network_proxy_state() {
    let mut parent_config = test_config().await;
    let mut parent_network = NetworkProxyConfig::default();
    parent_network.network.enabled = true;
    parent_network
        .network
        .set_allowed_domains(vec!["parent.example".to_string()]);
    parent_config.permissions.network = Some(
        NetworkProxySpec::from_config_and_constraints(
            parent_network,
            /*requirements*/ None,
            parent_config.permissions.sandbox_policy.get(),
        )
        .expect("parent network proxy spec"),
    );

    let mut live_network = NetworkProxyConfig::default();
    live_network.network.enabled = true;
    live_network
        .network
        .set_allowed_domains(vec!["github.com".to_string()]);

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        Some(live_network.clone()),
        "active-model",
        /*reasoning_effort*/ None,
    )
    .expect("guardian config");

    assert_eq!(
        guardian_config.permissions.network,
        Some(
            NetworkProxySpec::from_config_and_constraints(
                live_network,
                /*requirements*/ None,
                &SandboxPolicy::new_read_only_policy(),
            )
            .expect("live network proxy spec")
        )
    );
}

#[tokio::test]
async fn guardian_review_session_config_disables_mcp_apps_and_plugins() {
    let mut parent_config = test_config().await;
    let server: McpServerConfig =
        toml::from_str("command = \"docs-server\"").expect("deserialize MCP server");
    parent_config
        .mcp_servers
        .set(HashMap::from([("docs".to_string(), server)]))
        .expect("parent MCP servers are configurable");
    parent_config
        .features
        .enable(Feature::Apps)
        .expect("apps feature is configurable");
    parent_config
        .features
        .enable(Feature::Plugins)
        .expect("plugins feature is configurable");
    parent_config.include_apps_instructions = true;

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
    )
    .expect("guardian config");

    assert!(guardian_config.mcp_servers.get().is_empty());
    assert!(!guardian_config.features.enabled(Feature::Apps));
    assert!(!guardian_config.features.enabled(Feature::Plugins));
    assert!(!guardian_config.include_apps_instructions);
}

#[tokio::test]
async fn guardian_review_session_config_allows_pinned_disabled_feature() {
    let mut parent_config = test_config().await;
    parent_config.features = ManagedFeatures::from_configured(
        parent_config.features.get().clone(),
        Some(Sourced {
            value: FeatureRequirementsToml {
                entries: BTreeMap::from([("multi_agent".to_string(), true)]),
            },
            source: RequirementSource::Unknown,
        }),
    )
    .expect("managed features");

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
    )
    .expect("guardian config should continue when a disabled feature is pinned on");

    assert!(guardian_config.features.enabled(Feature::Collab));
    assert!(guardian_config.mcp_servers.get().is_empty());
    assert!(!guardian_config.include_apps_instructions);
}

#[tokio::test]
async fn guardian_review_session_config_uses_parent_active_model_instead_of_hardcoded_slug() {
    let mut parent_config = test_config().await;
    parent_config.model = Some("configured-model".to_string());

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
    )
    .expect("guardian config");

    assert_eq!(guardian_config.model, Some("active-model".to_string()));
}

#[tokio::test]
async fn guardian_review_session_config_uses_requirements_guardian_policy_config() {
    let codex_home = tempfile::tempdir().expect("create temp dir");
    let workspace = tempfile::tempdir().expect("create temp dir");
    let config_layer_stack = ConfigLayerStack::new(
        Vec::new(),
        Default::default(),
        crate::config_loader::ConfigRequirementsToml {
            guardian_policy_config: Some(
                "  Use the workspace-managed guardian policy.  ".to_string(),
            ),
            ..Default::default()
        },
    )
    .expect("config layer stack");
    let parent_config = Config::load_config_with_layer_stack(
        LOCAL_FS.as_ref(),
        ConfigToml::default(),
        ConfigOverrides {
            cwd: Some(workspace.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.abs(),
        config_layer_stack,
    )
    .await
    .expect("load config");

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
    )
    .expect("guardian config");

    assert_eq!(
        guardian_config.developer_instructions,
        Some(guardian_policy_prompt_with_config(
            "Use the workspace-managed guardian policy."
        ))
    );
}

#[tokio::test]
async fn guardian_review_session_config_uses_default_guardian_policy_without_requirements_override()
{
    let codex_home = tempfile::tempdir().expect("create temp dir");
    let workspace = tempfile::tempdir().expect("create temp dir");
    let config_layer_stack =
        ConfigLayerStack::new(Vec::new(), Default::default(), Default::default())
            .expect("config layer stack");
    let parent_config = Config::load_config_with_layer_stack(
        LOCAL_FS.as_ref(),
        ConfigToml::default(),
        ConfigOverrides {
            cwd: Some(workspace.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.abs(),
        config_layer_stack,
    )
    .await
    .expect("load config");

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        /*live_network_config*/ None,
        "active-model",
        /*reasoning_effort*/ None,
    )
    .expect("guardian config");

    assert_eq!(
        guardian_config.developer_instructions,
        Some(guardian_policy_prompt())
    );
}
