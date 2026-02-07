use super::*;
use crate::truncate;
use crate::truncate::TruncationPolicy;
use codex_git::GhostCommit;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use pretty_assertions::assert_eq;
use regex_lite::Regex;

const EXEC_FORMAT_MAX_BYTES: usize = 10_000;
const EXEC_FORMAT_MAX_TOKENS: usize = 2_500;

fn assistant_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
    }
}

fn create_history_with_items(items: Vec<ResponseItem>) -> ContextManager {
    let mut h = ContextManager::new();
    // Use a generous but fixed token budget; tests only rely on truncation
    // behavior, not on a specific model's token limit.
    h.record_items(items.iter(), TruncationPolicy::Tokens(10_000));
    h
}

fn user_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
    }
}

fn user_input_text_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
    }
}

fn function_call_output(call_id: &str, content: &str) -> ResponseItem {
    ResponseItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload::from_text(content.to_string()),
    }
}

fn custom_tool_call_output(call_id: &str, output: &str) -> ResponseItem {
    ResponseItem::CustomToolCallOutput {
        call_id: call_id.to_string(),
        output: output.to_string(),
    }
}

fn reasoning_msg(text: &str) -> ResponseItem {
    ResponseItem::Reasoning {
        id: String::new(),
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "summary".to_string(),
        }],
        content: Some(vec![ReasoningItemContent::ReasoningText {
            text: text.to_string(),
        }]),
        encrypted_content: None,
    }
}

fn reasoning_with_encrypted_content(len: usize) -> ResponseItem {
    ResponseItem::Reasoning {
        id: String::new(),
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "summary".to_string(),
        }],
        content: None,
        encrypted_content: Some("a".repeat(len)),
    }
}

fn truncate_exec_output(content: &str) -> String {
    truncate::truncate_text(content, TruncationPolicy::Tokens(EXEC_FORMAT_MAX_TOKENS))
}

fn approx_token_count_for_text(text: &str) -> i64 {
    i64::try_from(text.len().saturating_add(3) / 4).unwrap_or(i64::MAX)
}

#[test]
fn filters_non_api_messages() {
    let mut h = ContextManager::default();
    let policy = TruncationPolicy::Tokens(10_000);
    // System message is not API messages; Other is ignored.
    let system = ResponseItem::Message {
        id: None,
        role: "system".to_string(),
        content: vec![ContentItem::OutputText {
            text: "ignored".to_string(),
        }],
        end_turn: None,
        phase: None,
    };
    let reasoning = reasoning_msg("thinking...");
    h.record_items([&system, &reasoning, &ResponseItem::Other], policy);

    // User and assistant should be retained.
    let u = user_msg("hi");
    let a = assistant_msg("hello");
    h.record_items([&u, &a], policy);

    let items = h.raw_items();
    assert_eq!(
        items,
        vec![
            ResponseItem::Reasoning {
                id: String::new(),
                summary: vec![ReasoningItemReasoningSummary::SummaryText {
                    text: "summary".to_string(),
                }],
                content: Some(vec![ReasoningItemContent::ReasoningText {
                    text: "thinking...".to_string(),
                }]),
                encrypted_content: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "hi".to_string()
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "hello".to_string()
                }],
                end_turn: None,
                phase: None,
            }
        ]
    );
}

#[test]
fn non_last_reasoning_tokens_return_zero_when_no_user_messages() {
    let history = create_history_with_items(vec![reasoning_with_encrypted_content(800)]);

    assert_eq!(history.get_non_last_reasoning_items_tokens(), 0);
}

#[test]
fn non_last_reasoning_tokens_ignore_entries_after_last_user() {
    let history = create_history_with_items(vec![
        reasoning_with_encrypted_content(900),
        user_msg("first"),
        reasoning_with_encrypted_content(1_000),
        user_msg("second"),
        reasoning_with_encrypted_content(2_000),
    ]);
    // first: (900 * 0.75 - 650) / 4 = 6.25 tokens
    // second: (1000 * 0.75 - 650) / 4 = 25 tokens
    // first + second = 62.5
    assert_eq!(history.get_non_last_reasoning_items_tokens(), 32);
}

