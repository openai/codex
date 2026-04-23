use super::*;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_git_utils::GhostCommit;
use codex_protocol::AgentPath;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::default_input_modalities;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::TurnContextItem;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use image::ImageBuffer;
use image::ImageFormat;
use image::Rgba;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use std::path::PathBuf;

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
        exclude_from_compaction: false,
    }
}

fn inter_agent_assistant_msg(text: &str) -> ResponseItem {
    let communication = InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::root().join("worker").unwrap(),
        Vec::new(),
        text.to_string(),
        /*trigger_turn*/ true,
    );
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: serde_json::to_string(&communication).unwrap(),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
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
        exclude_from_compaction: false,
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
        exclude_from_compaction: false,
    }
}

fn developer_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    }
}

fn developer_msg_with_fragments(texts: &[&str]) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: texts
            .iter()
            .map(|text| ContentItem::InputText {
                text: (*text).to_string(),
            })
            .collect(),
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    }
}

fn reference_context_item() -> TurnContextItem {
    TurnContextItem {
        turn_id: Some("reference-turn".to_string()),
        trace_id: None,
        cwd: PathBuf::from("/tmp/reference-cwd"),
        current_date: Some("2026-03-23".to_string()),
        timezone: Some("America/Los_Angeles".to_string()),
        approval_policy: AskForApproval::OnRequest,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        network: None,
        file_system_sandbox_policy: None,
        model: "gpt-test".to_string(),
        personality: None,
        collaboration_mode: None,
        realtime_active: Some(false),
        effort: None,
        summary: ReasoningSummary::Auto,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: Some(codex_protocol::protocol::TruncationPolicy::Tokens(10_000)),
    }
}

fn custom_tool_call_output(call_id: &str, output: &str) -> ResponseItem {
    ResponseItem::CustomToolCallOutput {
        call_id: call_id.to_string(),
        name: None,
        output: FunctionCallOutputPayload::from_text(output.to_string()),
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
    truncate_text(content, TruncationPolicy::Tokens(EXEC_FORMAT_MAX_TOKENS))
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
        exclude_from_compaction: false,
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
                exclude_from_compaction: false,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "hello".to_string()
                }],
                end_turn: None,
                phase: None,
                exclude_from_compaction: false,
            }
        ]
    );
}

#[test]
fn non_last_reasoning_tokens_return_zero_when_no_user_messages() {
    let history =
        create_history_with_items(vec![reasoning_with_encrypted_content(/*len*/ 800)]);

    assert_eq!(history.get_non_last_reasoning_items_tokens(), 0);
}

#[test]
fn non_last_reasoning_tokens_ignore_entries_after_last_user() {
    let history = create_history_with_items(vec![
        reasoning_with_encrypted_content(/*len*/ 900),
        user_msg("first"),
        reasoning_with_encrypted_content(/*len*/ 1_000),
        user_msg("second"),
        reasoning_with_encrypted_content(/*len*/ 2_000),
    ]);
    // first: (900 * 0.75 - 650) / 4 = 6.25 tokens
    // second: (1000 * 0.75 - 650) / 4 = 25 tokens
    // first + second = 62.5
    assert_eq!(history.get_non_last_reasoning_items_tokens(), 32);
}

#[test]
fn items_after_last_model_generated_tokens_include_user_and_tool_output() {
    let history = create_history_with_items(vec![
        assistant_msg("already counted by API"),
        user_msg("new user message"),
        custom_tool_call_output("call-tail", "new tool output"),
    ]);
    let expected_tokens = estimate_item_token_count(&user_msg("new user message")).saturating_add(
        estimate_item_token_count(&custom_tool_call_output("call-tail", "new tool output")),
    );

    assert_eq!(
        history
            .items_after_last_model_generated_item()
            .iter()
            .map(estimate_item_token_count)
            .fold(0i64, i64::saturating_add),
        expected_tokens
    );
}

#[test]
fn items_after_last_model_generated_tokens_are_zero_without_model_generated_items() {
    let history = create_history_with_items(vec![user_msg("no model output yet")]);

    assert_eq!(
        history
            .items_after_last_model_generated_item()
            .iter()
            .map(estimate_item_token_count)
            .fold(0i64, i64::saturating_add),
        0
    );
}

