use super::*;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use pretty_assertions::assert_eq;

#[test]
fn pending_action_replays_the_encrypted_call_with_its_evidence_output() {
    let call: ResponseItem = serde_json::from_value(serde_json::json!({
        "type": "function_call",
        "namespace": "collaboration",
        "name": "send_message",
        "call_id": "send-1",
        "arguments": r#"{"target":"/root/worker","message":"opaque"}"#,
        "encrypted_content": "opaque-replay",
        "encrypted_function_args": ["message"]
    }))
    .unwrap();
    let action = crate::PlannedAction {
        kind: crate::PlannedActionKind::ToolCall(Box::new(call.clone())),
        reason: None,
        tool_descriptions: None,
    };
    let messages = ComposedContext::for_pending_action(
        action,
        /*permissions*/ None,
        crate::ContextBudgeting::Fixed,
    )
    .unwrap()
    .into_messages();
    let [ResponseItem::Message { .. }, replay, output] = messages.as_slice() else {
        panic!("expected the review instruction, exact call, and evidence-only output");
    };
    assert_eq!(replay, &call);
    assert_eq!(
        Some(output.clone()),
        crate::native_tool_call::evidence_output(&call)
    );
}

#[test]
fn elastic_pending_action_defers_the_exact_call_to_final_admission() {
    let fixed_limit_bytes = TruncationPolicy::Tokens(MAX_TEXT_PART_ESTIMATED_TOKENS).byte_budget();
    let arguments = serde_json::json!({ "content": "x".repeat(fixed_limit_bytes) });
    let call: ResponseItem = serde_json::from_value(serde_json::json!({
        "type": "function_call",
        "name": "write_record",
        "call_id": "write-1",
        "arguments": arguments.to_string(),
    }))
    .unwrap();
    let action = crate::PlannedAction {
        kind: crate::PlannedActionKind::ToolCall(Box::new(call.clone())),
        reason: None,
        tool_descriptions: None,
    };
    assert!(matches!(
        ComposedContext::for_pending_action(
            action.clone(),
            /*permissions*/ None,
            crate::ContextBudgeting::Fixed
        ),
        Err(SectionError::EvidenceLimitExceeded {
            section: "native_tool_call"
        })
    ));

    let context = ComposedContext::for_pending_action(
        action,
        /*permissions*/ None,
        crate::ContextBudgeting::Elastic,
    )
    .unwrap();
    let config =
        crate::budget::tests::complete_minimum_budget(crate::ElasticBudgetConfig::default());
    let prepared = context.prepare_elastic_entries(&config).unwrap();
    let budget = |max_input_tokens| crate::RequestBudget {
        max_input_tokens,
        existing_context_tokens: 0,
    };
    assert!(
        prepared
            .clone()
            .enforce_budget(
                budget(MAX_TEXT_PART_ESTIMATED_TOKENS),
                "Evidence omitted.".into()
            )
            .is_err()
    );
    let messages = prepared
        .enforce_budget(
            budget(crate::DEFAULT_MAX_INPUT_TOKENS),
            "Evidence omitted.".into(),
        )
        .unwrap()
        .into_messages();
    assert!(messages.contains(&call));
}

#[test]
fn delivery_preserves_arbitrary_message_boundaries_and_rejects_them_for_sync() {
    let message = crate::PreviousReviews::try_from_fragments(vec![crate::PreviousReview {
        id: codex_thread_context_ids_test_support::thread_context_ids().next_item("review"),
        fragment: "host-attested review".into(),
    }])
    .unwrap()
    .into_annotated_message();
    let sections = || {
        vec![
            SectionOutput {
                id: "new_user_section",
                delivery: text_content(vec!["first".into(), "second".into()]),
            },
            SectionOutput {
                id: "new_message_section",
                delivery: SectionDelivery::Message(crate::Budgeted::required(Box::new(
                    message.clone(),
                ))),
            },
            SectionOutput {
                id: "another_user_section",
                delivery: text_content(vec!["third".into()]),
            },
        ]
    };
    assert_eq!(
        ComposedContext {
            sections: sections(),
            truncations: Vec::new(),
            enclosing_call: None,
        }
        .into_messages(),
        vec![
            user_message(vec![
                ContentItem::InputText {
                    text: "first".into()
                },
                ContentItem::InputText {
                    text: "second".into()
                }
            ]),
            message.item.clone(),
            user_message(vec![ContentItem::InputText {
                text: "third".into()
            }]),
        ]
    );
    assert_eq!(
        ComposedContext {
            sections: sections(),
            truncations: Vec::new(),
            enclosing_call: None,
        }
        .into_user_inputs(),
        Err(SectionError::UnsupportedDelivery {
            section: "new_message_section"
        })
    );
}

