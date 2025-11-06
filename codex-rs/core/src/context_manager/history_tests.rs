use super::*;
use crate::context_manager::truncate;
use codex_git::GhostCommit;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use pretty_assertions::assert_eq;
use regex_lite::Regex;

fn assistant_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
    }
}

fn create_history_with_items(items: Vec<ResponseItem>) -> ContextManager {
    let mut h = ContextManager::new();
    h.record_items(items.iter());
    h
}

fn user_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
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

#[test]
fn filters_non_api_messages() {
    let mut h = ContextManager::default();
    // System message is not API messages; Other is ignored.
    let system = ResponseItem::Message {
        id: None,
        role: "system".to_string(),
        content: vec![ContentItem::OutputText {
            text: "ignored".to_string(),
        }],
    };
    let reasoning = reasoning_msg("thinking...");
    h.record_items([&system, &reasoning, &ResponseItem::Other]);

    // User and assistant should be retained.
    let u = user_msg("hi");
    let a = assistant_msg("hello");
    h.record_items([&u, &a]);

    let items = h.contents();
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
                }]
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "hello".to_string()
                }]
            }
        ]
    );
}

#[test]
fn get_history_for_prompt_drops_ghost_commits() {
    let items = vec![ResponseItem::GhostSnapshot {
        ghost_commit: GhostCommit::new("ghost-1".to_string(), None, Vec::new(), Vec::new()),
    }];
    let mut history = create_history_with_items(items);
    let filtered = history.get_history_for_prompt();
    assert_eq!(filtered, vec![]);
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
            output: FunctionCallOutputPayload {
                content: "ok".to_string(),
                ..Default::default()
            },
        },
    ];
    let mut h = create_history_with_items(items);
    h.remove_first_item();
    assert_eq!(h.contents(), vec![]);
}

#[test]
fn remove_first_item_removes_matching_call_for_output() {
    let items = vec![
        ResponseItem::FunctionCallOutput {
            call_id: "call-2".to_string(),
            output: FunctionCallOutputPayload {
                content: "ok".to_string(),
                ..Default::default()
            },
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
    assert_eq!(h.contents(), vec![]);
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
            output: FunctionCallOutputPayload {
                content: "ok".to_string(),
                ..Default::default()
            },
        },
    ];
    let mut h = create_history_with_items(items);
    h.remove_first_item();
    assert_eq!(h.contents(), vec![]);
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
    assert_eq!(h.contents(), vec![]);
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
            output: FunctionCallOutputPayload {
                content: "ok".to_string(),
                ..Default::default()
            },
        },
    ];

    let mut history = create_history_with_items(items.clone());
    let normalized = history.get_history();
    assert_eq!(normalized, items);
}

#[test]
fn record_items_truncates_function_call_output_content() {
    let mut history = ContextManager::new();
    let long_line = "a very long line to trigger truncation\n";
    let long_output = long_line.repeat(2_500);
    let item = ResponseItem::FunctionCallOutput {
        call_id: "call-100".to_string(),
        output: FunctionCallOutputPayload {
            content: long_output.clone(),
            success: Some(true),
            ..Default::default()
        },
    };

    history.record_items([&item]);

    assert_eq!(history.items.len(), 1);
    match &history.items[0] {
        ResponseItem::FunctionCallOutput { output, .. } => {
            assert_ne!(output.content, long_output);
            assert!(
                output.content.starts_with("Total output lines:"),
                "expected truncated summary, got {}",
                output.content
            );
        }
        other => panic!("unexpected history item: {other:?}"),
    }
}

#[test]
fn record_items_truncates_custom_tool_call_output_content() {
    let mut history = ContextManager::new();
    let line = "custom output that is very long\n";
    let long_output = line.repeat(2_500);
    let item = ResponseItem::CustomToolCallOutput {
        call_id: "tool-200".to_string(),
        output: long_output.clone(),
    };

    history.record_items([&item]);

    assert_eq!(history.items.len(), 1);
    match &history.items[0] {
        ResponseItem::CustomToolCallOutput { output, .. } => {
            assert_ne!(output, &long_output);
            assert!(
                output.starts_with("Total output lines:"),
                "expected truncated summary, got {output}"
            );
        }
        other => panic!("unexpected history item: {other:?}"),
    }
}

