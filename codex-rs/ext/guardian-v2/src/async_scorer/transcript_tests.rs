use codex_extension_api::ResponseItem;
use codex_protocol::AgentPath;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::TruncationPolicy;
use core_test_support::responses::user_message_item;
use pretty_assertions::assert_eq;

use super::MANUAL_APPROVAL_DEVELOPER_PREFIX;
use super::MAX_MESSAGE_ENTRY_TOKENS;
use super::MAX_MESSAGE_TRANSCRIPT_TOKENS;
use super::MAX_TOOL_ENTRY_TOKENS;
use super::MAX_TOOL_TRANSCRIPT_TOKENS;
use super::TranscriptConfig;
use super::TranscriptSource;
use super::truncate_entry;

fn assistant_message(text: impl Into<String>, phase: MessagePhase) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText { text: text.into() }],
        phase: Some(phase),
        internal_chat_message_metadata_passthrough: None,
    }
}

#[test]
fn transcript_keeps_conversation_and_configured_sources() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Inspect the workspace.".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "exec_command".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            encrypted_function_args: None,
            call_id: "call-1".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text("Workspace inspected.".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Reasoning {
            id: None,
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Review the current files.".to_string(),
            }],
            content: Some(vec![ReasoningItemContent::ReasoningText {
                text: "Plaintext reasoning.".to_string(),
            }]),
            encrypted_content: Some("encrypted-reasoning-blob".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: Vec::new(),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items).entries;
    assert_eq!(
        transcript,
        vec![
            "[1] user: Inspect the workspace.\n",
            "[2] tool exec_command call: {}\n",
            "[3] tool exec_command result: Workspace inspected.\n",
        ]
    );

    let output_and_reasoning = TranscriptConfig {
        sources: vec![TranscriptSource::ToolOutputs, TranscriptSource::Reasoning],
        ..TranscriptConfig::default()
    };

    let transcript = output_and_reasoning.build(&items).entries;
    assert_eq!(
        transcript,
        vec![
            "[1] user: Inspect the workspace.\n",
            "[2] tool exec_command result: Workspace inspected.\n",
            "[3] reasoning: Review the current files.\nPlaintext reasoning.\n",
        ]
    );

    let calls_only = TranscriptConfig {
        sources: vec![TranscriptSource::ToolCalls],
        ..TranscriptConfig::default()
    };

    let transcript = calls_only.build(&items).entries;
    assert_eq!(
        transcript,
        vec![
            "[1] user: Inspect the workspace.\n",
            "[2] tool exec_command call: {}\n",
        ]
    );
}

#[test]
fn transcript_truncates_oversized_entries_without_splitting_characters() {
    let prefix = "start é";
    let suffix = "é end";
    let oversized_message = format!(
        "{prefix}{}{suffix}",
        "é".repeat(TruncationPolicy::Tokens(MAX_MESSAGE_ENTRY_TOKENS).byte_budget())
    );
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: oversized_message,
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "latest response".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items).entries;

    assert_eq!(transcript.len(), 2);
    let user_entry = &transcript[0];
    assert!(user_entry.starts_with(&format!("[1] user: {prefix}")));
    assert!(user_entry.contains("<truncated omitted_approx_tokens=\""));
    assert!(user_entry.ends_with(&format!("{suffix}\n")));
    assert!(
        user_entry.len()
            <= TruncationPolicy::Tokens(MAX_MESSAGE_ENTRY_TOKENS).byte_budget()
                + "[1] user: \n".len()
    );
    assert_eq!(transcript[1], "[2] assistant: latest response\n");
}

#[test]
fn transcript_preserves_first_and_latest_user_messages_and_recent_history() {
    let oversized_user_message = "authorization ".repeat(1_000);
    let mut items = (0..8)
        .map(|index| ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("user turn {index}: {oversized_user_message}"),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        })
        .collect::<Vec<_>>();
    items.push(ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: "Most recent assistant context.".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    });

    let transcript = TranscriptConfig::default().build(&items).entries;

    assert!(transcript[0].starts_with("[1] user: user turn 0:"));
    assert!(
        transcript
            .iter()
            .any(|entry| entry.starts_with("[8] user: user turn 7:"))
    );
    assert!(
        !transcript
            .iter()
            .any(|entry| entry.contains("user turn 1:"))
    );
    assert!(
        transcript
            .iter()
            .any(|entry| entry.contains("user turn 6:"))
    );
    assert_eq!(
        transcript.last().map(String::as_str),
        Some("[9] assistant: Most recent assistant context.\n")
    );
    assert!(
        transcript
            .iter()
            .map(|entry| TruncationPolicy::Bytes(entry.len()).token_budget())
            .sum::<usize>()
            <= MAX_MESSAGE_TRANSCRIPT_TOKENS
    );
}