#[test]
fn inter_agent_assistant_messages_are_turn_boundaries() {
    let item = inter_agent_assistant_msg("continue");

    assert!(is_user_turn_boundary(&item));
}

#[test]
fn for_prompt_preserves_inter_agent_assistant_messages() {
    let item = inter_agent_assistant_msg("continue");
    let history = create_history_with_items(vec![item.clone()]);

    assert_eq!(history.raw_items(), std::slice::from_ref(&item));
    assert_eq!(history.for_prompt(&default_input_modalities()), vec![item]);
}

#[test]
fn for_prompt_strips_compaction_exclusion_marker() {
    let mut item = user_input_text_msg("generated warning");
    let ResponseItem::Message {
        exclude_from_compaction,
        ..
    } = &mut item
    else {
        panic!("expected message item");
    };
    *exclude_from_compaction = true;
    let history = create_history_with_items(vec![item]);

    let prompt_items = history.for_prompt(&default_input_modalities());

    let expected = vec![user_input_text_msg("generated warning")];
    assert_eq!(prompt_items, expected);
}

#[test]
fn drop_last_n_user_turns_treats_inter_agent_assistant_messages_as_instruction_turns() {
    let first_turn = user_input_text_msg("first");
    let first_reply = assistant_msg("done");
    let inter_agent_turn = inter_agent_assistant_msg("continue");
    let inter_agent_reply = assistant_msg("worker reply");
    let mut history = create_history_with_items(vec![
        first_turn.clone(),
        first_reply.clone(),
        inter_agent_turn,
        inter_agent_reply,
    ]);

    history.drop_last_n_user_turns(/*num_turns*/ 1);

    assert_eq!(history.raw_items(), &vec![first_turn, first_reply]);
}

#[test]
fn legacy_inter_agent_assistant_messages_are_not_turn_boundaries() {
    let item = assistant_msg(
        "author: /root\nrecipient: /root/worker\nother_recipients: []\nContent: continue",
    );

    assert!(!is_user_turn_boundary(&item));
}

#[test]
fn total_token_usage_includes_all_items_after_last_model_generated_item() {
    let mut history = create_history_with_items(vec![assistant_msg("already counted by API")]);
    history.update_token_info(
        &TokenUsage {
            total_tokens: 100,
            ..Default::default()
        },
        /*model_context_window*/ None,
    );
    let added_user = user_msg("new user message");
    let added_tool_output = custom_tool_call_output("tool-tail", "new tool output");
    history.record_items(
        [&added_user, &added_tool_output],
        TruncationPolicy::Tokens(10_000),
    );

    assert_eq!(
        history.get_total_token_usage(/*server_reasoning_included*/ true),
        100 + estimate_item_token_count(&added_user)
            + estimate_item_token_count(&added_tool_output)
    );
}

#[test]
fn for_prompt_strips_images_when_model_does_not_support_images() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "look at this".to_string(),
                },
                ContentItem::InputImage {
                    image_url: "https://example.com/img.png".to_string(),
                    detail: Some(DEFAULT_IMAGE_DETAIL),
                },
                ContentItem::InputText {
                    text: "caption".to_string(),
                },
            ],
            end_turn: None,
            phase: None,
            exclude_from_compaction: false,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "view_image".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "image result".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "https://example.com/result.png".to_string(),
                    detail: Some(DEFAULT_IMAGE_DETAIL),
                },
            ]),
        },
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "tool-1".to_string(),
            name: "js_repl".to_string(),
            input: "view_image".to_string(),
        },
        ResponseItem::CustomToolCallOutput {
            call_id: "tool-1".to_string(),
            name: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "js repl result".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "https://example.com/js-repl-result.png".to_string(),
                    detail: Some(DEFAULT_IMAGE_DETAIL),
                },
            ]),
        },
    ];
    let history = create_history_with_items(items);
    let text_only_modalities = vec![InputModality::Text];
    let stripped = history.for_prompt(&text_only_modalities);

    let expected = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "look at this".to_string(),
                },
                ContentItem::InputText {
                    text: "image content omitted because you do not support image input"
                        .to_string(),
                },
                ContentItem::InputText {
                    text: "caption".to_string(),
                },
            ],
            end_turn: None,
            phase: None,
            exclude_from_compaction: false,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "view_image".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "image result".to_string(),
                },
                FunctionCallOutputContentItem::InputText {
                    text: "image content omitted because you do not support image input"
                        .to_string(),
                },
            ]),
        },
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "tool-1".to_string(),
            name: "js_repl".to_string(),
            input: "view_image".to_string(),
        },
        ResponseItem::CustomToolCallOutput {
            call_id: "tool-1".to_string(),
            name: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "js repl result".to_string(),
                },
                FunctionCallOutputContentItem::InputText {
                    text: "image content omitted because you do not support image input"
                        .to_string(),
                },
            ]),
        },
    ];
    assert_eq!(stripped, expected);

    // With image support, images are preserved
    let modalities = default_input_modalities();
    let with_images = create_history_with_items(vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputText {
                text: "look".to_string(),
            },
            ContentItem::InputImage {
                image_url: "https://example.com/img.png".to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    }]);
    let preserved = with_images.for_prompt(&modalities);
    assert_eq!(preserved.len(), 1);
    if let ResponseItem::Message { content, .. } = &preserved[0] {
        assert_eq!(content.len(), 2);
        assert!(matches!(content[1], ContentItem::InputImage { .. }));
    } else {
        panic!("expected Message");
    }
}

