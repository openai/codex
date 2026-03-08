use super::*;
use crate::config::ManagedFeatures;
use crate::config::NetworkProxySpec;
use crate::config::test_config;
use crate::config_loader::FeatureRequirementsToml;
use crate::config_loader::NetworkConstraints;
use crate::config_loader::RequirementSource;
use crate::config_loader::Sourced;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::models::ContentItem;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::PathBuf;

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
    let action = serde_json::json!({
        "tool": "apply_patch",
        "cwd": PathBuf::from("/tmp"),
        "files": Vec::<String>::new(),
        "change_count": 1usize,
        "patch": "line\n".repeat(10_000),
    });

    let rendered = format_guardian_action_pretty(&action);
    let original_patch = action["patch"]
        .as_str()
        .expect("test patch should serialize as a string");

    assert!(rendered.contains("\"tool\": \"apply_patch\""));
    assert!(rendered.len() < original_patch.len());
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