#[test]
fn transcript_preserves_user_restrictions_before_final_assistant_messages() {
    let first_user_entry = "[1] user: Find a flight to New York.\n";
    let restriction_entry = "[2] user: Never book anything above $300.\n";
    let latest_user_entry = "[4] user: Yes.\n";
    let message_budget = [first_user_entry, restriction_entry, latest_user_entry]
        .iter()
        .map(|entry| TruncationPolicy::Bytes(entry.len()).token_budget())
        .sum();
    let items = vec![
        user_message_item("Find a flight to New York."),
        user_message_item("Never book anything above $300."),
        assistant_message(
            "I found a $450 flight. Should I book it?",
            MessagePhase::FinalAnswer,
        ),
        user_message_item("Yes."),
    ];

    let transcript = TranscriptConfig {
        max_message_transcript_tokens: message_budget,
        ..TranscriptConfig::default()
    }
    .build(&items)
    .entries;

    assert_eq!(
        transcript,
        vec![first_user_entry, restriction_entry, latest_user_entry]
    );
}

#[test]
fn transcript_preserves_recent_tool_evidence_when_protected_messages_fill_entry_limit() {
    let mut items = vec![user_message_item("Inspect the workspace.")];
    items.extend((0..4).map(|index| {
        assistant_message(format!("final answer {index}"), MessagePhase::FinalAnswer)
    }));
    items.push(ResponseItem::FunctionCall {
        id: None,
        name: "exec_command".to_string(),
        namespace: None,
        arguments: "recent evidence".to_string(),
        encrypted_function_args: None,
        call_id: "call-1".to_string(),
        internal_chat_message_metadata_passthrough: None,
    });

    let transcript = TranscriptConfig {
        max_recent_non_user_entries: 4,
        ..TranscriptConfig::default()
    }
    .build(&items);

    assert_eq!(
        transcript.entries,
        vec![
            "[1] user: Inspect the workspace.\n",
            "[4] assistant: final answer 2\n",
            "[5] assistant: final answer 3\n",
            "[6] tool exec_command call: recent evidence\n",
        ]
    );
    assert_eq!(
        transcript
            .truncations
            .iter()
            .map(|observation| (observation.component, observation.retained_bytes))
            .collect::<Vec<_>>(),
        vec![("transcript_message", 0); 2]
    );
}

#[test]
fn transcript_reserves_five_recent_tool_entries_from_protected_messages() {
    let mut items = vec![user_message_item("Inspect the workspace.")];
    items.extend((0..39).map(|index| {
        assistant_message(format!("final answer {index}"), MessagePhase::FinalAnswer)
    }));
    for index in 0..3 {
        let call_id = format!("call-{index}");
        items.push(ResponseItem::FunctionCall {
            id: None,
            name: "exec_command".to_string(),
            namespace: None,
            arguments: format!("click {index}"),
            encrypted_function_args: None,
            call_id: call_id.clone(),
            internal_chat_message_metadata_passthrough: None,
        });
        items.push(ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some(call_id),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text(format!("page state {index}")),
            internal_chat_message_metadata_passthrough: None,
        });
    }
    items.extend((0..20).map(|index| {
        assistant_message(
            format!("new final answer {index}"),
            MessagePhase::FinalAnswer,
        )
    }));

    let transcript = TranscriptConfig::default().build(&items).entries;
    let tool_entries = transcript
        .iter()
        .filter(|entry| entry.contains("tool exec_command "))
        .map(String::as_str)
        .collect::<Vec<_>>();

    assert_eq!(
        tool_entries,
        vec![
            "[42] tool exec_command result: page state 0\n",
            "[43] tool exec_command call: click 1\n",
            "[44] tool exec_command result: page state 1\n",
            "[45] tool exec_command call: click 2\n",
            "[46] tool exec_command result: page state 2\n",
        ]
    );
}