#[test]
fn for_prompt_preserves_image_generation_calls_when_images_are_supported() {
    let history = create_history_with_items(vec![
        ResponseItem::ImageGenerationCall {
            id: "ig_123".to_string(),
            status: "generating".to_string(),
            revised_prompt: Some("lobster".to_string()),
            result: "Zm9v".to_string(),
        },
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hi".to_string(),
            }],
            end_turn: None,
            phase: None,
            exclude_from_compaction: false,
        },
    ]);

    assert_eq!(
        history.for_prompt(&default_input_modalities()),
        vec![
            ResponseItem::ImageGenerationCall {
                id: "ig_123".to_string(),
                status: "generating".to_string(),
                revised_prompt: Some("lobster".to_string()),
                result: "Zm9v".to_string(),
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "hi".to_string(),
                }],
                end_turn: None,
                phase: None,
                exclude_from_compaction: false,
            }
        ]
    );
}

#[test]
fn for_prompt_clears_image_generation_result_when_images_are_unsupported() {
    let history = create_history_with_items(vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "generate a lobster".to_string(),
            }],
            end_turn: None,
            phase: None,
            exclude_from_compaction: false,
        },
        ResponseItem::ImageGenerationCall {
            id: "ig_123".to_string(),
            status: "completed".to_string(),
            revised_prompt: Some("lobster".to_string()),
            result: "Zm9v".to_string(),
        },
    ]);

    assert_eq!(
        history.for_prompt(&[InputModality::Text]),
        vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "generate a lobster".to_string(),
                }],
                end_turn: None,
                phase: None,
                exclude_from_compaction: false,
            },
            ResponseItem::ImageGenerationCall {
                id: "ig_123".to_string(),
                status: "completed".to_string(),
                revised_prompt: Some("lobster".to_string()),
                result: String::new(),
            },
        ]
    );
}

