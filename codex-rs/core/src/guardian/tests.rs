use super::*;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::config::ConfigOverrides;
use crate::config::ConfigToml;
use crate::config::Constrained;
use crate::config::ManagedFeatures;
use crate::config::NetworkProxySpec;
use crate::config::test_config;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::FeatureRequirementsToml;
use crate::config_loader::NetworkConstraints;
use crate::config_loader::RequirementSource;
use crate::config_loader::Sourced;
use crate::protocol::SandboxPolicy;
use crate::test_support;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::ReviewDecision;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use insta::Settings;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

async fn guardian_test_session_and_turn(
    server: &wiremock::MockServer,
) -> (Arc<Session>, Arc<TurnContext>) {
    guardian_test_session_and_turn_with_base_url(server.uri().as_str()).await
}

async fn guardian_test_session_and_turn_with_base_url(
    base_url: &str,
) -> (Arc<Session>, Arc<TurnContext>) {
    let (mut session, mut turn) = crate::codex::make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.model_provider.base_url = Some(format!("{base_url}/v1"));
    config.user_instructions = None;
    let config = Arc::new(config);
    let models_manager = Arc::new(test_support::models_manager_with_provider(
        config.codex_home.clone(),
        Arc::clone(&session.services.auth_manager),
        config.model_provider.clone(),
    ));
    session.services.models_manager = models_manager;
    turn.config = Arc::clone(&config);
    turn.provider = config.model_provider.clone();
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
                    metadata: None,
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::FunctionCall {
                    id: None,
                    name: "gh_repo_view".to_string(),
                    namespace: None,
                    arguments: "{\"repo\":\"openai/codex\"}".to_string(),
                    call_id: "call-1".to_string(),
                    metadata: None,
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "call-1".to_string(),
                    output: codex_protocol::models::FunctionCallOutputPayload::from_text(
                        "repo visibility: public".to_string(),
                    ),
                    metadata: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "The repo is public; I now need approval to push the docs fix."
                            .to_string(),
                    }],
                    metadata: None,
                    end_turn: None,
                    phase: None,
                },
            ],
            turn.as_ref(),
        )
        .await;
}

fn guardian_snapshot_options() -> ContextSnapshotOptions {
    ContextSnapshotOptions::default()
        .strip_capability_instructions()
        .strip_agents_md_user_context()
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

#[test]
fn collect_guardian_transcript_entries_skips_contextual_user_messages() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>".to_string(),
            }],
            metadata: None,
            end_turn: None,
            phase: None,
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "hello".to_string(),
            }],
            metadata: None,
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
            metadata: None,
            end_turn: None,
            phase: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "read_file".to_string(),
            namespace: None,
            arguments: "{\"path\":\"README.md\"}".to_string(),
            call_id: "call-1".to_string(),
            metadata: None,
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: codex_protocol::models::FunctionCallOutputPayload::from_text(
                "repo is public".to_string(),
            ),
            metadata: None,
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "I need to push a fix".to_string(),
            }],
            metadata: None,
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

    let truncated = guardian_truncate_text(&content, 20);

    assert!(truncated.starts_with("prefix"));
    assert!(truncated.contains("<truncated omitted_approx_tokens=\""));
    assert!(truncated.ends_with("suffix"));
}