#[test]
fn rejected_commentary_does_not_evict_retained_message_evidence() {
    let protected_entry =
        "[1] assistant: Previously approved a limited action after confirming authorization.\n";
    let retained_entry = "[2] assistant: ok\n";
    let message_budget = [protected_entry, retained_entry]
        .iter()
        .map(|entry| TruncationPolicy::Bytes(entry.len()).token_budget())
        .sum();
    let items = vec![
        assistant_message(
            "Previously approved a limited action after confirming authorization.",
            MessagePhase::FinalAnswer,
        ),
        assistant_message("ok", MessagePhase::Commentary),
        assistant_message(
            "Additional commentary cannot fit.",
            MessagePhase::Commentary,
        ),
    ];

    let transcript = TranscriptConfig {
        max_message_transcript_tokens: message_budget,
        ..TranscriptConfig::default()
    }
    .build(&items)
    .entries;

    assert_eq!(transcript, vec![protected_entry, retained_entry]);
}

#[test]
fn transcript_evicts_protected_messages_in_cacheable_chunks() {
    let user_entry = "[1] user: Inspect the workspace.\n";
    let assistant_entry = "[2] assistant: final answer 0\n";
    let message_budget = TruncationPolicy::Bytes(user_entry.len()).token_budget()
        + 4 * TruncationPolicy::Bytes(assistant_entry.len()).token_budget();
    let configurations = [
        TranscriptConfig {
            max_recent_non_user_entries: 4,
            ..TranscriptConfig::default()
        },
        TranscriptConfig {
            max_message_transcript_tokens: message_budget,
            ..TranscriptConfig::default()
        },
    ];

    for config in configurations {
        let build_transcript = |message_count| {
            let mut items = vec![user_message_item("Inspect the workspace.")];
            items.extend((0..message_count).map(|index| {
                assistant_message(format!("final answer {index}"), MessagePhase::FinalAnswer)
            }));
            config.build(&items).entries
        };

        assert_eq!(
            build_transcript(5),
            vec![
                user_entry,
                "[4] assistant: final answer 2\n",
                "[5] assistant: final answer 3\n",
                "[6] assistant: final answer 4\n",
            ]
        );
        assert_eq!(
            build_transcript(6),
            vec![
                user_entry,
                "[4] assistant: final answer 2\n",
                "[5] assistant: final answer 3\n",
                "[6] assistant: final answer 4\n",
                "[7] assistant: final answer 5\n",
            ]
        );
        assert_eq!(
            build_transcript(7),
            vec![
                user_entry,
                "[6] assistant: final answer 4\n",
                "[7] assistant: final answer 5\n",
                "[8] assistant: final answer 6\n",
            ]
        );
    }
}

#[test]
fn transcript_preserves_latest_final_when_reserved_tools_fill_entry_window() {
    let mut items = vec![
        user_message_item("Find a flight to New York."),
        assistant_message("Earlier booking details.", MessagePhase::FinalAnswer),
        assistant_message(
            "I found a $450 flight. Should I book it?",
            MessagePhase::FinalAnswer,
        ),
        user_message_item("Yes."),
    ];
    items.extend((0..3).map(|index| ResponseItem::FunctionCall {
        id: None,
        name: "exec_command".to_string(),
        namespace: None,
        arguments: format!("recent evidence {index}"),
        encrypted_function_args: None,
        call_id: format!("call-{index}"),
        internal_chat_message_metadata_passthrough: None,
    }));

    let transcript = TranscriptConfig {
        max_recent_non_user_entries: 4,
        ..TranscriptConfig::default()
    }
    .build(&items)
    .entries;

    assert_eq!(
        transcript,
        vec![
            "[1] user: Find a flight to New York.\n",
            "[3] assistant: I found a $450 flight. Should I book it?\n",
            "[4] user: Yes.\n",
            "[5] tool exec_command call: recent evidence 0\n",
            "[6] tool exec_command call: recent evidence 1\n",
            "[7] tool exec_command call: recent evidence 2\n",
        ]
    );
}

#[test]
fn transcript_does_not_protect_legacy_inter_agent_instructions() {
    let communication = InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::root()
            .join("worker")
            .expect("worker path should be valid"),
        Vec::new(),
        "Review the pending action.".to_string(),
        /*trigger_turn*/ true,
    );
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: serde_json::to_string(&communication)
                    .expect("legacy inter-agent communication should serialize"),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "I found a $450 flight. Should I book it?".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "exec_command".to_string(),
            namespace: None,
            arguments: "booking details".to_string(),
            encrypted_function_args: None,
            call_id: "call-1".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig {
        max_recent_non_user_entries: 2,
        ..TranscriptConfig::default()
    }
    .build(&items)
    .entries;

    assert_eq!(
        transcript,
        vec![
            "[2] assistant: I found a $450 flight. Should I book it?\n",
            "[3] tool exec_command call: booking details\n",
        ]
    );
}