fn assert_truncated_message_matches(message: &str, line: &str, total_lines: usize) {
    let pattern = truncated_message_pattern(line, total_lines);
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
        body.len() <= truncate::MODEL_FORMAT_MAX_BYTES,
        "body exceeds byte limit: {} bytes",
        body.len()
    );
}

fn truncated_message_pattern(line: &str, total_lines: usize) -> String {
    let head_take = truncate::MODEL_FORMAT_HEAD_LINES.min(total_lines);
    let tail_take = truncate::MODEL_FORMAT_TAIL_LINES.min(total_lines.saturating_sub(head_take));
    let omitted = total_lines.saturating_sub(head_take + tail_take);
    let escaped_line = regex_lite::escape(line);
    if omitted == 0 {
        return format!(
            r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} output truncated to fit {max_bytes} bytes \.{{3}}]\n\n.*)$",
            max_bytes = truncate::MODEL_FORMAT_MAX_BYTES,
        );
    }
    format!(
        r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} omitted {omitted} of {total_lines} lines \.{{3}}]\n\n.*)$",
    )
}

#[test]
fn format_exec_output_truncates_large_error() {
    let line = "very long execution error line that should trigger truncation\n";
    let large_error = line.repeat(2_500); // way beyond both byte and line limits

    let truncated = truncate::format_output_for_model_body(&large_error);

    let total_lines = large_error.lines().count();
    assert_truncated_message_matches(&truncated, line, total_lines);
    assert_ne!(truncated, large_error);
}

#[test]
fn format_exec_output_marks_byte_truncation_without_omitted_lines() {
    let long_line = "a".repeat(truncate::MODEL_FORMAT_MAX_BYTES + 50);
    let truncated = truncate::format_output_for_model_body(&long_line);

    assert_ne!(truncated, long_line);
    let marker_line = format!(
        "[... output truncated to fit {} bytes ...]",
        truncate::MODEL_FORMAT_MAX_BYTES
    );
    assert!(
        truncated.contains(&marker_line),
        "missing byte truncation marker: {truncated}"
    );
    assert!(
        !truncated.contains("omitted"),
        "line omission marker should not appear when no lines were dropped: {truncated}"
    );
}

#[test]
fn format_exec_output_returns_original_when_within_limits() {
    let content = "example output\n".repeat(10);

    assert_eq!(truncate::format_output_for_model_body(&content), content);
}

#[test]
fn format_exec_output_reports_omitted_lines_and_keeps_head_and_tail() {
    let total_lines = truncate::MODEL_FORMAT_MAX_LINES + 100;
    let content: String = (0..total_lines)
        .map(|idx| format!("line-{idx}\n"))
        .collect();

    let truncated = truncate::format_output_for_model_body(&content);
    let omitted = total_lines - truncate::MODEL_FORMAT_MAX_LINES;
    let expected_marker = format!("[... omitted {omitted} of {total_lines} lines ...]");

    assert!(
        truncated.contains(&expected_marker),
        "missing omitted marker: {truncated}"
    );
    assert!(
        truncated.contains("line-0\n"),
        "expected head line to remain: {truncated}"
    );

    let last_line = format!("line-{}\n", total_lines - 1);
    assert!(
        truncated.contains(&last_line),
        "expected tail line to remain: {truncated}"
    );
}

#[test]
fn format_exec_output_prefers_line_marker_when_both_limits_exceeded() {
    let total_lines = truncate::MODEL_FORMAT_MAX_LINES + 42;
    let long_line = "x".repeat(256);
    let content: String = (0..total_lines)
        .map(|idx| format!("line-{idx}-{long_line}\n"))
        .collect();

    let truncated = truncate::format_output_for_model_body(&content);

    assert!(
        truncated.contains("[... omitted 42 of 298 lines ...]"),
        "expected omitted marker when line count exceeds limit: {truncated}"
    );
    assert!(
        !truncated.contains("output truncated to fit"),
        "line omission marker should take precedence over byte marker: {truncated}"
    );
}

