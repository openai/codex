use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageReference;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

use super::ConversationTranscriptConfig;
use super::ConversationTranscriptEntry;
use super::ConversationTranscriptEntryKind;
use super::ConversationTranscriptOptions;
use super::MANUAL_APPROVAL_DEVELOPER_PREFIX;
use super::TranscriptEntryLimits;
use crate::ContextSection;
use crate::ContextTarget;
use crate::SectionInput;
use crate::collect_transcript;
use crate::default_registry;
use crate::truncate_text;

fn transcript_config() -> ConversationTranscriptConfig {
    ConversationTranscriptConfig {
        options: ConversationTranscriptOptions::default(),
        entry_limits: TranscriptEntryLimits {
            message_tokens: 2_000,
            tool_tokens: 1_000,
            node_repl_output_tokens: 2_000,
        },
    }
}

#[test]
fn heartbeat_references_preserve_positions_changes_and_human_messages() {
    let make = |time: &str, instructions: &str, kind: &str| {
        serde_json::from_value::<ResponseItem>(serde_json::json!({
        "type": "message", "role": "user",
        "content": [{"type": "input_text", "text": format!("<heartbeat>\n  <automation_id>monitor</automation_id>\n  <current_time_iso>{time}</current_time_iso>\n  <instructions>\n{instructions}\n  </instructions>\n</heartbeat>\n")}],
        "internal_chat_message_metadata_passthrough": {"content_item_kinds": [kind]}
    })).unwrap()
    };
    let history = vec![
        make("01:00Z", "Monitor only.", "user.heartbeat"),
        make("01:30Z", "Monitor only.", "user.heartbeat"),
        make("01:40Z", "Create a worktree.", "user.text"),
        make("02:00Z", "Monitor only.", "user.heartbeat"),
        make("02:30Z", "Never create a worktree.", "user.heartbeat"),
        make("03:00Z", "Monitor only.", "user.heartbeat"),
        make("03:10Z", "Monitor only.", "user.text"),
    ];
    let entries = collect_transcript(&history, &transcript_config());
    assert_eq!(entries.len(), history.len());
    for index in [0, 2, 4, 5, 6] {
        let ResponseItem::Message { content, .. } = &history[index] else {
            unreachable!()
        };
        let ContentItem::InputText { text } = &content[0] else {
            unreachable!()
        };
        assert_eq!(
            entries[index],
            entry(ConversationTranscriptEntryKind::User, text)
        );
    }
    for index in [1, 3] {
        assert!(
            entries[index]
                .text
                .contains("unchanged from transcript entry [1]")
        );
        assert!(!entries[index].text.contains("Monitor only."));
    }
    // A rebuilt window starts with a full body, never a dangling old reference.
    let rebuilt = collect_transcript(&history[3..].to_vec(), &transcript_config());
    assert!(rebuilt[0].text.contains("Monitor only."));
    let interrupted = vec![
        history[0].clone(),
        make("02:00Z", &"x".repeat(/*n*/ 3_600), "user.heartbeat"),
        history[1].clone(),
    ];
    let interrupted = collect_transcript(&interrupted, &transcript_config());
    assert!(interrupted[2].text.contains("Monitor only."));
}

fn entry(kind: ConversationTranscriptEntryKind, text: &str) -> ConversationTranscriptEntry {
    ConversationTranscriptEntry {
        kind,
        text: text.to_string(),
        original_bytes: text.len(),
    }
}

