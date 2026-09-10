//! Aggregate budgets preserve required evidence and explicit message boundaries.

use super::*;
use crate::budget::section_tokens;
use crate::composition::user_message;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

fn text(value: &str) -> ContentItem {
    ContentItem::InputText {
        text: value.to_owned(),
    }
}

#[test]
fn planned_action_budget_omits_descriptions_without_changing_arguments() {
    let action = crate::PlannedAction {
        json: r#"{"tool":"write_record","arguments":{"description":"required payload"}}"#
            .to_owned(),
        kind: crate::PlannedActionKind::Command,
        reason: None,
        tool_descriptions: Some("optional tool description ".repeat(/*n*/ 100)),
    };
    let required = action.render(crate::ActionPresentation::SyncFull);
    let context = crate::CollectedContext {
        sections: vec![crate::ContextSection::PlannedAction(action)],
    }
    .compose(
        crate::ContextPresentation::SyncFull {
            session_id: "review",
        },
        crate::RenderedTranscript {
            items: Vec::new(),
            omission_note: None,
            truncations: Vec::new(),
        },
    )
    .unwrap();
    let mut required_items = required.iter().map(|item| text(item)).collect::<Vec<_>>();
    required_items.push(text("evidence omitted"));
    let budget = crate::estimate_input_tokens(&user_message(required_items.clone())) + 100;
    let selected = context
        .enforce_budget(
            RequestBudget {
                max_input_tokens: budget,
                existing_context_tokens: 0,
            },
            "evidence omitted".to_owned(),
        )
        .unwrap();
    // The sync preamble is also required and remains ahead of the action.
    let messages = selected.into_messages();
    let ResponseItem::Message { content, .. } = &messages[0] else {
        panic!("expected user evidence");
    };
    assert_eq!(
        &content[content.len() - required_items.len()..],
        required_items
    );
}

#[test]
fn budget_reserves_existing_context_and_preserves_required_messages() {
    let trusted = crate::PreviousReviews::try_from_fragments(vec!["verified review".to_owned()])
        .unwrap()
        .into_message();
    let image = ContentItem::InputImage {
        image_url: "data:image/png;base64,AAAA".to_owned(),
        detail: None,
    };
    let make_context = || ComposedContext {
        sections: vec![
            SectionOutput {
                id: "conversation_transcript",
                delivery: SectionDelivery::UserContent(vec![
                    Budgeted::required(text("user restriction")),
                    Budgeted::optional(
                        text(&"old commentary".repeat(/*n*/ 200)),
                        BudgetPriority::Commentary,
                    ),
                    Budgeted::optional(
                        text(&"old tool output".repeat(/*n*/ 100)),
                        BudgetPriority::Tool,
                    ),
                    Budgeted::required(text("latest tool evidence")),
                    Budgeted::optional(image.clone(), BudgetPriority::Image),
                ]),
            },
            SectionOutput {
                id: "previous_reviews",
                delivery: SectionDelivery::Message(Box::new(trusted.clone())),
            },
            SectionOutput {
                id: "planned_action",
                delivery: SectionDelivery::UserContent(vec![Budgeted::required(text(
                    "exact action",
                ))]),
            },
        ],
        truncations: Vec::new(),
    };
    let notice = SectionOutput {
        id: "budget_omission",
        delivery: SectionDelivery::UserContent(vec![Budgeted::required(text("evidence omitted"))]),
    };
    let full = make_context();
    let available = full.estimated_tokens()
        - content_tokens(&text(&"old commentary".repeat(/*n*/ 200)))
        - content_tokens(&text(&"old tool output".repeat(/*n*/ 100)))
        + section_tokens(&notice);
    let context = full
        .enforce_budget(
            RequestBudget {
                max_input_tokens: available + 2_000,
                existing_context_tokens: 2_000,
            },
            "evidence omitted".to_owned(),
        )
        .unwrap();
    assert!(context.estimated_tokens() <= available);
    assert_eq!(
        context.into_messages(),
        vec![
            user_message(vec![
                text("user restriction"),
                text("latest tool evidence"),
                image.clone()
            ]),
            trusted.clone(),
            user_message(vec![text("exact action"), text("evidence omitted")]),
        ]
    );
    let without_image = make_context()
        .enforce_budget(
            RequestBudget {
                max_input_tokens: available - content_tokens(&image),
                existing_context_tokens: 0,
            },
            "evidence omitted".to_owned(),
        )
        .unwrap();
    assert_eq!(
        without_image.into_messages(),
        vec![
            user_message(vec![text("user restriction"), text("latest tool evidence")]),
            trusted.clone(),
            user_message(vec![text("exact action"), text("evidence omitted")]),
        ]
    );
}