#[test]
fn format_guardian_action_pretty_truncates_large_string_fields() -> serde_json::Result<()> {
    let patch = "line\n".repeat(10_000);
    let action = GuardianApprovalRequest::ApplyPatch {
        id: "patch-1".to_string(),
        cwd: PathBuf::from("/tmp"),
        files: Vec::new(),
        change_count: 1usize,
        patch: patch.clone(),
    };

    let rendered = format_guardian_action_pretty(&action)?;

    assert!(rendered.contains("\"tool\": \"apply_patch\""));
    assert!(rendered.len() < patch.len());
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
fn guardian_assessment_action_value_redacts_apply_patch_patch_text() {
    let (cwd, file) = if cfg!(windows) {
        (r"C:\tmp", r"C:\tmp\guardian.txt")
    } else {
        ("/tmp", "/tmp/guardian.txt")
    };
    let cwd = PathBuf::from(cwd);
    let file = AbsolutePathBuf::try_from(file).expect("absolute path");
    let action = GuardianApprovalRequest::ApplyPatch {
        id: "patch-1".to_string(),
        cwd: cwd.clone(),
        files: vec![file.clone()],
        change_count: 1usize,
        patch: "*** Begin Patch\n*** Update File: guardian.txt\n@@\n+secret\n*** End Patch"
            .to_string(),
    };

    assert_eq!(
        guardian_assessment_action_value(&action),
        serde_json::json!({
            "tool": "apply_patch",
            "cwd": cwd,
            "files": [file],
            "change_count": 1,
        })
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
        cwd: PathBuf::from("/tmp"),
        files: vec![AbsolutePathBuf::try_from("/tmp/guardian.txt").expect("absolute path")],
        change_count: 1usize,
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
    let (session, turn, rx) = crate::codex::make_session_and_context_with_rx().await;
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let decision = review_approval_request_with_cancel(
        &session,
        &turn,
        GuardianApprovalRequest::ApplyPatch {
            id: "patch-1".to_string(),
            cwd: PathBuf::from("/tmp"),
            files: vec![AbsolutePathBuf::try_from("/tmp/guardian.txt").expect("absolute path")],
            change_count: 1usize,
            patch: "*** Begin Patch\n*** Update File: guardian.txt\n@@\n+hello\n*** End Patch"
                .to_string(),
        },
        None,
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

#[tokio::test]
async fn routes_approval_to_guardian_requires_auto_only_review_policy() {
    let (_session, mut turn) = crate::codex::make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::User;
    turn.config = Arc::new(config.clone());

    assert!(!routes_approval_to_guardian(&turn));

    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    turn.config = Arc::new(config);

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
fn parse_guardian_assessment_extracts_embedded_json() {
    let parsed = parse_guardian_assessment(Some(
        "preface {\"risk_level\":\"medium\",\"risk_score\":42,\"rationale\":\"ok\",\"evidence\":[]}",
    ))
    .expect("guardian assessment");

    assert_eq!(parsed.risk_score, 42);
    assert_eq!(parsed.risk_level, GuardianRiskLevel::Medium);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_review_request_layout_matches_model_visible_request_snapshot()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let guardian_assessment = serde_json::json!({
        "risk_level": "medium",
        "risk_score": 35,
        "rationale": "The user explicitly requested pushing the reviewed branch to the known remote.",
        "evidence": [{
            "message": "The user asked to check repo visibility and then push the docs fix.",
            "why": "This authorizes the specific network action under review.",
        }],
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

    let (mut session, mut turn) = crate::codex::make_session_and_context().await;
    let temp_cwd = TempDir::new()?;
    let mut config = (*turn.config).clone();
    config.cwd = temp_cwd.path().to_path_buf();
    config.model_provider.base_url = Some(format!("{}/v1", server.uri()));
    let config = Arc::new(config);
    let models_manager = Arc::new(test_support::models_manager_with_provider(
        config.codex_home.clone(),
        Arc::clone(&session.services.auth_manager),
        config.model_provider.clone(),
    ));
    session.services.models_manager = models_manager;
    turn.config = Arc::clone(&config);
    turn.provider = config.model_provider.clone();
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    seed_guardian_parent_history(&session, &turn).await;

    let prompt = build_guardian_prompt_items(
        session.as_ref(),
        Some("Sandbox denied outbound git push to github.com.".to_string()),
        GuardianApprovalRequest::Shell {
            id: "shell-1".to_string(),
            command: vec![
                "git".to_string(),
                "push".to_string(),
                "origin".to_string(),
                "guardian-approval-mvp".to_string(),
            ],
            cwd: PathBuf::from("/repo/codex-rs/core"),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some(
                "Need to push the reviewed docs fix to the repo remote.".to_string(),
            ),
        },
    )
    .await?;

    let outcome = run_guardian_review_session_for_test(
        Arc::clone(&session),
        Arc::clone(&turn),
        prompt,
        guardian_output_schema(),
        None,
    )
    .await;
    let GuardianReviewOutcome::Completed(Ok(assessment)) = outcome else {
        panic!("expected guardian assessment");
    };
    assert_eq!(assessment.risk_score, 35);

    let request = request_log.single_request();
    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("snapshots");
    settings.set_prepend_module_to_snapshot(false);
    settings.bind(|| {
        assert_snapshot!(
            "codex_core__guardian__tests__guardian_review_request_layout",
            context_snapshot::format_labeled_requests_snapshot(
                "Guardian review request layout",
                &[("Guardian Review Request", &request)],
                &guardian_snapshot_options(),
            )
        );
    });

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
                        "{{\"risk_level\":\"low\",\"risk_score\":5,\"rationale\":\"{first_rationale}\",\"evidence\":[]}}"
                    ),
                ),
                ev_completed("resp-guardian-1"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-2"),
                ev_assistant_message(
                    "msg-guardian-2",
                    "{\"risk_level\":\"low\",\"risk_score\":7,\"rationale\":\"second guardian rationale\",\"evidence\":[]}",
                ),
                ev_completed("resp-guardian-2"),
            ]),
        ],
    )
    .await;

    let (session, turn) = guardian_test_session_and_turn(&server).await;
    seed_guardian_parent_history(&session, &turn).await;

    let first_prompt = build_guardian_prompt_items(
        session.as_ref(),
        Some("First retry reason".to_string()),
        GuardianApprovalRequest::Shell {
            id: "shell-1".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: PathBuf::from("/repo/codex-rs/core"),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the first docs fix.".to_string()),
        },
    )
    .await?;
    let first_outcome = run_guardian_review_session_for_test(
        Arc::clone(&session),
        Arc::clone(&turn),
        first_prompt,
        guardian_output_schema(),
        None,
    )
    .await;
    let second_prompt = build_guardian_prompt_items(
        session.as_ref(),
        Some("Second retry reason".to_string()),
        GuardianApprovalRequest::Shell {
            id: "shell-2".to_string(),
            command: vec![
                "git".to_string(),
                "push".to_string(),
                "--force-with-lease".to_string(),
            ],
            cwd: PathBuf::from("/repo/codex-rs/core"),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the second docs fix.".to_string()),
        },
    )
    .await?;
    let second_outcome = run_guardian_review_session_for_test(
        Arc::clone(&session),
        Arc::clone(&turn),
        second_prompt,
        guardian_output_schema(),
        None,
    )
    .await;

    let GuardianReviewOutcome::Completed(Ok(first_assessment)) = first_outcome else {
        panic!("expected first guardian assessment");
    };
    let GuardianReviewOutcome::Completed(Ok(second_assessment)) = second_outcome else {
        panic!("expected second guardian assessment");
    };
    assert_eq!(first_assessment.risk_score, 5);
    assert_eq!(second_assessment.risk_score, 7);

    let requests = request_log.requests();
    assert_eq!(requests.len(), 2);

    let first_body = requests[0].body_json();
    let second_body = requests[1].body_json();
    assert_eq!(
        first_body["prompt_cache_key"],
        second_body["prompt_cache_key"]
    );
    assert!(
        second_body.to_string().contains(first_rationale),
        "guardian session should append earlier reviews into the follow-up request"
    );

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("snapshots");
    settings.set_prepend_module_to_snapshot(false);
    settings.bind(|| {
        assert_snapshot!(
            "codex_core__guardian__tests__guardian_followup_review_request_layout",
            format!(
                "{}\n\nshared_prompt_cache_key: {}\nfollowup_contains_first_rationale: {}",
                context_snapshot::format_labeled_requests_snapshot(
                    "Guardian follow-up review request layout",
                    &[
                        ("Initial Guardian Review Request", &requests[0]),
                        ("Follow-up Guardian Review Request", &requests[1]),
                    ],
                    &guardian_snapshot_options(),
                ),
                first_body["prompt_cache_key"] == second_body["prompt_cache_key"],
                second_body.to_string().contains(first_rationale),
            )
        );
    });

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn guardian_parallel_reviews_fork_from_last_committed_trunk_history() -> anyhow::Result<()> {
    let first_assessment = serde_json::json!({
        "risk_level": "low",
        "risk_score": 4,
        "rationale": "first guardian rationale",
        "evidence": [],
    })
    .to_string();
    let second_assessment = serde_json::json!({
        "risk_level": "low",
        "risk_score": 7,
        "rationale": "second guardian rationale",
        "evidence": [],
    })
    .to_string();
    let third_assessment = serde_json::json!({
        "risk_level": "low",
        "risk_score": 9,
        "rationale": "third guardian rationale",
        "evidence": [],
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
        cwd: PathBuf::from("/repo/codex-rs/core"),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: Some("Inspect repo state before proceeding.".to_string()),
    };
    assert_eq!(
        review_approval_request(&session, &turn, initial_request, None).await,
        ReviewDecision::Approved
    );

    let second_request = GuardianApprovalRequest::Shell {
        id: "shell-guardian-2".to_string(),
        command: vec!["git".to_string(), "diff".to_string()],
        cwd: PathBuf::from("/repo/codex-rs/core"),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: Some("Inspect pending changes before proceeding.".to_string()),
    };
    let third_request = GuardianApprovalRequest::Shell {
        id: "shell-guardian-3".to_string(),
        command: vec!["git".to_string(), "push".to_string()],
        cwd: PathBuf::from("/repo/codex-rs/core"),
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

    let third_decision = review_approval_request(
        &session,
        &turn,
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
}
#[test]
fn guardian_review_session_config_preserves_parent_network_proxy() {
    let mut parent_config = test_config();
    let network = NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig::default(),
        Some(NetworkConstraints {
            enabled: Some(true),
            allowed_domains: Some(vec!["github.com".to_string()]),
            ..Default::default()
        }),
        parent_config.permissions.sandbox_policy.get(),
    )
    .expect("network proxy spec");
    parent_config.permissions.network = Some(network.clone());

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        None,
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

#[test]
fn guardian_review_session_config_overrides_parent_developer_instructions() {
    let mut parent_config = test_config();
    parent_config.developer_instructions =
        Some("parent or managed config should not replace guardian policy".to_string());

    let guardian_config =
        build_guardian_review_session_config_for_test(&parent_config, None, "active-model", None)
            .expect("guardian config");

    assert_eq!(
        guardian_config.developer_instructions,
        Some(guardian_policy_prompt())
    );
}

#[test]
fn guardian_review_session_config_uses_live_network_proxy_state() {
    let mut parent_config = test_config();
    let mut parent_network = NetworkProxyConfig::default();
    parent_network.network.enabled = true;
    parent_network.network.allowed_domains = vec!["parent.example".to_string()];
    parent_config.permissions.network = Some(
        NetworkProxySpec::from_config_and_constraints(
            parent_network,
            None,
            parent_config.permissions.sandbox_policy.get(),
        )
        .expect("parent network proxy spec"),
    );

    let mut live_network = NetworkProxyConfig::default();
    live_network.network.enabled = true;
    live_network.network.allowed_domains = vec!["github.com".to_string()];

    let guardian_config = build_guardian_review_session_config_for_test(
        &parent_config,
        Some(live_network.clone()),
        "active-model",
        None,
    )
    .expect("guardian config");

    assert_eq!(
        guardian_config.permissions.network,
        Some(
            NetworkProxySpec::from_config_and_constraints(
                live_network,
                None,
                &SandboxPolicy::new_read_only_policy(),
            )
            .expect("live network proxy spec")
        )
    );
}

#[test]
fn guardian_review_session_config_rejects_pinned_collab_feature() {
    let mut parent_config = test_config();
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

    let err =
        build_guardian_review_session_config_for_test(&parent_config, None, "active-model", None)
            .expect_err("guardian config should fail when collab is pinned on");

    assert!(
        err.to_string()
            .contains("guardian review session requires `features.multi_agent` to be disabled")
    );
}

#[test]
fn guardian_review_session_config_uses_parent_active_model_instead_of_hardcoded_slug() {
    let mut parent_config = test_config();
    parent_config.model = Some("configured-model".to_string());

    let guardian_config =
        build_guardian_review_session_config_for_test(&parent_config, None, "active-model", None)
            .expect("guardian config");

    assert_eq!(guardian_config.model, Some("active-model".to_string()));
}

#[test]
fn guardian_review_session_config_uses_requirements_guardian_override() {
    let codex_home = tempfile::tempdir().expect("create temp dir");
    let workspace = tempfile::tempdir().expect("create temp dir");
    let config_layer_stack = ConfigLayerStack::new(
        Vec::new(),
        Default::default(),
        crate::config_loader::ConfigRequirementsToml {
            guardian_developer_instructions: Some(
                "  Use the workspace-managed guardian policy.  ".to_string(),
            ),
            ..Default::default()
        },
    )
    .expect("config layer stack");
    let parent_config = Config::load_config_with_layer_stack(
        ConfigToml::default(),
        ConfigOverrides {
            cwd: Some(workspace.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
        config_layer_stack,
    )
    .expect("load config");

    let guardian_config =
        build_guardian_review_session_config_for_test(&parent_config, None, "active-model", None)
            .expect("guardian config");

    assert_eq!(
        guardian_config.developer_instructions,
        Some("Use the workspace-managed guardian policy.".to_string())
    );
}

#[test]
fn guardian_review_session_config_uses_default_guardian_policy_without_requirements_override() {
    let codex_home = tempfile::tempdir().expect("create temp dir");
    let workspace = tempfile::tempdir().expect("create temp dir");
    let config_layer_stack =
        ConfigLayerStack::new(Vec::new(), Default::default(), Default::default())
            .expect("config layer stack");
    let parent_config = Config::load_config_with_layer_stack(
        ConfigToml::default(),
        ConfigOverrides {
            cwd: Some(workspace.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
        config_layer_stack,
    )
    .expect("load config");

    let guardian_config =
        build_guardian_review_session_config_for_test(&parent_config, None, "active-model", None)
            .expect("guardian config");

    assert_eq!(
        guardian_config.developer_instructions,
        Some(guardian_policy_prompt())
    );
}
