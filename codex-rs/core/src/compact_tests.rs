use super::*;
use crate::session::tests::make_session_and_context_with_auth_and_config_and_rx;
use codex_features::Feature;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::ResponseItemId;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::ExecutedToolCall;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageReference;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[test_case(true; "metadata enabled")]
#[test_case(false; "metadata disabled after capture")]
#[tokio::test]
async fn local_compaction_respects_tool_metadata_state(
    metadata_enabled: bool,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let provider = ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())));
    let (session, turn, _events) = make_session_and_context_with_auth_and_config_and_rx(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        Vec::new(),
        move |config| {
            config.model = Some("gpt-5.2".to_string());
            config.model_provider = provider;
            config.model_provider.supports_websockets = false;
            config
                .features
                .enable(Feature::ExecutedToolCallMetadata)
                .expect("enable tool-call metadata");
        },
    )
    .await;

    let mut items = vec![user_message("Update the plan")];
    for index in 0..5 {
        let call_id = format!("direct-{index}");
        let arguments = json!({"plan": [{"step": "x".repeat(7 * 1024), "status": "completed"}]});
        assert!(serde_json::to_vec(&arguments)?.len() < 8 * 1024);
        items.push(ResponseItem::FunctionCall {
            id: None,
            name: "update_plan".to_string(),
            namespace: None,
            arguments: arguments.to_string(),
            encrypted_function_args: None,
            call_id: call_id.clone(),
            internal_chat_message_metadata_passthrough: None,
        });
        let mut output = ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some(call_id),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text("Plan updated".to_string()),
            internal_chat_message_metadata_passthrough: Some(
                InternalChatMessageMetadataPassthrough {
                    turn_id: Some("metadata-canary".to_string()),
                    create_time: Some(123.into()),
                    ..Default::default()
                },
            ),
        };
        output.append_executed_tool_calls(vec![ExecutedToolCall::new(
            "update_plan".to_string(),
            arguments,
        )]);
        output.mark_tool_calls_complete();
        items.push(output);
    }
    session
        .record_conversation_items(&turn, turn.model_info(), &items)
        .await;
    let live_history = session.clone_history().await;
    let outputs = live_history
        .raw_items()
        .filter_map(|item| match item {
            ResponseItem::FunctionCallOutput { .. } => Some(serde_json::to_value(item)),
            _ => None,
        })
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(outputs.len(), 5);
    let metadata_bytes: usize = outputs
        .iter()
        .map(|item| {
            serde_json::to_vec(&item["internal_chat_message_metadata_passthrough"])
                .unwrap()
                .len()
        })
        .sum();
    // Compaction does not rebudget source records as a normal inference request.
    // Passthrough bytes are also excluded from model token estimates.
    assert!(metadata_bytes > 32 * 1024);

    if !metadata_enabled {
        let mut config = (*session.get_config().await).clone();
        config.features.disable(Feature::ExecutedToolCallMetadata)?;
        session.refresh_runtime_config(config).await;
    }

    let mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("summary", "The prior calls finished."),
            responses::ev_completed("compact-response"),
        ]),
    )
    .await;
    // OpenAI identity keeps the client from removing passthrough for compatibility.
    run_compact_task(
        Arc::clone(&session),
        turn,
        vec![UserInput::Text {
            text: "Summarize the conversation.".to_string(),
            text_elements: Vec::new(),
        }],
    )
    .await?;

    let request = mock.single_request();
    assert!(request.inputs_of_type("compaction_trigger").is_empty());
    for mut output in outputs {
        let call_id = output["call_id"].as_str().expect("source call id");
        let compact_output = request.function_call_output(call_id);
        assert_eq!(compact_output["output"], json!("Plan updated"));
        if !metadata_enabled {
            let metadata = output["internal_chat_message_metadata_passthrough"]
                .as_object_mut()
                .expect("source metadata");
            metadata.remove("executed_tool_calls");
            metadata.remove("tool_calls_complete");
        }
        assert_eq!(
            compact_output["internal_chat_message_metadata_passthrough"],
            output["internal_chat_message_metadata_passthrough"]
        );
    }
    let compacted_history = session.clone_history().await;
    let expected_summary = format!("{SUMMARY_PREFIX}\nThe prior calls finished.");
    assert!(compacted_history.raw_items().any(|item| {
        matches!(item, ResponseItem::Message { role, content, .. }
            if role == "user"
                && content_items_to_text(content).as_deref() == Some(expected_summary.as_str()))
    }));
    Ok(())
}

fn annotated(items: Vec<ResponseItem>) -> Vec<ResponseItemEnvelope> {
    items.into_iter().map(ResponseItemEnvelope::new).collect()
}

fn raw(items: Vec<ResponseItemEnvelope>) -> Vec<ResponseItem> {
    items
        .into_iter()
        .map(ResponseItemEnvelope::into_item)
        .collect()
}

