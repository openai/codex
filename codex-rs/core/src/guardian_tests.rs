use super::*;
use crate::config::ManagedFeatures;
use crate::config::NetworkProxySpec;
use crate::config::test_config;
use crate::config_loader::FeatureRequirementsToml;
use crate::config_loader::NetworkConstraints;
use crate::config_loader::RequirementSource;
use crate::config_loader::Sourced;
use crate::test_support;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::config_types::ApprovalReviewPolicy;
use codex_protocol::models::ContentItem;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use insta::Settings;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

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

    let truncated = guardian_truncate_text(&content, 20);

    assert!(truncated.starts_with("prefix"));
    assert!(truncated.contains("<guardian_truncated omitted_approx_tokens=\""));
    assert!(truncated.ends_with("suffix"));
}

#[test]
fn format_guardian_action_pretty_truncates_large_string_fields() {
    let patch = "line\n".repeat(10_000);
    let action = GuardianApprovalRequest::ApplyPatch {
        id: "patch-1".to_string(),
        cwd: PathBuf::from("/tmp"),
        files: Vec::new(),
        changes: HashMap::new(),
        change_count: 1usize,
        patch: patch.clone(),
    };

    let rendered = format_guardian_action_pretty(&action);

    assert!(rendered.contains("\"tool\": \"apply_patch\""));
    assert!(rendered.len() < patch.len());
}

#[test]
fn guardian_review_status_message_includes_reviewed_command() {
    let action = GuardianApprovalRequest::Shell {
        id: "shell-1".to_string(),
        command: vec![
            "rm".to_string(),
            "-rf".to_string(),
            "/tmp/guardian target".to_string(),
        ],
        cwd: PathBuf::from("/tmp"),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions: None,
        justification: None,
    };

    assert_eq!(
        guardian_review_status_message(&action),
        "Reviewing approval request: rm -rf '/tmp/guardian target'"
    );
}

#[test]
fn guardian_approval_request_to_json_renders_mcp_tool_call_shape() {
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
        guardian_approval_request_to_json(&action),
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
}

#[test]
fn guardian_assessment_action_value_redacts_apply_patch_patch_text() {
    let action = GuardianApprovalRequest::ApplyPatch {
        id: "patch-1".to_string(),
        cwd: PathBuf::from("/tmp"),
        files: vec![AbsolutePathBuf::try_from("/tmp/guardian.txt").expect("absolute path")],
        changes: HashMap::new(),
        change_count: 1usize,
        patch: "*** Begin Patch\n*** Update File: guardian.txt\n@@\n+secret\n*** End Patch"
            .to_string(),
    };

    assert_eq!(
        guardian_assessment_action_value(&action),
        serde_json::json!({
            "tool": "apply_patch",
            "cwd": "/tmp",
            "files": ["/tmp/guardian.txt"],
            "change_count": 1,
        })
    );
}

#[tokio::test]
async fn routes_approval_to_guardian_requires_auto_only_review_policy() {
    let (_session, mut turn) = crate::codex::make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.approval_review_policy = ApprovalReviewPolicy::ManualOnly;
    turn.config = Arc::new(config.clone());

    assert!(!routes_approval_to_guardian(&turn));

    config.approval_review_policy = ApprovalReviewPolicy::AutoOnly;
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
    let mut config = (*turn.config).clone();
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
    .await;

    let assessment = run_guardian_subagent(
        Arc::clone(&session),
        Arc::clone(&turn),
        prompt,
        guardian_output_schema(),
        CancellationToken::new(),
    )
    .await?;
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
                &ContextSnapshotOptions::default(),
            )
        );
    });

    Ok(())
}
#[test]
fn guardian_subagent_config_preserves_parent_network_proxy() {
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

    let guardian_config = build_guardian_subagent_config(
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
fn guardian_subagent_config_uses_live_network_proxy_state() {
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

    let guardian_config = build_guardian_subagent_config(
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
fn guardian_subagent_config_rejects_pinned_collab_feature() {
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

    let err = build_guardian_subagent_config(&parent_config, None, "active-model", None)
        .expect_err("guardian config should fail when collab is pinned on");

    assert!(
        err.to_string()
            .contains("guardian subagent requires `features.multi_agent` to be disabled")
    );
}

#[test]
fn guardian_subagent_config_uses_parent_active_model_instead_of_hardcoded_slug() {
    let mut parent_config = test_config();
    parent_config.model = Some("configured-model".to_string());

    let guardian_config =
        build_guardian_subagent_config(&parent_config, None, "active-model", None)
            .expect("guardian config");

    assert_eq!(guardian_config.model, Some("active-model".to_string()));
}
