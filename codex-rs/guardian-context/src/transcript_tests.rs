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
    let raw = collect_transcript(&history, &transcript_config());
    let collected = crate::CollectedContext {
        sections: vec![ContextSection::ConversationTranscript { items: raw.clone() }],
    };
    let mut profile = crate::ContextProfile::asynchronous();
    profile.transcript_format = crate::TranscriptFormat::Json;
    let prepared = crate::prepare_context(
        std::borrow::Cow::Borrowed(&collected),
        &profile,
        /*entry_number_offset*/ 0,
        crate::ContextPresentation::Async,
        crate::ContextBudgeting::Fixed,
        &[],
    )
    .unwrap();
    let entries = prepared
        .sections
        .into_iter()
        .flat_map(|section| {
            let crate::composition::SectionDelivery::UserContent(items) = section.delivery else {
                panic!("expected transcript content");
            };
            items.into_iter().filter_map(|item| match item.content {
                crate::composition::SectionContent::Transcript(record) => Some(record),
                crate::composition::SectionContent::Other(_) => None,
            })
        })
        .zip(&raw)
        .map(|(record, entry)| ConversationTranscriptEntry {
            content: crate::TranscriptContent::Text(record.text().to_owned()),
            ..entry.clone()
        })
        .collect::<Vec<_>>();
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
        let crate::TranscriptContent::Text(text) = &entries[index].content else {
            panic!("text entry")
        };
        assert!(text.contains("unchanged from transcript entry [1]"));
        assert!(!text.contains("Monitor only."));
    }
    assert_eq!(collected.transcript_entries(), raw.as_slice());
    const DELTA_AFTER_HUMAN_MESSAGE: usize = 3;
    for offset in [0, 1, DELTA_AFTER_HUMAN_MESSAGE] {
        let [expected, actual] = [entries.clone(), raw.clone()].map(|items| {
            let presentation = if offset == 0 {
                crate::ContextPresentation::SyncFull {
                    session_id: "heartbeat",
                }
            } else {
                crate::ContextPresentation::SyncDelta {
                    session_id: "heartbeat",
                }
            };
            crate::prepare_context(
                std::borrow::Cow::Borrowed(&crate::CollectedContext {
                    sections: vec![crate::ContextSection::ConversationTranscript { items }],
                }),
                &crate::ContextProfile::synchronous(),
                offset,
                presentation,
                crate::ContextBudgeting::Fixed,
                &[],
            )
            .unwrap()
            .into_messages()
        });
        assert_eq!(actual, expected);
    }
    // Rebuilt and interrupted runs start with a full body, never an old reference.
    const OVERSIZED_HEARTBEAT_CHARS: usize = 3_600;
    for history in [
        history[DELTA_AFTER_HUMAN_MESSAGE..].to_vec(),
        vec![
            history[0].clone(),
            make(
                "02:00Z",
                &"x".repeat(OVERSIZED_HEARTBEAT_CHARS),
                "user.heartbeat",
            ),
            history[1].clone(),
        ],
    ] {
        let context = crate::CollectedContext {
            sections: vec![ContextSection::ConversationTranscript {
                items: collect_transcript(&history, &transcript_config()),
            }],
        };
        let prepared = crate::prepare_context(
            std::borrow::Cow::Owned(context),
            &crate::ContextProfile::asynchronous(),
            /*entry_number_offset*/ 0,
            crate::ContextPresentation::Async,
            crate::ContextBudgeting::Fixed,
            &[],
        )
        .unwrap();
        let text = serde_json::to_string(&prepared.into_messages()).unwrap();
        assert!(text.contains("Monitor only."));
        assert!(!text.contains("unchanged from transcript entry"));
    }
}

fn entry(kind: ConversationTranscriptEntryKind, text: &str) -> ConversationTranscriptEntry {
    ConversationTranscriptEntry {
        kind,
        content: crate::TranscriptContent::Text(text.to_string()),
        original_bytes: text.len(),
        retained_source: None,
    }
}

