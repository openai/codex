use super::*;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use pretty_assertions::assert_eq;

#[test]
fn delivery_preserves_arbitrary_message_boundaries_and_rejects_them_for_sync() {
    let message = crate::PreviousReviews::try_from_fragments(vec!["host-attested review".into()])
        .unwrap()
        .into_message();
    let sections = || {
        vec![
            SectionOutput {
                id: "new_user_section",
                delivery: text_content(vec!["first".into(), "second".into()]),
            },
            SectionOutput {
                id: "new_message_section",
                delivery: SectionDelivery::Message(Box::new(message.clone())),
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
            truncations: Vec::new()
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
            message.clone(),
            user_message(vec![ContentItem::InputText {
                text: "third".into()
            }]),
        ]
    );
    assert_eq!(
        ComposedContext {
            sections: sections(),
            truncations: Vec::new()
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
            delivery: SectionDelivery::UserContent(vec![
                Budgeted::required(ContentItem::InputText { text: text.clone() }),
                Budgeted::required(ContentItem::InputImage {
                    image: file_image.clone(),
                    detail: Some(ImageDetail::High),
                }),
            ]),
        }],
        truncations: Vec::new(),
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
        parts
            .iter()
            .all(|part| part.len() <= TruncationPolicy::Tokens(9_000).byte_budget())
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
        .compose(
            presentation,
            RenderedTranscript {
                items: vec![Budgeted::optional(
                    "old tool output ".repeat(/*n*/ 2_000),
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
            vec![Budgeted::required(ContentItem::InputText {
                text: sender[0].clone()
            })]
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