#[test]
fn trailing_codex_generated_tokens_stop_at_first_non_generated_item() {
    let earlier_output = function_call_output("call-earlier", "earlier output");
    let trailing_function_output = function_call_output("call-tail-1", "tail function output");
    let trailing_custom_output = custom_tool_call_output("call-tail-2", "tail custom output");
    let history = create_history_with_items(vec![
        earlier_output,
        user_msg("boundary item"),
        trailing_function_output.clone(),
        trailing_custom_output.clone(),
    ]);
    let expected_tokens = estimate_item_token_count(&trailing_function_output)
        .saturating_add(estimate_item_token_count(&trailing_custom_output));

    assert_eq!(
        history.get_trailing_codex_generated_items_tokens(),
        expected_tokens
    );
}

#[test]
fn trailing_codex_generated_tokens_exclude_function_call_tail() {
    let history = create_history_with_items(vec![ResponseItem::FunctionCall {
        id: None,
        name: "not-generated".to_string(),
        arguments: "{}".to_string(),
        call_id: "call-tail".to_string(),
    }]);

    assert_eq!(history.get_trailing_codex_generated_items_tokens(), 0);
}

#[test]
fn total_token_usage_includes_only_trailing_codex_generated_items() {
    let non_trailing_output = function_call_output("call-before-message", "not trailing");
    let trailing_assistant = assistant_msg("assistant boundary");
    let trailing_output = custom_tool_call_output("tool-tail", "trailing output");
    let mut history = create_history_with_items(vec![
        non_trailing_output,
        user_msg("boundary"),
        trailing_assistant,
        trailing_output.clone(),
    ]);
    history.update_token_info(
        &TokenUsage {
            total_tokens: 100,
            ..Default::default()
        },
        None,
    );

    assert_eq!(
        history.get_total_token_usage(true),
        100 + estimate_item_token_count(&trailing_output)
    );
}

#[test]
fn get_history_for_prompt_drops_ghost_commits() {
    let items = vec![ResponseItem::GhostSnapshot {
        ghost_commit: GhostCommit::new("ghost-1".to_string(), None, Vec::new(), Vec::new()),
    }];
    let history = create_history_with_items(items);
    let filtered = history.for_prompt();
    assert_eq!(filtered, vec![]);
}

#[test]
fn estimate_token_count_with_base_instructions_uses_provided_text() {
    let history = create_history_with_items(vec![assistant_msg("hello from history")]);
    let short_base = BaseInstructions {
        text: "short".to_string(),
    };
    let long_base = BaseInstructions {
        text: "x".repeat(1_000),
    };

    let short_estimate = history
        .estimate_token_count_with_base_instructions(&short_base)
        .expect("token estimate");
    let long_estimate = history
        .estimate_token_count_with_base_instructions(&long_base)
        .expect("token estimate");

    let expected_delta = approx_token_count_for_text(&long_base.text)
        - approx_token_count_for_text(&short_base.text);
    assert_eq!(long_estimate - short_estimate, expected_delta);
}

#[test]
fn remove_first_item_removes_matching_output_for_function_call() {
    let items = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "do_it".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
    ];
    let mut h = create_history_with_items(items);
    h.remove_first_item();
    assert_eq!(h.raw_items(), vec![]);
}

#[test]
fn remove_first_item_removes_matching_call_for_output() {
    let items = vec![
        ResponseItem::FunctionCallOutput {
            call_id: "call-2".to_string(),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "do_it".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-2".to_string(),
        },
    ];
    let mut h = create_history_with_items(items);
    h.remove_first_item();
    assert_eq!(h.raw_items(), vec![]);
}