#[test]
fn long_text_and_file_image_delivery_is_lossless_bounded_and_fully_budgeted() {
    let text = "é🙂\"\n".repeat(/*n*/ 20_000);
    let file_image = ImageReference::File {
        file_id: "file_123".to_owned(),
    };
    let context = ComposedContext {
        sections: vec![SectionOutput {
            id: "planned_action",
            delivery: SectionDelivery::user_content(vec![
                Budgeted::required(ContentItem::InputText { text: text.clone() }),
                Budgeted::required(ContentItem::InputImage {
                    image: file_image.clone(),
                    detail: Some(ImageDetail::High),
                }),
            ]),
        }],
        truncations: Vec::new(),
        enclosing_call: None,
    };
    let estimated = context.estimated_tokens();
    let messages = context.clone().into_messages();
    let ResponseItem::Message { content, .. } = &messages[0] else {
        panic!("user message")
    };
    let (image, text_content) = content.split_last().expect("message content");
    assert_eq!(
        image,
        &ContentItem::InputImage {
            image: file_image.clone(),
            detail: Some(ImageDetail::High),
        }
    );
    let parts = text_content
        .iter()
        .map(|item| match item {
            ContentItem::InputText { text } => text.as_str(),
            _ => panic!("text part"),
        })
        .collect::<Vec<_>>();
    assert_eq!(parts.concat(), text);
    assert!(
        parts.iter().all(|part| part.len()
            <= TruncationPolicy::Tokens(MAX_TEXT_PART_ESTIMATED_TOKENS).byte_budget())
    );
    assert!(estimated >= crate::estimate_input_tokens(&messages[0]));
    let mut expected_inputs = parts
        .into_iter()
        .map(|part| UserInput::Text {
            text: part.to_owned(),
            text_elements: Vec::new(),
        })
        .collect::<Vec<_>>();
    expected_inputs.push(UserInput::Image {
        image: file_image,
        detail: Some(ImageDetail::High),
    });
    assert_eq!(context.into_user_inputs().unwrap(), expected_inputs);
}

#[test]
fn sender_restrictions_survive_budget_trimming_for_both_reviewers() {
    for presentation in [
        ContextPresentation::SyncFull {
            session_id: "receiver",
        },
        ContextPresentation::SyncDelta {
            session_id: "receiver",
        },
        ContextPresentation::Async,
    ] {
        let sender = vec!["user: Only use the staging pool.\n".to_owned()];
        let context = CollectedContext {
            sections: vec![
                ContextSection::SenderUserMessages {
                    items: sender.clone(),
                },
                ContextSection::ConversationTranscript { items: Vec::new() },
            ],
        }
        .compose_prepared(
            presentation,
            PreparedTranscript {
                items: vec![Budgeted::optional(
                    crate::TranscriptContent::Text("old tool output ".repeat(/*n*/ 2_000)),
                    BudgetPriority::Tool,
                )],
                omission_note: None,
                truncations: Vec::new(),
            },
        )
        .unwrap();
        let selected = context
            .clone()
            .enforce_budget(
                crate::RequestBudget {
                    max_input_tokens: 1_000,
                    existing_context_tokens: 0,
                },
                "evidence omitted".to_owned(),
                crate::HistoryTruncation::Allow,
            )
            .unwrap();
        assert!(selected.estimated_tokens() <= 1_000);
        assert_eq!(selected.truncations.len(), 1);
        let sender_section = selected
            .sections
            .into_iter()
            .find(|section| section.id == "sender_user_messages")
            .unwrap();
        let SectionDelivery::UserContent(items) = sender_section.delivery else {
            panic!("sender context must remain user evidence");
        };
        assert_eq!(
            items,
            vec![Budgeted::required(SectionContent::Other(
                ContentItem::InputText {
                    text: sender[0].clone()
                }
            ))]
        );
        assert!(matches!(
            context.enforce_budget(
                crate::RequestBudget {
                    max_input_tokens: 1,
                    existing_context_tokens: 0
                },
                "evidence omitted".to_owned(),
                crate::HistoryTruncation::Allow,
            ),
            Err(SectionError::EvidenceLimitExceeded {
                section: "request_budget"
            })
        ));
    }
}