#[test]
fn transcript_reserves_separate_budget_for_recent_tool_evidence() {
    let mut items = (0..8)
        .map(|index| ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("user turn {index}: {}", "authorization ".repeat(1_000)),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        })
        .collect::<Vec<_>>();
    items.extend((0..12).map(|index| ResponseItem::FunctionCall {
        id: None,
        name: "exec_command".to_string(),
        namespace: None,
        arguments: format!("tool evidence {index}: {}", "signal ".repeat(1_000)),
        encrypted_function_args: None,
        call_id: format!("call-{index}"),
        internal_chat_message_metadata_passthrough: None,
    }));

    let transcript = TranscriptConfig::default().build(&items).entries;

    assert!(transcript[0].contains("user turn 0:"));
    assert!(
        transcript
            .iter()
            .any(|entry| entry.contains("user turn 7:"))
    );
    assert!(
        transcript.last().is_some_and(
            |entry| entry.starts_with("[20] tool exec_command call: tool evidence 11:")
        )
    );
    assert!(
        !transcript
            .iter()
            .any(|entry| entry.contains("tool evidence 0:"))
    );
    let tool_entries = transcript
        .iter()
        .filter(|entry| entry.contains("tool exec_command call:"))
        .collect::<Vec<_>>();
    assert!(tool_entries.len() < 12);
    assert!(tool_entries.iter().all(|entry| {
        entry.len()
            <= TruncationPolicy::Tokens(MAX_TOOL_ENTRY_TOKENS).byte_budget()
                + "[20] tool exec_command call: \n".len()
    }));
    assert!(
        tool_entries
            .iter()
            .map(|entry| TruncationPolicy::Bytes(entry.len()).token_budget())
            .sum::<usize>()
            <= MAX_TOOL_TRANSCRIPT_TOKENS
    );

    let first_retained_tool = tool_entries
        .first()
        .expect("at least one tool entry should be retained")
        .to_string();
    items.push(ResponseItem::FunctionCall {
        id: None,
        name: "exec_command".to_string(),
        namespace: None,
        arguments: format!("tool evidence 12: {}", "signal ".repeat(1_000)),
        encrypted_function_args: None,
        call_id: "call-12".to_string(),
        internal_chat_message_metadata_passthrough: None,
    });
    let next_transcript = TranscriptConfig::default().build(&items).entries;
    let next_first_retained_tool = next_transcript
        .iter()
        .find(|entry| entry.contains("tool exec_command call:"))
        .expect("at least one tool entry should be retained");

    assert_eq!(next_first_retained_tool, &first_retained_tool);
    assert!(
        next_transcript
            .iter()
            .filter(|entry| entry.contains("tool exec_command call:"))
            .map(|entry| TruncationPolicy::Bytes(entry.len()).token_budget())
            .sum::<usize>()
            <= MAX_TOOL_TRANSCRIPT_TOKENS
    );
}

#[test]
fn transcript_preserves_newest_manual_approval_when_message_budget_overflows() {
    let older_text = "older assistant evidence ".repeat(20);
    let approval_text = format!(
        "{MANUAL_APPROVAL_DEVELOPER_PREFIX} {}",
        "approval details ".repeat(20)
    );
    let older_entry = format!("[1] assistant: {older_text}\n");
    let approval_entry = format!("[2] developer: {approval_text}\n");
    let message_budget = TruncationPolicy::Bytes(older_entry.len())
        .token_budget()
        .max(TruncationPolicy::Bytes(approval_entry.len()).token_budget());
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText { text: older_text }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: approval_text,
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig {
        max_message_transcript_tokens: message_budget,
        ..TranscriptConfig::default()
    }
    .build(&items)
    .entries;

    assert_eq!(transcript, vec![approval_entry]);
}