#[test]
fn remove_last_item_removes_matching_call_for_output() {
    let items = vec![
        user_msg("before tool call"),
        ResponseItem::FunctionCall {
            id: None,
            name: "do_it".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-delete-last".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-delete-last".to_string(),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
    ];
    let mut h = create_history_with_items(items);

    assert!(h.remove_last_item());
    assert_eq!(h.raw_items(), vec![user_msg("before tool call")]);
}

#[test]
fn replace_last_turn_images_replaces_tool_output_images() {
    let items = vec![
        user_input_text_msg("hi"),
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::ContentItems(vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "data:image/png;base64,AAA".to_string(),
                    },
                ]),
                success: Some(true),
            },
        },
    ];
    let mut history = create_history_with_items(items);

    assert!(history.replace_last_turn_images("Invalid image"));

    assert_eq!(
        history.raw_items(),
        vec![
            user_input_text_msg("hi"),
            ResponseItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::ContentItems(vec![
                        FunctionCallOutputContentItem::InputText {
                            text: "Invalid image".to_string(),
                        },
                    ]),
                    success: Some(true),
                },
            },
        ]
    );
}

#[test]
fn replace_last_turn_images_does_not_touch_user_images() {
    let items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputImage {
            image_url: "data:image/png;base64,AAA".to_string(),
        }],
        end_turn: None,
        phase: None,
    }];
    let mut history = create_history_with_items(items.clone());

    assert!(!history.replace_last_turn_images("Invalid image"));
    assert_eq!(history.raw_items(), items);
}

#[test]
fn remove_first_item_handles_local_shell_pair() {
    let items = vec![
        ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("call-3".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".to_string(), "hi".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-3".to_string(),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
    ];
    let mut h = create_history_with_items(items);
    h.remove_first_item();
    assert_eq!(h.raw_items(), vec![]);
}

#[test]
fn drop_last_n_user_turns_preserves_prefix() {
    let items = vec![
        assistant_msg("session prefix item"),
        user_msg("u1"),
        assistant_msg("a1"),
        user_msg("u2"),
        assistant_msg("a2"),
    ];

    let mut history = create_history_with_items(items);
    history.drop_last_n_user_turns(1);
    assert_eq!(
        history.for_prompt(),
        vec![
            assistant_msg("session prefix item"),
            user_msg("u1"),
            assistant_msg("a1"),
        ]
    );

    let mut history = create_history_with_items(vec![
        assistant_msg("session prefix item"),
        user_msg("u1"),
        assistant_msg("a1"),
        user_msg("u2"),
        assistant_msg("a2"),
    ]);
    history.drop_last_n_user_turns(99);
    assert_eq!(
        history.for_prompt(),
        vec![assistant_msg("session prefix item")]
    );
}

#[test]
fn drop_last_n_user_turns_ignores_session_prefix_user_messages() {
    let items = vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg("<user_instructions>do the thing</user_instructions>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
        user_input_text_msg("turn 2 user"),
        assistant_msg("turn 2 assistant"),
    ];

    let mut history = create_history_with_items(items);
    history.drop_last_n_user_turns(1);

    let expected_prefix_and_first_turn = vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg("<user_instructions>do the thing</user_instructions>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
    ];

    assert_eq!(history.for_prompt(), expected_prefix_and_first_turn);

    let expected_prefix_only = vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg("<user_instructions>do the thing</user_instructions>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
    ];

    let mut history = create_history_with_items(vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg("<user_instructions>do the thing</user_instructions>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
        user_input_text_msg("turn 2 user"),
        assistant_msg("turn 2 assistant"),
    ]);
    history.drop_last_n_user_turns(2);
    assert_eq!(history.for_prompt(), expected_prefix_only);

    let mut history = create_history_with_items(vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg("<user_instructions>do the thing</user_instructions>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
        user_input_text_msg("turn 2 user"),
        assistant_msg("turn 2 assistant"),
    ]);
    history.drop_last_n_user_turns(3);
    assert_eq!(history.for_prompt(), expected_prefix_only);
}

#[test]
fn remove_first_item_handles_custom_tool_pair() {
    let items = vec![
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "tool-1".to_string(),
            name: "my_tool".to_string(),
            input: "{}".to_string(),
        },
        ResponseItem::CustomToolCallOutput {
            call_id: "tool-1".to_string(),
            output: "ok".to_string(),
        },
    ];
    let mut h = create_history_with_items(items);
    h.remove_first_item();
    assert_eq!(h.raw_items(), vec![]);
}

