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
                delivery: SectionDelivery::UserContent(vec![
                    Budgeted::required(ContentItem::InputText {
                        text: "évidence".to_owned(),
                    }),
                    Budgeted::required(ContentItem::InputImage {
                        image: ImageReference::Inline {
                            image_url: "data:image/png;base64,AAAA".to_owned(),
                        },
                        detail: None,
                    }),
                ]),
            },
            SectionOutput {
                id: "trusted",
                delivery: SectionDelivery::Message(Box::new(message(vec![
                    ContentItem::InputText {
                        text: "verified".to_owned(),
                    },
                ]))),
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
                    image_count: 1
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
}

#[test]
fn section_estimate_bounds_the_delivered_message() {
    // Individually rounded costs do not always leave room for JSON separators.
    for text in ["x", "quoted \"text\"", "évidence"] {
        for count in 1..=40 {
            let context = ComposedContext {
                sections: vec![SectionOutput {
                    id: "transcript",
                    delivery: SectionDelivery::UserContent(vec![
                        Budgeted::required(
                            ContentItem::InputText {
                                text: text.to_owned(),
                            }
                        );
                        count
                    ]),
                }],
                truncations: Vec::new(),
            };
            let estimate = context.estimated_tokens();
            let delivered = context.into_messages();
            assert!(estimate >= estimate_input_tokens(&delivered[0]));
        }
    }
}
