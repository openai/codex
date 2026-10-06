//! Costs distinguish evidence payloads from complete model request estimates.

use super::*;
use crate::Budgeted;
use crate::composition::SectionOutput;
use crate::composition::user_message as message;
use codex_protocol::models::ImageReference;
use pretty_assertions::assert_eq;

#[test]
fn section_costs_keep_multimodal_payloads_separate() {
    let context = ComposedContext {
        sections: vec![
            SectionOutput {
                id: "transcript",
                delivery: SectionDelivery::user_content(vec![
                    Budgeted::required(ContentItem::InputText {
                        text: "évidence".to_owned(),
                    }),
                    Budgeted::required(ContentItem::InputImage {
                        image: ImageReference::Inline {
                            image_url: "data:image/png;base64,AAAA".to_owned(),
                        },
                        detail: None,
                    }),
                    Budgeted::required(ContentItem::InputImage {
                        image: ImageReference::File {
                            file_id: "file_123".to_owned(),
                        },
                        detail: None,
                    }),
                ]),
            },
            SectionOutput {
                id: "trusted",
                delivery: SectionDelivery::Message(crate::Budgeted::required(Box::new(
                    message(vec![ContentItem::InputText {
                        text: "verified".to_owned(),
                    }])
                    .into(),
                ))),
            },
        ],
        truncations: Vec::new(),
    };
    assert_eq!(
        context.section_costs().collect::<Vec<_>>(),
        vec![
            (
                "transcript",
                SectionCost {
                    text_bytes: "évidence".len(),
                    image_bytes: "data:image/png;base64,AAAA".len(),
                    image_count: 2
                }
            ),
            (
                "trusted",
                SectionCost {
                    text_bytes: "verified".len(),
                    ..SectionCost::default()
                }
            ),
        ]
    );
}

#[test]
fn request_estimate_reserves_images_independently_of_encoded_size() {
    let image = |payload: &str| {
        message(vec![ContentItem::InputImage {
            image: ImageReference::Inline {
                image_url: format!("data:image/png;base64,{payload}"),
            },
            detail: None,
        }])
    };
    let short = estimate_input_tokens(&image("AAAA"));
    assert_eq!(short, estimate_input_tokens(&image(&"A".repeat(200_000))));
    assert!(short >= IMAGE_TOKEN_RESERVATION);

    let file = message(vec![ContentItem::InputImage {
        image: ImageReference::File {
            file_id: "file_123".to_owned(),
        },
        detail: Some(codex_protocol::models::ImageDetail::Original),
    }]);
    assert!(estimate_input_tokens(&file) >= IMAGE_TOKEN_RESERVATION);
}

#[test]
fn section_estimate_bounds_the_delivered_message() {
    // Account for both JSON escaping layers and UTF-8-safe chunk boundaries.
    for format in [crate::TranscriptFormat::Line, crate::TranscriptFormat::Json] {
        for (text, max_count) in [
            ("x".to_owned(), 40),
            ("quoted \"text\"".to_owned(), 40),
            ("é🙂\n\0\\".repeat(20_000), 1),
        ] {
            let mut record = crate::TranscriptRecord::new(
                &crate::ConversationTranscriptEntryKind::Assistant,
                /*index*/ 7,
                text,
                /*retained_source_order*/ None,
                format,
                /*suffix*/ "",
            );
            // Composition changes framing after preparation has populated the cache.
            record.frame_for_sync("\n");
            let expected = ContentItem::InputText {
                text: record.to_string(),
            };
            assert_eq!(record.rendered().tokens, content_tokens(&expected));
            assert_eq!(record.clone().into_content_item(), expected);
            // Individually rounded costs must still leave room for separators.
            for count in 1..=max_count {
                let context = ComposedContext {
                    sections: vec![SectionOutput {
                        id: "transcript",
                        delivery: SectionDelivery::UserContent(vec![
                            Budgeted::required(
                                record.clone().into()
                            );
                            count
                        ]),
                    }],
                    truncations: Vec::new(),
                };
                assert_eq!(
                    context.section_costs().collect::<Vec<_>>(),
                    vec![(
                        "transcript",
                        SectionCost {
                            text_bytes: record.to_string().len() * count,
                            ..SectionCost::default()
                        }
                    )]
                );
                let estimate = context.estimated_tokens();
                assert!(estimate >= estimate_input_tokens(&context.into_messages()[0]));
            }
        }
    }
}