#[test]
fn truncates_across_multiple_under_limit_texts_and_reports_omitted() {
    // Arrange: several text items, none exceeding per-item limit, but total exceeds budget.
    let budget = truncate::MODEL_FORMAT_MAX_BYTES;
    let t1_len = (budget / 2).saturating_sub(10);
    let t2_len = (budget / 2).saturating_sub(10);
    let remaining_after_t1_t2 = budget.saturating_sub(t1_len + t2_len);
    let t3_len = 50; // gets truncated to remaining_after_t1_t2
    let t4_len = 5; // omitted
    let t5_len = 7; // omitted

    let t1 = "a".repeat(t1_len);
    let t2 = "b".repeat(t2_len);
    let t3 = "c".repeat(t3_len);
    let t4 = "d".repeat(t4_len);
    let t5 = "e".repeat(t5_len);

    let item = ResponseItem::FunctionCallOutput {
        call_id: "call-omit".to_string(),
        output: FunctionCallOutputPayload {
            content: "irrelevant".to_string(),
            content_items: Some(vec![
                FunctionCallOutputContentItem::InputText { text: t1 },
                FunctionCallOutputContentItem::InputText { text: t2 },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "img:mid".to_string(),
                },
                FunctionCallOutputContentItem::InputText { text: t3 },
                FunctionCallOutputContentItem::InputText { text: t4 },
                FunctionCallOutputContentItem::InputText { text: t5 },
            ]),
            success: Some(true),
        },
    };

    let mut history = ContextManager::new();
    history.record_items([&item]);
    assert_eq!(history.items.len(), 1);
    let json = serde_json::to_value(&history.items[0]).expect("serialize to json");

    let output = json
        .get("output")
        .expect("output field")
        .as_array()
        .expect("array output");

    // Expect: t1 (full), t2 (full), image, t3 (truncated), summary mentioning 2 omitted.
    assert_eq!(output.len(), 5);

    let first = output[0].as_object().expect("first obj");
    assert_eq!(first.get("type").unwrap(), "input_text");
    let first_text = first.get("text").unwrap().as_str().unwrap();
    assert_eq!(first_text.len(), t1_len);

    let second = output[1].as_object().expect("second obj");
    assert_eq!(second.get("type").unwrap(), "input_text");
    let second_text = second.get("text").unwrap().as_str().unwrap();
    assert_eq!(second_text.len(), t2_len);

    assert_eq!(
        output[2],
        serde_json::json!({"type": "input_image", "image_url": "img:mid"})
    );

    let fourth = output[3].as_object().expect("fourth obj");
    assert_eq!(fourth.get("type").unwrap(), "input_text");
    let fourth_text = fourth.get("text").unwrap().as_str().unwrap();
    assert_eq!(fourth_text.len(), remaining_after_t1_t2);

    let summary = output[4].as_object().expect("summary obj");
    assert_eq!(summary.get("type").unwrap(), "input_text");
    let summary_text = summary.get("text").unwrap().as_str().unwrap();
    assert!(summary_text.contains("omitted 2 text items"));
}

//TODO(aibrahim): run CI in release mode.
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
        h.contents(),
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "do_it".to_string(),
                arguments: "{}".to_string(),
                call_id: "call-x".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-x".to_string(),
                output: FunctionCallOutputPayload {
                    content: "aborted".to_string(),
                    ..Default::default()
                },
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
        h.contents(),
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
        h.contents(),
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
                output: FunctionCallOutputPayload {
                    content: "aborted".to_string(),
                    ..Default::default()
                },
            },
        ]
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_removes_orphan_function_call_output() {
    let items = vec![ResponseItem::FunctionCallOutput {
        call_id: "orphan-1".to_string(),
        output: FunctionCallOutputPayload {
            content: "ok".to_string(),
            ..Default::default()
        },
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history();

    assert_eq!(h.contents(), vec![]);
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

    assert_eq!(h.contents(), vec![]);
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
            output: FunctionCallOutputPayload {
                content: "ok".to_string(),
                ..Default::default()
            },
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
        h.contents(),
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "f1".to_string(),
                arguments: "{}".to_string(),
                call_id: "c1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "c1".to_string(),
                output: FunctionCallOutputPayload {
                    content: "aborted".to_string(),
                    ..Default::default()
                },
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
                output: FunctionCallOutputPayload {
                    content: "aborted".to_string(),
                    ..Default::default()
                },
            },
        ]
    );
}

// In debug builds we panic on normalization errors instead of silently fixing them.
#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_adds_missing_output_for_function_call_panics_in_debug() {
    let items = vec![ResponseItem::FunctionCall {
        id: None,
        name: "do_it".to_string(),
        arguments: "{}".to_string(),
        call_id: "call-x".to_string(),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history();
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
        output: FunctionCallOutputPayload {
            content: "ok".to_string(),
            ..Default::default()
        },
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
            output: FunctionCallOutputPayload {
                content: "ok".to_string(),
                ..Default::default()
            },
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