#[test]
fn normalization_retains_local_shell_outputs() {
    let items = vec![
        ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell-1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".to_string(), "hi".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "shell-1".to_string(),
            output: FunctionCallOutputPayload::from_text("Total output lines: 1\n\nok".to_string()),
        },
    ];

    let history = create_history_with_items(items.clone());
    let normalized = history.for_prompt();
    assert_eq!(normalized, items);
}

#[test]
fn record_items_truncates_function_call_output_content() {
    let mut history = ContextManager::new();
    // Any reasonably small token budget works; the test only cares that
    // truncation happens and the marker is present.
    let policy = TruncationPolicy::Tokens(1_000);
    let long_line = "a very long line to trigger truncation\n";
    let long_output = long_line.repeat(2_500);
    let item = ResponseItem::FunctionCallOutput {
        call_id: "call-100".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text(long_output.clone()),
            success: Some(true),
        },
    };

    history.record_items([&item], policy);

    assert_eq!(history.items.len(), 1);
    match &history.items[0] {
        ResponseItem::FunctionCallOutput { output, .. } => {
            let content = output.text_content().unwrap_or_default();
            assert_ne!(content, long_output);
            assert!(
                content.contains("tokens truncated"),
                "expected token-based truncation marker, got {content}"
            );
            assert!(
                content.contains("tokens truncated"),
                "expected truncation marker, got {content}"
            );
        }
        other => panic!("unexpected history item: {other:?}"),
    }
}

#[test]
fn record_items_truncates_custom_tool_call_output_content() {
    let mut history = ContextManager::new();
    let policy = TruncationPolicy::Tokens(1_000);
    let line = "custom output that is very long\n";
    let long_output = line.repeat(2_500);
    let item = ResponseItem::CustomToolCallOutput {
        call_id: "tool-200".to_string(),
        output: long_output.clone(),
    };

    history.record_items([&item], policy);

    assert_eq!(history.items.len(), 1);
    match &history.items[0] {
        ResponseItem::CustomToolCallOutput { output, .. } => {
            assert_ne!(output, &long_output);
            assert!(
                output.contains("tokens truncated"),
                "expected token-based truncation marker, got {output}"
            );
            assert!(
                output.contains("tokens truncated") || output.contains("bytes truncated"),
                "expected truncation marker, got {output}"
            );
        }
        other => panic!("unexpected history item: {other:?}"),
    }
}

#[test]
fn record_items_respects_custom_token_limit() {
    let mut history = ContextManager::new();
    let policy = TruncationPolicy::Tokens(10);
    let long_output = "tokenized content repeated many times ".repeat(200);
    let item = ResponseItem::FunctionCallOutput {
        call_id: "call-custom-limit".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text(long_output),
            success: Some(true),
        },
    };

    history.record_items([&item], policy);

    let stored = match &history.items[0] {
        ResponseItem::FunctionCallOutput { output, .. } => output,
        other => panic!("unexpected history item: {other:?}"),
    };
    assert!(
        stored
            .text_content()
            .is_some_and(|content| content.contains("tokens truncated"))
    );
}

fn assert_truncated_message_matches(message: &str, line: &str, expected_removed: usize) {
    let pattern = truncated_message_pattern(line);
    let regex = Regex::new(&pattern).unwrap_or_else(|err| {
        panic!("failed to compile regex {pattern}: {err}");
    });
    let captures = regex
        .captures(message)
        .unwrap_or_else(|| panic!("message failed to match pattern {pattern}: {message}"));
    let body = captures
        .name("body")
        .expect("missing body capture")
        .as_str();
    assert!(
        body.len() <= EXEC_FORMAT_MAX_BYTES,
        "body exceeds byte limit: {} bytes",
        body.len()
    );
    let removed: usize = captures
        .name("removed")
        .expect("missing removed capture")
        .as_str()
        .parse()
        .unwrap_or_else(|err| panic!("invalid removed tokens: {err}"));
    assert_eq!(removed, expected_removed, "mismatched removed token count");
}

