use super::*;
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
fn long_text_delivery_is_lossless_bounded_and_fully_budgeted() {
    let text = "é🙂\"\n".repeat(/*n*/ 20_000);
    let context = ComposedContext {
        sections: vec![SectionOutput {
            id: "planned_action",
            delivery: text_content(vec![text.clone()]),
        }],
        truncations: Vec::new(),
    };
    let estimated = context.estimated_tokens();
    let messages = context.clone().into_messages();
    let ResponseItem::Message { content, .. } = &messages[0] else {
        panic!("user message")
    };
    let parts = content
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
    assert_eq!(
        context.into_user_inputs().unwrap(),
        parts
            .into_iter()
            .map(|part| UserInput::Text {
                text: part.to_owned(),
                text_elements: Vec::new(),
            })
            .collect::<Vec<_>>()
    );
}