#[test]
fn registered_transcript_filters_roles_and_preserves_node_repl_tool_attribution() {
    let approved_action = format!(
        "{MANUAL_APPROVAL_DEVELOPER_PREFIX}\nApproved action: {}",
        "exact action ".repeat(/*n*/ 1_000)
    );
    let history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Inspect the workspace.".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: "ordinary developer context".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: approved_action.clone(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "Inspection complete.".to_string(),
            }],
            phase: Some(MessagePhase::FinalAnswer),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "read_file".to_string(),
            namespace: Some("mcp__node_repl__".to_string()),
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
            encrypted_function_args: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text("file contents".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let registry = default_registry();
    let config = transcript_config();

    let sections = registry
        .collect(&SectionInput {
            target: ContextTarget::Async,
            history: &history,
            transcript: &config,
            root_conversation: &[],
            trusted_user_answers: &[],
            planned_action: None,
            permissions: None,
            previous_reviews: None,
            trusted_tool: None,
            trusted_skill_paths: &[],
            images: None,
            node_repl: None,
        })
        .expect("transcript collection should succeed");

    assert_eq!(
        sections,
        vec![ContextSection::ConversationTranscript {
            items: vec![
                entry(
                    ConversationTranscriptEntryKind::User,
                    "Inspect the workspace."
                ),
                entry(ConversationTranscriptEntryKind::Developer, &approved_action),
                entry(
                    ConversationTranscriptEntryKind::ProtectedAssistant,
                    "Inspection complete."
                ),
                entry(
                    ConversationTranscriptEntryKind::ToolCall("tool read_file call".to_string()),
                    "{}"
                ),
                entry(
                    ConversationTranscriptEntryKind::NodeReplToolOutput(
                        "tool read_file result".to_string()
                    ),
                    "file contents"
                ),
            ],
        }]
    );
    assert_eq!(
        sections,
        vec![ContextSection::ConversationTranscript {
            items: collect_transcript(&history, &config),
        }]
    );
}

#[test]
fn excluded_tool_calls_still_attribute_included_results() {
    let history = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "read_file".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
            encrypted_function_args: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text("file contents".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let config = ConversationTranscriptConfig {
        options: ConversationTranscriptOptions {
            include_tool_calls: false,
            ..ConversationTranscriptOptions::default()
        },
        ..transcript_config()
    };
    let sections = default_registry()
        .collect(&SectionInput {
            target: ContextTarget::Async,
            history: &history,
            transcript: &config,
            root_conversation: &[],
            trusted_user_answers: &[],
            planned_action: None,
            permissions: None,
            previous_reviews: None,
            trusted_tool: None,
            trusted_skill_paths: &[],
            images: None,
            node_repl: None,
        })
        .expect("transcript collection should succeed");

    assert_eq!(
        transcript_items(&sections[0]),
        vec![entry(
            ConversationTranscriptEntryKind::ToolOutput("tool read_file result".to_string()),
            "file contents"
        )]
    );
}

#[test]
fn outputs_with_call_ids_or_explicit_names_are_retained() {
    let output =
        |call_id: Option<&str>, name: Option<&str>, text: &str| ResponseItem::FunctionCallOutput {
            id: None,
            call_id: call_id.map(str::to_string),
            name: name.map(str::to_string),
            namespace: Some("slack".to_string()),
            output: FunctionCallOutputPayload::from_text(text.to_string()),
            internal_chat_message_metadata_passthrough: None,
        };
    let custom_output = |name: Option<&str>, text: &str| ResponseItem::CustomToolCallOutput {
        id: None,
        call_id: "missing-custom-call".to_string(),
        name: name.map(str::to_string),
        output: FunctionCallOutputPayload::from_text(text.to_string()),
        internal_chat_message_metadata_passthrough: None,
    };
    let shell_action = LocalShellAction::Exec(LocalShellExecAction {
        command: vec!["echo".to_string(), "hello".to_string()],
        timeout_ms: None,
        working_directory: None,
        env: None,
        user: None,
    });
    let shell_text = serde_json::to_string(&shell_action).unwrap();
    let history = [
        output(
            /*call_id*/ None,
            /*name*/ None,
            "anonymous output",
        ),
        output(
            Some("missing-call"),
            /*name*/ None,
            "orphaned function output",
        ),
        output(
            /*call_id*/ None,
            Some("notifications"),
            "named notification",
        ),
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: None,
            name: Some("notifications".to_string()),
            namespace: Some("slack".to_string()),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: "data:image/png;base64,image".to_string(),
                    },
                    detail: None,
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
        output(
            Some("missing-call"),
            Some("notifications"),
            "named orphaned function output",
        ),
        custom_output(/*name*/ None, "orphaned custom output"),
        custom_output(Some("notifications"), "named orphaned custom output"),
        ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell-1".to_string()),
            status: LocalShellStatus::Completed,
            action: shell_action,
            internal_chat_message_metadata_passthrough: None,
        },
        output(Some("shell-1"), /*name*/ None, "local shell output"),
    ];
    let named = entry(
        ConversationTranscriptEntryKind::ToolOutput("tool slack.notifications result".to_string()),
        "named notification",
    );
    let generic = |text| {
        entry(
            ConversationTranscriptEntryKind::ToolOutput("tool result".to_string()),
            text,
        )
    };
    let mut config = transcript_config();
    for target in [ContextTarget::Sync, ContextTarget::Async] {
        for include_tool_calls in [true, false] {
            config.options.include_tool_calls = include_tool_calls;
            let sections = default_registry()
                .collect(&SectionInput {
                    target,
                    history: &history,
                    transcript: &config,
                    root_conversation: &[],
                    trusted_user_answers: &[],
                    planned_action: None,
                    permissions: None,
                    previous_reviews: None,
                    trusted_tool: None,
                    trusted_skill_paths: &[],
                    images: None,
                    node_repl: None,
                })
                .expect("transcript collection should succeed");
            let mut expected = vec![
                generic("orphaned function output"),
                named.clone(),
                entry(
                    ConversationTranscriptEntryKind::ToolOutput(
                        "tool slack.notifications result".to_string(),
                    ),
                    "[non-text output]",
                ),
                generic("named orphaned function output"),
                generic("orphaned custom output"),
                generic("named orphaned custom output"),
            ];
            if include_tool_calls {
                expected.push(entry(
                    ConversationTranscriptEntryKind::ToolCall("tool shell call".to_string()),
                    &shell_text,
                ));
            }
            expected.push(generic("local shell output"));
            assert_eq!(
                sections,
                vec![ContextSection::ConversationTranscript { items: expected }]
            );
        }
    }
}