fn truncated_message_pattern(line: &str) -> String {
    let escaped_line = regex_lite::escape(line);
    format!(r"(?s)^(?P<body>{escaped_line}.*?)(?:\r?)?…(?P<removed>\d+) tokens truncated…(?:.*)?$")
}

#[test]
fn format_exec_output_truncates_large_error() {
    let line = "very long execution error line that should trigger truncation\n";
    let large_error = line.repeat(2_500); // way beyond both byte and line limits

    let truncated = truncate_exec_output(&large_error);

    assert_truncated_message_matches(&truncated, line, 36250);
    assert_ne!(truncated, large_error);
}

#[test]
fn format_exec_output_marks_byte_truncation_without_omitted_lines() {
    let long_line = "a".repeat(EXEC_FORMAT_MAX_BYTES + 10000);
    let truncated = truncate_exec_output(&long_line);
    assert_ne!(truncated, long_line);
    assert_truncated_message_matches(&truncated, "a", 2500);
    assert!(
        !truncated.contains("omitted"),
        "line omission marker should not appear when no lines were dropped: {truncated}"
    );
}

#[test]
fn format_exec_output_returns_original_when_within_limits() {
    let content = "example output\n".repeat(10);
    assert_eq!(truncate_exec_output(&content), content);
}

#[test]
fn format_exec_output_reports_omitted_lines_and_keeps_head_and_tail() {
    let total_lines = 2_000;
    let filler = "x".repeat(64);
    let content: String = (0..total_lines)
        .map(|idx| format!("line-{idx}-{filler}\n"))
        .collect();

    let truncated = truncate_exec_output(&content);
    assert_truncated_message_matches(&truncated, "line-0-", 34_723);
    assert!(
        truncated.contains("line-0-"),
        "expected head line to remain: {truncated}"
    );

    let last_line = format!("line-{}-", total_lines - 1);
    assert!(
        truncated.contains(&last_line),
        "expected tail line to remain: {truncated}"
    );
}