fn user_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn compacted_user_message(text: &str) -> CompactedUserMessage {
    CompactedUserMessage {
        id: None,
        message: text.to_string(),
        internal_chat_message_metadata_passthrough: None,
        harness_metadata: None,
    }
}

#[test]
fn content_items_to_text_joins_non_empty_segments() {
    let items = vec![
        ContentItem::InputText {
            text: "hello".to_string(),
        },
        ContentItem::OutputText {
            text: String::new(),
        },
        ContentItem::OutputText {
            text: "world".to_string(),
        },
    ];

    let joined = content_items_to_text(&items);

    assert_eq!(Some("hello\nworld".to_string()), joined);
}

#[test]
fn content_items_to_text_ignores_image_only_content() {
    let items = vec![ContentItem::InputImage {
        image: ImageReference::Inline {
            image_url: "file://image.png".to_string(),
        },
        detail: Some(DEFAULT_IMAGE_DETAIL),
    }];

    let joined = content_items_to_text(&items);

    assert_eq!(None, joined);
}

#[test]
fn collect_user_messages_extracts_user_text_only() {
    let items = vec![
        ResponseItem::Message {
            id: Some(ResponseItemId::with_suffix("msg", "assistant")),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "ignored".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: Some(ResponseItemId::with_suffix("msg", "user")),
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "first".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Other,
    ];

    let collected = collect_user_messages(&items);

    assert_eq!(
        vec![CompactedUserMessage {
            id: Some(ResponseItemId::with_suffix("msg", "user")),
            ..compacted_user_message("first")
        }],
        collected,
    );
}

#[test]
fn collect_annotated_user_messages_extracts_user_text_only() {
    let items = vec![
        ResponseItemEnvelope {
            item: user_message("first"),
            metadata: Some(CodexHarnessMetadata::default()),
        },
        ResponseItemEnvelope::new(ResponseItem::Other),
    ];

    let collected = collect_annotated_user_messages(&items, CompactedMessageIdentity::Preserve);

    assert_eq!(
        vec![CompactedUserMessage {
            id: None,
            message: "first".to_string(),
            internal_chat_message_metadata_passthrough: None,
            harness_metadata: Some(CodexHarnessMetadata::default()),
        }],
        collected
    );
}

#[test]
fn collect_user_messages_filters_session_prefix_entries() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: r#"# AGENTS.md instructions for project

<INSTRUCTIONS>
do things
</INSTRUCTIONS>"#
                    .to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "<ENVIRONMENT_CONTEXT>cwd=/tmp</ENVIRONMENT_CONTEXT>".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "real user message".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    let collected = collect_user_messages(&items);

    assert_eq!(vec![compacted_user_message("real user message")], collected);
}

#[test]
fn collect_user_messages_filters_legacy_warnings() {
    let items = vec![
        user_message(
            "Warning: The maximum number of unified exec processes you can keep open is 60 and you currently have 61 processes open. Reuse older processes or close them to prevent automatic pruning of old processes",
        ),
        user_message(
            "Warning: apply_patch was requested via exec_command. Use the apply_patch tool instead of exec_command.",
        ),
        user_message(
            "Warning: Your account was flagged for potentially high-risk cyber activity and this request was routed to gpt-5.2 as a fallback. To regain access to gpt-5.3-codex, apply for trusted access: https://chatgpt.com/cyber or learn more: https://developers.openai.com/codex/concepts/cyber-safety",
        ),
        user_message("real user message"),
    ];

    let collected = collect_user_messages(&items);

    assert_eq!(vec![compacted_user_message("real user message")], collected);
}

#[test]
fn build_token_limited_compacted_history_truncates_overlong_user_messages() {
    // Use a small truncation limit so the test remains fast while still validating
    // that oversized user content is truncated.
    let max_tokens = 16;
    let big = "word ".repeat(200);
    let user_message = CompactedUserMessage {
        id: Some(ResponseItemId::with_suffix("msg", "long-user")),
        message: big.clone(),
        internal_chat_message_metadata_passthrough: None,
        harness_metadata: Some(CodexHarnessMetadata::default()),
    };
    let history = super::build_compacted_history_with_limit(
        Vec::new(),
        std::slice::from_ref(&user_message),
        "SUMMARY",
        max_tokens,
    );
    assert_eq!(history.len(), 2);

    let truncated_message = &history[0].item;
    let summary_message = &history[1].item;

    let truncated_text = match truncated_message {
        ResponseItem::Message { role, content, .. } if role == "user" => {
            content_items_to_text(content).unwrap_or_default()
        }
        other => panic!("unexpected item in history: {other:?}"),
    };

    assert!(
        truncated_text.contains("tokens truncated"),
        "expected truncation marker in truncated user message"
    );
    assert!(
        !truncated_text.contains(&big),
        "truncated user message should not include the full oversized user text"
    );

    let summary_text = match summary_message {
        ResponseItem::Message { role, content, .. } if role == "user" => {
            content_items_to_text(content).unwrap_or_default()
        }
        other => panic!("unexpected item in history: {other:?}"),
    };
    assert_eq!(summary_text, "SUMMARY");
    assert_eq!(history[0].id(), user_message.id.as_ref());
    assert_eq!(history[0].metadata, Some(CodexHarnessMetadata::default()));
    assert_eq!(history[1].metadata, None);
}