#[test]
fn get_history_for_prompt_drops_ghost_commits() {
    let items = vec![ResponseItem::GhostSnapshot {
        ghost_commit: GhostCommit::new(
            "ghost-1".to_string(),
            /*parent*/ None,
            Vec::new(),
            Vec::new(),
        ),
    }];
    let history = create_history_with_items(items);
    let modalities = default_input_modalities();
    let filtered = history.for_prompt(&modalities);
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
            namespace: None,
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
            namespace: None,
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
            namespace: None,
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
                        detail: Some(DEFAULT_IMAGE_DETAIL),
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
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
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

    let modalities = default_input_modalities();
    let mut history = create_history_with_items(items);
    history.drop_last_n_user_turns(/*num_turns*/ 1);
    assert_eq!(
        history.for_prompt(&modalities),
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
    history.drop_last_n_user_turns(/*num_turns*/ 99);
    assert_eq!(
        history.for_prompt(&modalities),
        vec![assistant_msg("session prefix item")]
    );
}

#[test]
fn drop_last_n_user_turns_ignores_session_prefix_user_messages() {
    let items = vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg(
            "<subagent_notification>{\"agent_id\":\"a\",\"status\":\"completed\"}</subagent_notification>",
        ),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
        user_input_text_msg("turn 2 user"),
        assistant_msg("turn 2 assistant"),
    ];

    let modalities = default_input_modalities();
    let mut history = create_history_with_items(items);
    history.drop_last_n_user_turns(/*num_turns*/ 1);

    let expected_prefix_and_first_turn = vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg(
            "<subagent_notification>{\"agent_id\":\"a\",\"status\":\"completed\"}</subagent_notification>",
        ),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
    ];

    assert_eq!(
        history.for_prompt(&modalities),
        expected_prefix_and_first_turn
    );

    let expected_prefix_only = vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg(
            "<subagent_notification>{\"agent_id\":\"a\",\"status\":\"completed\"}</subagent_notification>",
        ),
    ];

    let mut history = create_history_with_items(vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg(
            "<subagent_notification>{\"agent_id\":\"a\",\"status\":\"completed\"}</subagent_notification>",
        ),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
        user_input_text_msg("turn 2 user"),
        assistant_msg("turn 2 assistant"),
    ]);
    history.drop_last_n_user_turns(/*num_turns*/ 2);
    assert_eq!(history.for_prompt(&modalities), expected_prefix_only);

    let mut history = create_history_with_items(vec![
        user_input_text_msg("<environment_context>ctx</environment_context>"),
        user_input_text_msg(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
        ),
        user_input_text_msg(
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
        ),
        user_input_text_msg("<user_shell_command>echo 42</user_shell_command>"),
        user_input_text_msg(
            "<subagent_notification>{\"agent_id\":\"a\",\"status\":\"completed\"}</subagent_notification>",
        ),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
        user_input_text_msg("turn 2 user"),
        assistant_msg("turn 2 assistant"),
    ]);
    history.drop_last_n_user_turns(/*num_turns*/ 3);
    assert_eq!(history.for_prompt(&modalities), expected_prefix_only);
}

#[test]
fn drop_last_n_user_turns_trims_context_updates_above_rolled_back_turn() {
    let items = vec![
        assistant_msg("session prefix item"),
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
        developer_msg("Generated images are saved to /tmp as /tmp/image-1.png by default."),
        developer_msg("<collaboration_mode>ROLLED_BACK_DEV_INSTRUCTIONS</collaboration_mode>"),
        user_input_text_msg(
            "<environment_context><cwd>PRETURN_CONTEXT_DIFF_CWD</cwd></environment_context>",
        ),
        user_input_text_msg("turn 2 user"),
        assistant_msg("turn 2 assistant"),
    ];

    let modalities = default_input_modalities();
    let mut history = create_history_with_items(items);
    let reference_context_item = reference_context_item();
    history.set_reference_context_item(Some(reference_context_item.clone()));
    history.drop_last_n_user_turns(/*num_turns*/ 1);

    assert_eq!(
        history.clone().for_prompt(&modalities),
        vec![
            assistant_msg("session prefix item"),
            user_input_text_msg("turn 1 user"),
            assistant_msg("turn 1 assistant"),
            developer_msg("Generated images are saved to /tmp as /tmp/image-1.png by default."),
        ]
    );
    assert_eq!(
        serde_json::to_value(history.reference_context_item())
            .expect("serialize retained reference context item"),
        serde_json::to_value(Some(reference_context_item))
            .expect("serialize expected reference context item")
    );
}