#[test]
fn image_omission_preserves_text_and_later_eviction_policy() {
    let evidence = text(&"optional commentary ".repeat(/*n*/ 100));
    let mut context = ComposedContext {
        sections: vec![SectionOutput {
            id: "evidence",
            delivery: SectionDelivery::UserContent(vec![
                Budgeted::optional(
                    ContentItem::InputImage {
                        image_url: "rejected-image".to_owned(),
                        detail: None,
                    },
                    BudgetPriority::Image,
                ),
                Budgeted::optional(evidence.clone(), BudgetPriority::Commentary),
                Budgeted::required(text("user restriction")),
            ]),
        }],
        truncations: Vec::new(),
    };
    let without_oversized_image = context
        .clone()
        .enforce_budget(
            RequestBudget {
                max_input_tokens: 1_000,
                existing_context_tokens: 0,
            },
            "evidence omitted".to_owned(),
        )
        .unwrap();
    assert_eq!(
        without_oversized_image.into_messages(),
        vec![user_message(vec![
            evidence.clone(),
            text("user restriction"),
            text("evidence omitted")
        ])]
    );
    context.retain_images(|_, _| false);
    let available = context.estimated_tokens();
    let retained = context
        .clone()
        .enforce_budget(
            RequestBudget {
                max_input_tokens: available,
                existing_context_tokens: 0,
            },
            "evidence omitted".to_owned(),
        )
        .unwrap();
    assert_eq!(
        retained.into_messages(),
        vec![user_message(vec![evidence, text("user restriction")])]
    );
    let smaller = context
        .enforce_budget(
            RequestBudget {
                max_input_tokens: 100,
                existing_context_tokens: 0,
            },
            "evidence omitted".to_owned(),
        )
        .unwrap();
    assert_eq!(
        smaller.into_messages(),
        vec![user_message(vec![
            text("user restriction"),
            text("evidence omitted")
        ])]
    );

    let older = ContentItem::InputImage {
        image_url: "older-image".to_owned(),
        detail: None,
    };
    let newer = ContentItem::InputImage {
        image_url: "newer-image".to_owned(),
        detail: None,
    };
    let image_section = |images: Vec<ContentItem>| SectionOutput {
        id: "transcript_images",
        delivery: SectionDelivery::UserContent(
            images
                .into_iter()
                .map(|image| Budgeted::optional(image, BudgetPriority::Image))
                .collect(),
        ),
    };
    let notice = SectionOutput {
        id: "budget_omission",
        delivery: SectionDelivery::UserContent(vec![Budgeted::required(text("evidence omitted"))]),
    };
    let available = section_tokens(&image_section(vec![newer.clone()])) + section_tokens(&notice);
    // Removing the older image frees a separator in one section, or an entire
    // wrapper in separate sections. Either way, the newer image fits exactly.
    for sections in [
        vec![image_section(vec![older.clone(), newer.clone()])],
        vec![
            image_section(vec![older]),
            image_section(vec![newer.clone()]),
        ],
    ] {
        let context = ComposedContext {
            sections,
            truncations: Vec::new(),
        }
        .enforce_budget(
            RequestBudget {
                max_input_tokens: available,
                existing_context_tokens: 0,
            },
            "evidence omitted".to_owned(),
        )
        .unwrap();
        assert_eq!(
            context.into_messages(),
            vec![user_message(vec![newer.clone(), text("evidence omitted")])]
        );
    }
}