#[test]
fn build_token_limited_compacted_history_appends_summary_message() {
    let initial_context: Vec<ResponseItemEnvelope> = Vec::new();
    let user_messages = vec![compacted_user_message("first user message")];
    let summary_text = "summary text";

    let history = build_compacted_history(initial_context, &user_messages, summary_text);
    assert!(
        !history.is_empty(),
        "expected compacted history to include summary"
    );

    let last = history.last().expect("history should have a summary entry");
    let summary = match &last.item {
        ResponseItem::Message { role, content, .. } if role == "user" => {
            content_items_to_text(content).unwrap_or_default()
        }
        other => panic!("expected summary message, found {other:?}"),
    };
    assert_eq!(summary, summary_text);
}

#[test]
fn build_compacted_history_preserves_user_message_passthrough_metadata() {
    let history = build_compacted_history(
        Vec::new(),
        &[CompactedUserMessage {
            id: Some(ResponseItemId::with_suffix("msg", "user")),
            message: "first user message".to_string(),
            internal_chat_message_metadata_passthrough: Some(
                InternalChatMessageMetadataPassthrough {
                    turn_id: Some("turn-1".to_string()),
                    content_item_kinds: Some(vec![
                        ContentItemKind("user.image".to_string()),
                        ContentItemKind("user.text".to_string()),
                        ContentItemKind("user.audio".to_string()),
                    ]),
                    ..Default::default()
                },
            ),
            harness_metadata: Some(CodexHarnessMetadata::default()),
        }],
        "summary text",
    );

    assert_eq!(
        history,
        vec![
            ResponseItemEnvelope {
                item: ResponseItem::Message {
                    id: Some(ResponseItemId::with_suffix("msg", "user")),
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "first user message".to_string(),
                    }],
                    phase: None,
                    internal_chat_message_metadata_passthrough: Some(
                        InternalChatMessageMetadataPassthrough {
                            turn_id: Some("turn-1".to_string()),
                            content_item_kinds: Some(vec![ContentItemKind(
                                "user.text".to_string()
                            )]),
                            ..Default::default()
                        },
                    ),
                },
                metadata: Some(CodexHarnessMetadata::default()),
            },
            ResponseItemEnvelope::new(ContextualUserFragment::into(CompactionSummary::new(
                "summary text",
            ))),
        ]
    );
}

#[test]
fn insert_initial_context_before_last_real_user_or_summary_keeps_summary_last() {
    let agent_completion = ResponseItem::AgentMessage {
        id: None,
        author: "child".to_string(),
        recipient: "parent".to_string(),
        content: vec![AgentMessageInputContent::InputText {
            text: "Message Type: FINAL_ANSWER\nPayload:\nchild completion".to_string(),
        }],
        internal_chat_message_metadata_passthrough: None,
    };
    let compacted_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "older user".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "latest user".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        agent_completion.clone(),
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("{SUMMARY_PREFIX}\nsummary text"),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let initial_context = vec![ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: "fresh permissions".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    let refreshed = raw(insert_initial_context_before_last_real_user_or_summary(
        annotated(compacted_history),
        annotated(initial_context),
    ));
    let expected = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "older user".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: "fresh permissions".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "latest user".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        agent_completion,
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("{SUMMARY_PREFIX}\nsummary text"),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    assert_eq!(refreshed, expected);
}

#[test]
fn insert_initial_context_before_last_real_user_or_summary_keeps_compaction_last() {
    let agent_task = ResponseItem::AgentMessage {
        id: None,
        author: "parent".to_string(),
        recipient: "child".to_string(),
        content: Vec::new(),
        internal_chat_message_metadata_passthrough: None,
    };
    let compacted_history = vec![
        agent_task.clone(),
        ResponseItem::Compaction {
            id: None,
            encrypted_content: "encrypted".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let initial_context = vec![ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: "fresh permissions".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    let refreshed = raw(insert_initial_context_before_last_real_user_or_summary(
        annotated(compacted_history),
        annotated(initial_context),
    ));
    let expected = vec![
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: "fresh permissions".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        agent_task,
        ResponseItem::Compaction {
            id: None,
            encrypted_content: "encrypted".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    assert_eq!(refreshed, expected);
}