#[test]
fn drop_last_n_user_turns_clears_reference_context_for_mixed_developer_context_bundles() {
    let items = vec![
        user_input_text_msg("turn 1 user"),
        assistant_msg("turn 1 assistant"),
        developer_msg_with_fragments(&[
            "<permissions instructions>contextual permissions</permissions instructions>",
            "persistent plugin instructions",
        ]),
        user_input_text_msg(
            "<environment_context><cwd>PRETURN_CONTEXT_DIFF_CWD</cwd></environment_context>",
        ),
        user_input_text_msg("turn 2 user"),
        assistant_msg("turn 2 assistant"),
    ];

    let modalities = default_input_modalities();
    let mut history = create_history_with_items(items);
    history.set_reference_context_item(Some(reference_context_item()));
    history.drop_last_n_user_turns(/*num_turns*/ 1);

    assert_eq!(
        history.clone().for_prompt(&modalities),
        vec![
            user_input_text_msg("turn 1 user"),
            assistant_msg("turn 1 assistant"),
        ]
    );
    assert!(history.reference_context_item().is_none());
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
            name: None,
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
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

    let modalities = default_input_modalities();
    let history = create_history_with_items(items.clone());
    let normalized = history.for_prompt(&modalities);
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
        name: None,
        output: FunctionCallOutputPayload::from_text(long_output.clone()),
    };

    history.record_items([&item], policy);

    assert_eq!(history.items.len(), 1);
    match &history.items[0] {
        ResponseItem::CustomToolCallOutput { output, .. } => {
            let output = output.text_content().unwrap_or_default();
            assert_ne!(output, long_output);
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

    assert_truncated_message_matches(&truncated, line, /*expected_removed*/ 36250);
    assert_ne!(truncated, large_error);
}

#[test]
fn format_exec_output_marks_byte_truncation_without_omitted_lines() {
    let long_line = "a".repeat(EXEC_FORMAT_MAX_BYTES + 10000);
    let truncated = truncate_exec_output(&long_line);
    assert_ne!(truncated, long_line);
    assert_truncated_message_matches(&truncated, "a", /*expected_removed*/ 2500);
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
    assert_truncated_message_matches(&truncated, "line-0-", /*expected_removed*/ 34_723);
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

    assert_truncated_message_matches(&truncated, "line-0-", /*expected_removed*/ 17_423);
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_adds_missing_output_for_function_call() {
    let items = vec![ResponseItem::FunctionCall {
        id: None,
        name: "do_it".to_string(),
        namespace: None,
        arguments: "{}".to_string(),
        call_id: "call-x".to_string(),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history(&default_input_modalities());

    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "do_it".to_string(),
                namespace: None,
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

    h.normalize_history(&default_input_modalities());

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
                name: None,
                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
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

    h.normalize_history(&default_input_modalities());

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

    h.normalize_history(&default_input_modalities());

    assert_eq!(h.raw_items(), vec![]);
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_removes_orphan_custom_tool_call_output() {
    let items = vec![ResponseItem::CustomToolCallOutput {
        call_id: "orphan-2".to_string(),
        name: None,
        output: FunctionCallOutputPayload::from_text("ok".to_string()),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history(&default_input_modalities());

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
            namespace: None,
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

    h.normalize_history(&default_input_modalities());

    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "f1".to_string(),
                namespace: None,
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
                name: None,
                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
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
        namespace: None,
        arguments: "{}".to_string(),
        call_id: "call-x".to_string(),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history(&default_input_modalities());
    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "do_it".to_string(),
                namespace: None,
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

#[test]
fn normalize_adds_missing_output_for_tool_search_call() {
    let items = vec![ResponseItem::ToolSearchCall {
        id: None,
        call_id: Some("search-call-x".to_string()),
        status: Some("completed".to_string()),
        execution: "client".to_string(),
        arguments: "{}".into(),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history(&default_input_modalities());

    assert_eq!(
        h.raw_items(),
        vec![
            ResponseItem::ToolSearchCall {
                id: None,
                call_id: Some("search-call-x".to_string()),
                status: Some("completed".to_string()),
                execution: "client".to_string(),
                arguments: "{}".into(),
            },
            ResponseItem::ToolSearchOutput {
                call_id: Some("search-call-x".to_string()),
                status: "completed".to_string(),
                execution: "client".to_string(),
                tools: Vec::new(),
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
    h.normalize_history(&default_input_modalities());
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
    h.normalize_history(&default_input_modalities());
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
    h.normalize_history(&default_input_modalities());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_removes_orphan_custom_tool_call_output_panics_in_debug() {
    let items = vec![ResponseItem::CustomToolCallOutput {
        call_id: "orphan-2".to_string(),
        name: None,
        output: FunctionCallOutputPayload::from_text("ok".to_string()),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history(&default_input_modalities());
}

#[cfg(not(debug_assertions))]
#[test]
fn normalize_removes_orphan_client_tool_search_output() {
    let items = vec![ResponseItem::ToolSearchOutput {
        call_id: Some("orphan-search".to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: Vec::new(),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history(&default_input_modalities());

    assert_eq!(h.raw_items(), vec![]);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_removes_orphan_client_tool_search_output_panics_in_debug() {
    let items = vec![ResponseItem::ToolSearchOutput {
        call_id: Some("orphan-search".to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: Vec::new(),
    }];
    let mut h = create_history_with_items(items);
    h.normalize_history(&default_input_modalities());
}

#[test]
fn normalize_keeps_server_tool_search_output_without_matching_call() {
    let items = vec![ResponseItem::ToolSearchOutput {
        call_id: Some("server-search".to_string()),
        status: "completed".to_string(),
        execution: "server".to_string(),
        tools: Vec::new(),
    }];
    let mut h = create_history_with_items(items);

    h.normalize_history(&default_input_modalities());

    assert_eq!(
        h.raw_items(),
        vec![ResponseItem::ToolSearchOutput {
            call_id: Some("server-search".to_string()),
            status: "completed".to_string(),
            execution: "server".to_string(),
            tools: Vec::new(),
        }]
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic]
fn normalize_mixed_inserts_and_removals_panics_in_debug() {
    let items = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "f1".to_string(),
            namespace: None,
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
    h.normalize_history(&default_input_modalities());
}

#[test]
fn image_data_url_payload_does_not_dominate_message_estimate() {
    let payload = "A".repeat(100_000);
    let image_url = format!("data:image/png;base64,{payload}");
    let image_item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputText {
                text: "Here is the screenshot".to_string(),
            },
            ContentItem::InputImage {
                image_url,
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    };
    let text_only_item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "Here is the screenshot".to_string(),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    };

    let raw_len = serde_json::to_string(&image_item).unwrap().len() as i64;
    let estimated = estimate_response_item_model_visible_bytes(&image_item);
    let expected = raw_len - payload.len() as i64 + RESIZED_IMAGE_BYTES_ESTIMATE;
    let text_only_estimated = estimate_response_item_model_visible_bytes(&text_only_item);

    assert_eq!(estimated, expected);
    assert!(estimated < raw_len);
    assert!(estimated > text_only_estimated);
}

#[test]
fn image_data_url_payload_does_not_dominate_function_call_output_estimate() {
    let payload = "B".repeat(50_000);
    let image_url = format!("data:image/png;base64,{payload}");
    let item = ResponseItem::FunctionCallOutput {
        call_id: "call-abc".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputText {
                text: "Screenshot captured".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]),
    };

    let raw_len = serde_json::to_string(&item).unwrap().len() as i64;
    let estimated = estimate_response_item_model_visible_bytes(&item);
    let expected = raw_len - payload.len() as i64 + RESIZED_IMAGE_BYTES_ESTIMATE;

    assert_eq!(estimated, expected);
    assert!(estimated < raw_len);
}

#[test]
fn image_data_url_payload_does_not_dominate_custom_tool_call_output_estimate() {
    let payload = "C".repeat(50_000);
    let image_url = format!("data:image/png;base64,{payload}");
    let item = ResponseItem::CustomToolCallOutput {
        call_id: "call-js-repl".to_string(),
        name: None,
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputText {
                text: "Screenshot captured".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]),
    };

    let raw_len = serde_json::to_string(&item).unwrap().len() as i64;
    let estimated = estimate_response_item_model_visible_bytes(&item);
    let expected = raw_len - payload.len() as i64 + RESIZED_IMAGE_BYTES_ESTIMATE;

    assert_eq!(estimated, expected);
    assert!(estimated < raw_len);
}

#[test]
fn non_base64_image_urls_are_unchanged() {
    let message_item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputImage {
            image_url: "https://example.com/foo.png".to_string(),
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    };
    let function_output_item = ResponseItem::FunctionCallOutput {
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: "file:///tmp/foo.png".to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]),
    };

    assert_eq!(
        estimate_response_item_model_visible_bytes(&message_item),
        serde_json::to_string(&message_item).unwrap().len() as i64
    );
    assert_eq!(
        estimate_response_item_model_visible_bytes(&function_output_item),
        serde_json::to_string(&function_output_item).unwrap().len() as i64
    );
}

#[test]
fn data_url_without_base64_marker_is_unchanged() {
    let item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputImage {
            image_url: "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'/>".to_string(),
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    };

    assert_eq!(
        estimate_response_item_model_visible_bytes(&item),
        serde_json::to_string(&item).unwrap().len() as i64
    );
}

#[test]
fn non_image_base64_data_url_is_unchanged() {
    let payload = "C".repeat(4_096);
    let image_url = format!("data:application/octet-stream;base64,{payload}");
    let item = ResponseItem::FunctionCallOutput {
        call_id: "call-octet".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]),
    };

    let raw_len = serde_json::to_string(&item).unwrap().len() as i64;
    let estimated = estimate_response_item_model_visible_bytes(&item);

    assert_eq!(estimated, raw_len);
}

#[test]
fn mixed_case_data_url_markers_are_adjusted() {
    let payload = "F".repeat(1_024);
    let image_url = format!("DATA:image/png;BASE64,{payload}");
    let item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputImage {
            image_url,
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    };

    let raw_len = serde_json::to_string(&item).unwrap().len() as i64;
    let estimated = estimate_response_item_model_visible_bytes(&item);
    let expected = raw_len - payload.len() as i64 + RESIZED_IMAGE_BYTES_ESTIMATE;

    assert_eq!(estimated, expected);
}

#[test]
fn multiple_inline_images_apply_multiple_fixed_costs() {
    let payload_one = "D".repeat(100);
    let payload_two = "E".repeat(200);
    let image_url_one = format!("data:image/png;base64,{payload_one}");
    let image_url_two = format!("data:image/jpeg;base64,{payload_two}");
    let item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputText {
                text: "images".to_string(),
            },
            ContentItem::InputImage {
                image_url: image_url_one,
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
            ContentItem::InputImage {
                image_url: image_url_two,
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    };

    let raw_len = serde_json::to_string(&item).unwrap().len() as i64;
    let payload_sum = (payload_one.len() + payload_two.len()) as i64;
    let estimated = estimate_response_item_model_visible_bytes(&item);
    let expected = raw_len - payload_sum + (2 * RESIZED_IMAGE_BYTES_ESTIMATE);

    assert_eq!(estimated, expected);
}

#[test]
fn original_detail_images_scale_with_dimensions() {
    // 2304x864 at 32px patches yields 72 * 27 = 1,944 patches.
    // The byte heuristic uses 4 bytes per token, so the replacement cost is 7,776 bytes.
    const EXPECTED_ORIGINAL_DETAIL_IMAGE_BYTES: i64 = 7_776;

    let width = 2304;
    let height = 864;
    let image = ImageBuffer::from_pixel(width, height, Rgba([12u8, 34, 56, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("encode png");
    let payload = BASE64_STANDARD.encode(bytes.get_ref());
    let image_url = format!("data:image/png;base64,{payload}");
    let item = ResponseItem::FunctionCallOutput {
        call_id: "call-original".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: Some(ImageDetail::Original),
            },
        ]),
    };

    let raw_len = serde_json::to_string(&item).unwrap().len() as i64;
    let estimated = estimate_response_item_model_visible_bytes(&item);
    let expected = raw_len - payload.len() as i64 + EXPECTED_ORIGINAL_DETAIL_IMAGE_BYTES;

    assert_eq!(estimated, expected);
}

#[test]
fn original_detail_webp_images_scale_with_dimensions() {
    // Same dimensions as the PNG case above, so the patch-based replacement cost is the same.
    const EXPECTED_ORIGINAL_DETAIL_IMAGE_BYTES: i64 = 7_776;

    let width = 2304;
    let height = 864;
    let image = ImageBuffer::from_pixel(width, height, Rgba([12u8, 34, 56, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::WebP)
        .expect("encode webp");
    let payload = BASE64_STANDARD.encode(bytes.get_ref());
    let image_url = format!("data:image/webp;base64,{payload}");
    let item = ResponseItem::FunctionCallOutput {
        call_id: "call-original-webp".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: Some(ImageDetail::Original),
            },
        ]),
    };

    let raw_len = serde_json::to_string(&item).unwrap().len() as i64;
    let estimated = estimate_response_item_model_visible_bytes(&item);
    let expected = raw_len - payload.len() as i64 + EXPECTED_ORIGINAL_DETAIL_IMAGE_BYTES;

    assert_eq!(estimated, expected);
}

#[test]
fn text_only_items_unchanged() {
    let item = ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: "Hello world, this is a response.".to_string(),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    };

    let estimated = estimate_response_item_model_visible_bytes(&item);
    let raw_len = serde_json::to_string(&item).unwrap().len() as i64;

    assert_eq!(estimated, raw_len);
}