#[test]
fn reused_registry_applies_current_history_sources_and_entry_limits() {
    let text = "é🙂".repeat(/*n*/ 10_000);
    let mut history = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: text.clone() }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];
    let registry = default_registry();
    let mut config = transcript_config();
    for (target, message_tokens) in [(ContextTarget::Async, 80), (ContextTarget::Sync, 120)] {
        config.entry_limits.message_tokens = message_tokens;
        let sections = registry
            .collect(&SectionInput {
                target,
                history: &history,
                transcript: &config,
                root_conversation: &[],
                trusted_user_answers: &[],
                planned_action: None,
                permissions: None,
                previous_reviews: None,
                trusted_tool: None,
                trusted_skill_paths: &[],
                images: None,
                node_repl: None,
            })
            .expect("transcript collection should succeed");
        assert_eq!(
            transcript_items(&sections[0]),
            vec![ConversationTranscriptEntry {
                kind: ConversationTranscriptEntryKind::User,
                text: text.clone(),
                original_bytes: text.len(),
            }]
        );
    }

    history.push(ResponseItem::FunctionCall {
        id: None,
        name: "exec_command".to_string(),
        namespace: None,
        arguments: text.clone(),
        call_id: "call-1".to_string(),
        encrypted_function_args: None,
        internal_chat_message_metadata_passthrough: None,
    });
    config.entry_limits.message_tokens = 60;
    config.entry_limits.tool_tokens = 30;
    for include_tool_calls in [true, false] {
        config.options.include_tool_calls = include_tool_calls;
        let sections = registry
            .collect(&SectionInput {
                target: ContextTarget::Async,
                history: &history,
                transcript: &config,
                root_conversation: &[],
                trusted_user_answers: &[],
                planned_action: None,
                permissions: None,
                previous_reviews: None,
                trusted_tool: None,
                trusted_skill_paths: &[],
                images: None,
                node_repl: None,
            })
            .expect("transcript collection should succeed");
        let mut expected = vec![ConversationTranscriptEntry {
            kind: ConversationTranscriptEntryKind::User,
            text: text.clone(),
            original_bytes: text.len(),
        }];
        if include_tool_calls {
            expected.push(ConversationTranscriptEntry {
                kind: ConversationTranscriptEntryKind::ToolCall(
                    "tool exec_command call".to_string(),
                ),
                text: truncate_text(&text, /*max_tokens*/ 30),
                original_bytes: text.len(),
            });
        }
        assert_eq!(transcript_items(&sections[0]), expected);
    }
}

fn transcript_items(section: &ContextSection) -> &[ConversationTranscriptEntry] {
    let ContextSection::ConversationTranscript { items } = section else {
        panic!("expected transcript section");
    };
    items
}