#[test]
fn rejected_message_does_not_evict_retained_tool_entries() {
    let user_text = "Inspect the workspace.";
    let user_entry = format!("[1] user: {user_text}\n");
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: user_text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];
    items.extend((0..40).map(|index| ResponseItem::FunctionCall {
        id: None,
        name: "exec_command".to_string(),
        namespace: None,
        arguments: format!("tool evidence {index}"),
        encrypted_function_args: None,
        call_id: format!("call-{index}"),
        internal_chat_message_metadata_passthrough: None,
    }));
    items.push(ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: "This message has no remaining budget.".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    });
    let mut expected = vec![user_entry.clone()];
    expected.extend((0..40).map(|index| {
        format!(
            "[{}] tool exec_command call: tool evidence {index}\n",
            index + 2
        )
    }));

    let transcript = TranscriptConfig {
        max_message_transcript_tokens: TruncationPolicy::Bytes(user_entry.len()).token_budget(),
        ..TranscriptConfig::default()
    }
    .build(&items)
    .entries;

    assert_eq!(transcript, expected);
}

#[test]
fn transcript_evicts_non_user_entries_in_cacheable_chunks() {
    let config = TranscriptConfig {
        max_recent_non_user_entries: 4,
        ..TranscriptConfig::default()
    };
    let build_transcript = |tool_call_count| {
        let mut items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Inspect the workspace.".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }];
        items.extend(
            (0..tool_call_count).map(|index| ResponseItem::FunctionCall {
                id: None,
                name: "exec_command".to_string(),
                namespace: None,
                arguments: format!("tool evidence {index}"),
                encrypted_function_args: None,
                call_id: format!("call-{index}"),
                internal_chat_message_metadata_passthrough: None,
            }),
        );
        config.build(&items).entries
    };

    let first_overflow = build_transcript(5);
    let next_transcript = build_transcript(6);
    let second_overflow = build_transcript(7);

    assert_eq!(
        first_overflow,
        vec![
            "[1] user: Inspect the workspace.\n",
            "[4] tool exec_command call: tool evidence 2\n",
            "[5] tool exec_command call: tool evidence 3\n",
            "[6] tool exec_command call: tool evidence 4\n",
        ]
    );
    assert_eq!(
        next_transcript,
        vec![
            "[1] user: Inspect the workspace.\n",
            "[4] tool exec_command call: tool evidence 2\n",
            "[5] tool exec_command call: tool evidence 3\n",
            "[6] tool exec_command call: tool evidence 4\n",
            "[7] tool exec_command call: tool evidence 5\n",
        ]
    );
    assert_eq!(
        second_overflow,
        vec![
            "[1] user: Inspect the workspace.\n",
            "[6] tool exec_command call: tool evidence 4\n",
            "[7] tool exec_command call: tool evidence 5\n",
            "[8] tool exec_command call: tool evidence 6\n",
        ]
    );
}

#[test]
fn transcript_truncates_tool_results_using_standard_budget() {
    let output = format!("first {} last", "evidence ".repeat(5_000));
    let items = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "node_repl__run".to_owned(),
            namespace: None,
            arguments: "{}".to_owned(),
            encrypted_function_args: None,
            call_id: "call-1".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_owned()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text(output),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items).entries;
    let result = transcript
        .iter()
        .find(|entry| entry.contains(" result: "))
        .expect("the tool result should be retained");
    let prefix = "[2] tool node_repl__run result: ";

    assert!(result.starts_with(&format!("{prefix}first ")));
    assert!(result.contains("<truncated omitted_approx_tokens=\""));
    assert!(result.ends_with(" last\n"));
    assert!(
        result.len()
            <= TruncationPolicy::Tokens(MAX_TOOL_ENTRY_TOKENS).byte_budget() + prefix.len() + 1
    );
}

#[test]
fn transcript_preserves_named_unpaired_tool_source() {
    let mut items = vec![ResponseItem::FunctionCallOutput {
        id: None,
        call_id: None,
        name: Some("notifications".to_owned()),
        namespace: Some("slack".to_owned()),
        output: FunctionCallOutputPayload::from_text("new message".to_owned()),
        internal_chat_message_metadata_passthrough: None,
    }];

    assert_eq!(
        TranscriptConfig::default().build(&items).entries,
        vec!["[1] tool slack.notifications result: new message\n"]
    );

    if let ResponseItem::FunctionCallOutput { output, .. } = &mut items[0] {
        *output = FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,image".to_owned(),
                detail: None,
            },
        ]);
    }
    assert_eq!(
        TranscriptConfig::default().build(&items).entries,
        vec!["[1] tool slack.notifications result: [non-text output]\n"]
    );
}