#[test]
fn registered_transcript_filters_roles_and_preserves_node_repl_tool_attribution() {
    let approved_action = format!(
        "{MANUAL_APPROVAL_DEVELOPER_PREFIX}\nApproved action: {}",
        "exact action ".repeat(/*n*/ 1_000)
    );
    let history = vec![
        serde_json::from_value(serde_json::json!({
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "Inspect the workspace."}],
            "internal_chat_message_metadata_passthrough": {
                "content_item_kinds": ["compaction.summary"]
            }
        }))
        .unwrap(),
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Inspect the workspace.".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: "ordinary developer context".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: approved_action.clone(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                annotations: None,
                logprobs: None,
                text: "The first result is ready.".to_string(),
            }],
            phase: Some(MessagePhase::PartialAnswer),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                annotations: None,
                logprobs: None,
                text: "Inspection complete.".to_string(),
            }],
            phase: Some(MessagePhase::FinalAnswer),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            status: None,
            encrypted_content: None,
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
            is_selected_exec_output: None,
            root_conversation: &[],
            root_conversation_candidates: None,
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
                    ConversationTranscriptEntryKind::Assistant,
                    "Inspect the workspace."
                ),
                entry(
                    ConversationTranscriptEntryKind::User,
                    "Inspect the workspace."
                ),
                entry(ConversationTranscriptEntryKind::Developer, &approved_action),
                entry(
                    ConversationTranscriptEntryKind::ProtectedAssistant,
                    "The first result is ready."
                ),
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
            status: None,
            encrypted_content: None,
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
            is_selected_exec_output: None,
            root_conversation: &[],
            root_conversation_candidates: None,
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
fn missing_exec_call_preserves_recorder_verified_output() {
    let outputs = ["verified", "unknown"].map(|call_id| ResponseItem::CustomToolCallOutput {
        id: None,
        call_id: call_id.to_owned(),
        name: None,
        output: FunctionCallOutputPayload::from_text("saved instructions".to_owned()),
        internal_chat_message_metadata_passthrough: None,
    });
    let is_verified = |output: &ResponseItem| matches!(output, ResponseItem::CustomToolCallOutput { call_id, .. } if call_id == "verified");
    let ordinary = entry(
        ConversationTranscriptEntryKind::ToolOutput("tool result".into()),
        "saved instructions",
    );
    assert_eq!(
        collect_transcript(&outputs, &transcript_config()),
        vec![ordinary.clone(), ordinary.clone()]
    );
    assert_eq!(
        super::collect_transcript_for_review(
            &outputs,
            &transcript_config(),
            /*proposed_call_id*/ None,
            Some(&is_verified)
        ),
        vec![
            entry(
                ConversationTranscriptEntryKind::SelectedExecOutput,
                "saved instructions"
            ),
            ordinary,
        ]
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
            encrypted_content: None,
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
                    is_selected_exec_output: None,
                    root_conversation: &[],
                    root_conversation_candidates: None,
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
fn reused_registry_applies_current_history_sources_and_preserves_originals() {
    let text = "é🙂".repeat(/*n*/ 10_000);
    let mut history = vec![ResponseItem::Message {
        status: None,
        encrypted_content: None,
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
                is_selected_exec_output: None,
                root_conversation: &[],
                root_conversation_candidates: None,
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
                content: crate::TranscriptContent::Text(text.clone()),
                original_bytes: text.len(),
                retained_source: None,
            }]
        );
    }

    history.push(ResponseItem::FunctionCall {
        status: None,
        encrypted_content: None,
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
    let mut assistant = history[0].clone();
    let ResponseItem::Message { role, .. } = &mut assistant else {
        unreachable!();
    };
    *role = "assistant".to_owned();
    history.push(assistant);
    for include_tool_calls in [true, false] {
        config.options.include_tool_calls = include_tool_calls;
        let sections = registry
            .collect(&SectionInput {
                target: ContextTarget::Async,
                history: &history,
                transcript: &config,
                is_selected_exec_output: None,
                root_conversation: &[],
                root_conversation_candidates: None,
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
            content: crate::TranscriptContent::Text(text.clone()),
            original_bytes: text.len(),
            retained_source: None,
        }];
        if include_tool_calls {
            expected.push(ConversationTranscriptEntry {
                kind: ConversationTranscriptEntryKind::ToolCall(
                    "tool exec_command call".to_string(),
                ),
                content: crate::TranscriptContent::Text(text.clone()),
                original_bytes: text.len(),
                retained_source: None,
            });
        }
        expected.push(ConversationTranscriptEntry {
            kind: ConversationTranscriptEntryKind::ProtectedAssistant,
            content: crate::TranscriptContent::Text(text.clone()),
            original_bytes: text.len(),
            retained_source: None,
        });
        assert_eq!(transcript_items(&sections[0]), expected);
    }
}

fn transcript_items(section: &ContextSection) -> &[ConversationTranscriptEntry] {
    let ContextSection::ConversationTranscript { items } = section else {
        panic!("expected transcript section");
    };
    items
}

#[test]
fn encrypted_messages_preserve_order_and_budget_for_both_reviewers() {
    use crate::ContextPresentation;
    use crate::ContextProfile;
    use crate::HistoryTruncation;
    use crate::RequestBudget;
    use codex_protocol::models::AgentMessageInputContent;
    let message = ResponseItem::AgentMessage {
        id: None,
        author: "/root".into(),
        recipient: "/root/worker".into(),
        content: vec![AgentMessageInputContent::EncryptedContent {
            encrypted_content: "opaque-parent-reply".repeat(/*n*/ 80),
        }],
        internal_chat_message_metadata_passthrough: None,
    };
    let call: ResponseItem = serde_json::from_value(serde_json::json!({
        "type": "function_call", "call_id": "call", "namespace": "team", "name": "send_message",
        "arguments": serde_json::json!({"target": "worker", "message": "opaque-parent-reply".repeat(80)}).to_string(),
        "encrypted_function_args": ["message"],
    })).unwrap();
    let user = ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: None,
        role: "user".into(),
        content: vec![ContentItem::InputText {
            text: "Update this page.".into(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    for mut profile in [
        ContextProfile::synchronous(),
        ContextProfile::asynchronous(),
    ] {
        let native_calls = profile.target == crate::ContextTarget::Sync;
        profile.transcript.options.native_multi_agent_namespace =
            native_calls.then(|| "team".to_owned());
        if native_calls {
            let mut limited = profile.clone();
            limited.transcript.entry_limits.tool_tokens = 1;
            let deferred = collect_transcript(&vec![call.clone()], &limited.transcript);
            assert!(
                matches!(&deferred[0].content, crate::TranscriptContent::NativeMessage(item)
                if item.as_ref() == &call)
            );
            let mut limited = profile.clone();
            // The call fits alone; its paired output must count toward retention too.
            limited.retention.max_tool_transcript_tokens = crate::estimate_input_tokens(&call);
            let entries = collect_transcript(&vec![call.clone()], &limited.transcript);
            assert!(
                limited
                    .prepare_transcript(&entries, /*entry_number_offset*/ 0)
                    .items
                    .is_empty()
            );
        }
        let mut history = vec![user.clone(), message.clone()];
        if native_calls {
            history.push(call.clone());
        }
        history.push(user.clone());
        // Move the native call outside the five most recent tool entries, which stay required.
        history.extend((0..5).map(|index| {
            serde_json::from_value(serde_json::json!({
                "type": "function_call", "call_id": format!("later-{index}"),
                "name": "exec_command", "arguments": r#"{"cmd":"pwd"}"#,
            }))
            .unwrap()
        }));
        let entries = collect_transcript(&history, &profile.transcript);
        let context = crate::CollectedContext {
            sections: vec![ContextSection::ConversationTranscript {
                items: entries.clone(),
            }],
        }
        .compose_prepared(
            ContextPresentation::SyncFull {
                session_id: "worker",
            },
            profile.prepare_transcript(&entries, /*entry_number_offset*/ 0),
        )
        .unwrap();
        assert_eq!(
            context
                .section_costs()
                .filter(|(name, _)| *name == "conversation_transcript")
                .count(),
            1
        );
        let messages = context.clone().into_messages();
        assert_eq!(messages[1], message);
        let mut budget = context.estimated_tokens() - crate::estimate_input_tokens(&message) + 100;
        if native_calls {
            assert_eq!(messages.len(), 5);
            assert_eq!(messages[2], call);
            assert!(
                matches!(&messages[3], ResponseItem::FunctionCallOutput { call_id: Some(id), .. } if id == "call")
            );
            budget -= crate::estimate_input_tokens(&call);
        } else {
            assert_eq!(messages.len(), 3);
        }
        // Native evidence and paired outputs must be evicted together.
        let reduced = context
            .enforce_budget(
                RequestBudget {
                    max_input_tokens: budget,
                    existing_context_tokens: 0,
                },
                "Evidence omitted.".into(),
                HistoryTruncation::Preserve,
            )
            .unwrap();
        assert!(reduced.estimated_tokens() <= budget);
        let reduced = serde_json::to_value(reduced.into_messages()).unwrap();
        assert!(reduced.to_string().contains("Evidence omitted."));
        assert!(!reduced.to_string().contains("opaque-parent-reply"));
        assert!(
            reduced
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["type"] != "function_call_output")
        );
    }
}

#[test]
fn collect_guardian_transcript_entries_includes_recent_tool_calls_and_output() {
    let mut profile = crate::ContextProfile::synchronous();
    let mut items = vec![
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "check the repo".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            status: None,
            encrypted_content: None,
            id: None,
            name: "read_file".to_string(),
            namespace: None,
            arguments: "{\"path\":\"README.md\"}".to_string(),
            call_id: "call-1".to_string(),
            encrypted_function_args: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: codex_protocol::models::FunctionCallOutputPayload::from_text(
                "repo is public".to_string(),
            ),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                annotations: None,
                logprobs: None,
                text: "I need to push a fix".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let entries = collect_transcript(&items, &profile.transcript);

    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries[1],
        ConversationTranscriptEntry {
            kind: ConversationTranscriptEntryKind::ToolCall("tool read_file call".to_string()),
            content: crate::TranscriptContent::Text("{\"path\":\"README.md\"}".to_string()),
            original_bytes: "{\"path\":\"README.md\"}".len(),
            retained_source: None,
        }
    );
    assert_eq!(
        entries[2],
        ConversationTranscriptEntry {
            kind: ConversationTranscriptEntryKind::ToolOutput("tool read_file result".to_string()),
            content: crate::TranscriptContent::Text("repo is public".to_string()),
            original_bytes: "repo is public".len(),
            retained_source: None,
        }
    );
    if let ResponseItem::FunctionCall { namespace, .. } = &mut items[1] {
        *namespace = Some("mcp__node_repl__".to_string());
    }
    assert!(matches!(
        collect_transcript(&items, &profile.transcript)[2].kind,
        ConversationTranscriptEntryKind::NodeReplToolOutput(_)
    ));

    let oversized_result = "é🙂".repeat(/*n*/ 10_000);
    if let ResponseItem::FunctionCallOutput { output, .. } = &mut items[2] {
        *output =
            codex_protocol::models::FunctionCallOutputPayload::from_text(oversized_result.clone());
    }
    for token_cap in [profile.transcript.entry_limits.tool_tokens, 6_000] {
        profile.transcript.entry_limits.node_repl_output_tokens = token_cap;
        let entries = collect_transcript(&items, &profile.transcript);
        assert_eq!(
            entries[2],
            ConversationTranscriptEntry {
                kind: ConversationTranscriptEntryKind::NodeReplToolOutput(
                    "tool read_file result".to_string()
                ),
                content: crate::TranscriptContent::Text(oversized_result.clone()),
                original_bytes: oversized_result.len(),
                retained_source: None,
            }
        );
        assert_eq!(entries.len(), 4);
        let prepared = profile.prepare_transcript(&entries, /*entry_number_offset*/ 0);
        let rendered = prepared
            .items
            .into_iter()
            .map(|item| match item.content {
                crate::TranscriptContent::Text(text) => text,
                crate::TranscriptContent::Record(record) => record.to_string(),
                crate::TranscriptContent::ScheduledMessage(_)
                | crate::TranscriptContent::NativeMessage(_) => {
                    panic!("expected text transcript")
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            (rendered, prepared.omission_note),
            (
                vec![
                    "[1] user: check the repo".to_string(),
                    "[2] tool read_file call: {\"path\":\"README.md\"}".to_string(),
                    format!(
                        "[3] tool read_file result: {}",
                        truncate_text(&oversized_result, token_cap)
                    ),
                    "[4] assistant: I need to push a fix".to_string(),
                ],
                None,
            )
        );
    }
}