#[test]
fn format_exec_output_prefers_line_marker_when_both_limits_exceeded() {
    let total_lines = 300;
    let long_line = "x".repeat(256);
    let content: String = (0..total_lines)
        .map(|idx| format!("line-{idx}-{long_line}\n"))
        .collect();

    let truncated = truncate_exec_output(&content);

    assert_truncated_message_matches(&truncated, "line-0-", 17_423);
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_adds_missing_output_for_function_call() {
    let items = vec![ResponseItem::FunctionCall {
        id: None,
        name: "do_it".to_string(),
        arguments: "{}".to_string(),
        call_id: "call-x".to_string(),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history();

    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "do_it".to_string(),
                arguments: "{}".to_string(),
                call_id: "call-x".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-x".to_string(),
                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
            },
        ]
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_adds_missing_output_for_custom_tool_call() {
    let items = vec![ResponseItem::CustomToolCall {
        id: None,
        status: None,
        call_id: "tool-x".to_string(),
        name: "custom".to_string(),
        input: "{}".to_string(),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history();

    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "tool-x".to_string(),
                name: "custom".to_string(),
                input: "{}".to_string(),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "tool-x".to_string(),
                output: "aborted".to_string(),
            },
        ]
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_adds_missing_output_for_local_shell_call_with_id() {
    let items = vec![ResponseItem::LocalShellCall {
        id: None,
        call_id: Some("shell-1".to_string()),
        status: LocalShellStatus::Completed,
        action: LocalShellAction::Exec(LocalShellExecAction {
            command: vec!["echo".to_string(), "hi".to_string()],
            timeout_ms: None,
            working_directory: None,
            env: None,
            user: None,
        }),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history();

    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::LocalShellCall {
                id: None,
                call_id: Some("shell-1".to_string()),
                status: LocalShellStatus::Completed,
                action: LocalShellAction::Exec(LocalShellExecAction {
                    command: vec!["echo".to_string(), "hi".to_string()],
                    timeout_ms: None,
                    working_directory: None,
                    env: None,
                    user: None,
                }),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "shell-1".to_string(),
                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
            },
        ]
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_removes_orphan_function_call_output() {
    let items = vec![ResponseItem::FunctionCallOutput {
        call_id: "orphan-1".to_string(),
        output: FunctionCallOutputPayload::from_text("ok".to_string()),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history();

    assert_eq!(h.raw_items(), vec![]);
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_removes_orphan_custom_tool_call_output() {
    let items = vec![ResponseItem::CustomToolCallOutput {
        call_id: "orphan-2".to_string(),
        output: "ok".to_string(),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history();

    assert_eq!(h.raw_items(), vec![]);
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_mixed_inserts_and_removals() {
    let items = vec![
        // Will get an inserted output
        ResponseItem::FunctionCall {
            id: None,
            name: "f1".to_string(),
            arguments: "{}".to_string(),
            call_id: "c1".to_string(),
        },
        // Orphan output that should be removed
        ResponseItem::FunctionCallOutput {
            call_id: "c2".to_string(),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
        // Will get an inserted custom tool output
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "t1".to_string(),
            name: "tool".to_string(),
            input: "{}".to_string(),
        },
        // Local shell call also gets an inserted function call output
        ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("s1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        },
    ];
    let mut h = create_history_with_items(items);

    h.normalize_history();

    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "f1".to_string(),
                arguments: "{}".to_string(),
                call_id: "c1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "c1".to_string(),
                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
            },
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "t1".to_string(),
                name: "tool".to_string(),
                input: "{}".to_string(),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "t1".to_string(),
                output: "aborted".to_string(),
            },
            ResponseItem::LocalShellCall {
                id: None,
                call_id: Some("s1".to_string()),
                status: LocalShellStatus::Completed,
                action: LocalShellAction::Exec(LocalShellExecAction {
                    command: vec!["echo".to_string()],
                    timeout_ms: None,
                    working_directory: None,
                    env: None,
                    user: None,
                }),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "s1".to_string(),
                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
            },
        ]
    );
}

#[test]
fn normalize_adds_missing_output_for_function_call_inserts_output() {
    let items = vec![ResponseItem::FunctionCall {
        id: None,
        name: "do_it".to_string(),
        arguments: "{}".to_string(),
        call_id: "call-x".to_string(),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history();
    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "do_it".to_string(),
                arguments: "{}".to_string(),
                call_id: "call-x".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-x".to_string(),
                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
            },
        ]
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_adds_missing_output_for_custom_tool_call_panics_in_debug() {
    let items = vec![ResponseItem::CustomToolCall {
        id: None,
        status: None,
        call_id: "tool-x".to_string(),
        name: "custom".to_string(),
        input: "{}".to_string(),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history();
}

#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_adds_missing_output_for_local_shell_call_with_id_panics_in_debug() {
    let items = vec![ResponseItem::LocalShellCall {
        id: None,
        call_id: Some("shell-1".to_string()),
        status: LocalShellStatus::Completed,
        action: LocalShellAction::Exec(LocalShellExecAction {
            command: vec!["echo".to_string(), "hi".to_string()],
            timeout_ms: None,
            working_directory: None,
            env: None,
            user: None,
        }),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history();
}

#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_removes_orphan_function_call_output_panics_in_debug() {
    let items = vec![ResponseItem::FunctionCallOutput {
        call_id: "orphan-1".to_string(),
        output: FunctionCallOutputPayload::from_text("ok".to_string()),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history();
}

#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_removes_orphan_custom_tool_call_output_panics_in_debug() {
    let items = vec![ResponseItem::CustomToolCallOutput {
        call_id: "orphan-2".to_string(),
        output: "ok".to_string(),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history();
}

#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_mixed_inserts_and_removals_panics_in_debug() {
    let items = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "f1".to_string(),
            arguments: "{}".to_string(),
            call_id: "c1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "c2".to_string(),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "t1".to_string(),
            name: "tool".to_string(),
            input: "{}".to_string(),
        },
        ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("s1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        },
    ];
    let mut h = create_history_with_items(items);
    h.normalize_history();
}