#[test]
fn configured_reasoning_counts_against_message_budget() {
    let mut items = (0..8)
        .map(|index| ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("user turn {index}: {}", "authorization ".repeat(1_000)),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        })
        .collect::<Vec<_>>();
    items.push(ResponseItem::Reasoning {
        id: None,
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "Recent reasoning evidence.".to_string(),
        }],
        content: None,
        encrypted_content: None,
        internal_chat_message_metadata_passthrough: None,
    });

    let transcript = TranscriptConfig {
        sources: vec![TranscriptSource::Reasoning],
        ..TranscriptConfig::default()
    }
    .build(&items)
    .entries;

    assert!(transcript[0].contains("user turn 0:"));
    assert!(
        transcript
            .iter()
            .any(|entry| entry.contains("user turn 7:"))
    );
    assert_eq!(
        transcript.last().map(String::as_str),
        Some("[9] reasoning: Recent reasoning evidence.\n")
    );
    assert!(
        transcript
            .iter()
            .map(|entry| TruncationPolicy::Bytes(entry.len()).token_budget())
            .sum::<usize>()
            <= MAX_MESSAGE_TRANSCRIPT_TOKENS
    );
}

#[test]
fn truncate_entry_preserves_prefix_suffix_and_utf8_boundaries() {
    let text = format!("prefix é{}é suffix", "🦀".repeat(2_000));
    let truncated = truncate_entry(&text, /*max_tokens*/ 200);

    assert!(truncated.starts_with("prefix é"));
    assert!(truncated.contains("<truncated omitted_approx_tokens=\""));
    assert!(truncated.ends_with("é suffix"));
    assert!(truncated.len() <= TruncationPolicy::Tokens(200).byte_budget());
}

#[test]
fn transcript_keeps_only_manual_approval_developer_messages() {
    let approval_text = format!("{MANUAL_APPROVAL_DEVELOPER_PREFIX}\n\nApproved action:\n{{}}");
    let items = vec![
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
                text: approval_text.clone(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items).entries;
    assert_eq!(
        transcript,
        vec![format!("[1] developer: {approval_text}\n")]
    );
}

#[test]
fn transcript_omits_media_payloads_and_keeps_readable_content() {
    let oversized_image =
        "A".repeat(TruncationPolicy::Tokens(MAX_MESSAGE_TRANSCRIPT_TOKENS).byte_budget() + 1);
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "Review this screenshot.".to_string(),
                },
                ContentItem::InputImage {
                    image_url: format!("data:image/png;base64,{oversized_image}"),
                    detail: None,
                },
                ContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,audio-payload".to_string(),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "Screenshot captured.".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,tool-image".to_string(),
                    detail: None,
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,tool-audio".to_string(),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ImageGenerationCall {
            id: None,
            status: "completed".to_string(),
            revised_prompt: Some("A screenshot.".to_string()),
            result: oversized_image,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("search-1".to_string()),
            status: Some("completed".to_string()),
            execution: "client".to_string(),
            arguments: serde_json::json!({ "query": "Find screenshot tools" }),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ToolSearchOutput {
            id: None,
            call_id: Some("search-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![serde_json::json!({ "name": "capture_screenshot" })],
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items).entries;
    assert_eq!(
        transcript,
        vec![
            "[1] user: Review this screenshot.\n",
            "[2] tool result: Screenshot captured.\n",
        ]
    );
}

#[test]
fn transcript_omits_encrypted_messages_arguments_and_tool_outputs() {
    let items = vec![
        ResponseItem::AgentMessage {
            id: None,
            author: "worker".to_string(),
            recipient: "parent".to_string(),
            content: vec![AgentMessageInputContent::EncryptedContent {
                encrypted_content: "encrypted-agent-message".to_string(),
            }],
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::AgentMessage {
            id: None,
            author: "worker".to_string(),
            recipient: "parent".to_string(),
            content: vec![AgentMessageInputContent::InputText {
                text: "The workspace is ready.".to_string(),
            }],
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "exec_command".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            encrypted_function_args: Some(vec!["encrypted-function-arguments".to_string()]),
            call_id: "call-1".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::CustomToolCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            name: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "Command completed.".to_string(),
                },
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: "encrypted-tool-output".to_string(),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let transcript = TranscriptConfig::default().build(&items).entries;
    assert_eq!(
        transcript,
        vec![
            "[1] assistant: Agent message from worker:\nThe workspace is ready.\n",
            "[2] tool exec_command call: {}\n",
            "[3] tool exec_command result: Command completed.\n",
        ]
    );
}
