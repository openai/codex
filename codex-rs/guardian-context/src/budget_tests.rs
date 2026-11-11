//! Costs distinguish evidence payloads from complete model request estimates.

use super::*;
use crate::Budgeted;
use crate::composition::SectionOutput;
use crate::composition::user_message as message;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ImageReference;
use pretty_assertions::assert_eq;

/// Keep allocation fixtures focused on their exercised sections. Trusted skills
/// are absent in these sync fixtures and return their entire reservation.
pub(crate) fn complete_minimum_budget(mut config: ElasticBudgetConfig) -> ElasticBudgetConfig {
    let section = ElasticSection::TrustedSkills;
    let limits = config.section_limits(section);
    let minimum = limits.min_total_section_tokens() + DEFAULT_MAX_INPUT_TOKENS
        - usize::try_from(config.total_minimum_tokens()).unwrap();
    config.sections.insert(
        section,
        ElasticSectionLimits::new(minimum, limits.max_section_entry_tokens()).unwrap(),
    );
    config.validate().unwrap();
    config
}

#[test]
fn request_token_ranges_keep_boundary_values_in_the_lower_bucket() {
    let tokens = [
        0,
        19_999,
        20_000,
        20_001,
        40_000,
        40_001,
        240_000,
        240_001,
        256_000,
        256_001,
        usize::MAX,
    ];
    assert_eq!(
        tokens.map(request_token_bucket),
        [
            "01_0-20k",
            "01_0-20k",
            "01_0-20k",
            "02_20k-40k",
            "02_20k-40k",
            "03_40k-60k",
            "12_220k-240k",
            "13_240k-256k",
            "13_240k-256k",
            "14_256k-max",
            "14_256k-max"
        ]
    );
}

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
                    Budgeted::required(ContentItem::InputAudio {
                        audio: AudioReference::Inline {
                            audio_url: "data:audio/wav;base64,AAAA".to_owned(),
                        },
                    }),
                    Budgeted::required(ContentItem::InputAudio {
                        audio: AudioReference::File {
                            file_id: "file_audio".to_owned(),
                        },
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
                    text_bytes: "évidence".len() + "data:audio/wav;base64,AAAA".len(),
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

#[test]
fn elastic_section_config_validates_entry_limits() {
    let mut policy = complete_minimum_budget(ElasticBudgetConfig::default());
    assert_eq!(policy.validate(), Ok(()));
    const RESERVATION: usize = 2_000;
    let mut with_selected_exec = policy.clone();
    with_selected_exec.sections.insert(
        ElasticSection::TranscriptSelectedExecResults,
        ElasticSectionLimits::new(RESERVATION, RESERVATION).unwrap(),
    );
    assert_eq!(
        with_selected_exec.total_minimum_tokens(),
        policy.total_minimum_tokens() + RESERVATION as u128
    );
    let section = ElasticSection::TrustedSkills;
    let limits = policy.section_limits(section);
    const BELOW_MINIMUM: isize = -1;
    for adjustment in [BELOW_MINIMUM, 1] {
        policy.sections.insert(
            section,
            ElasticSectionLimits::new(
                limits
                    .min_total_section_tokens()
                    .checked_add_signed(adjustment)
                    .unwrap(),
                limits.max_section_entry_tokens(),
            )
            .unwrap(),
        );
        assert_eq!(
            policy.validate(),
            Err(crate::SectionError::SectionMinimumTotalMismatch {
                total: (DEFAULT_MAX_INPUT_TOKENS as u128)
                    .checked_add_signed(adjustment as i128)
                    .unwrap(),
            })
        );
    }
    let cap = crate::composition::MAX_TEXT_PART_ESTIMATED_TOKENS;
    let config = serde_json::json!({
        "sections": {"transcript_tools": {
            "min_total_section_tokens": cap,
            "max_section_entry_tokens": cap,
        }},
    });
    let parsed = serde_json::from_value::<ElasticBudgetConfig>(config.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), config);
    for (minimum, entry_cap) in [(0, cap), (cap, 0), (cap - 1, cap), (cap + 1, cap + 1)] {
        let mut invalid = config.clone();
        invalid["sections"]["transcript_tools"]["min_total_section_tokens"] = minimum.into();
        invalid["sections"]["transcript_tools"]["max_section_entry_tokens"] = entry_cap.into();
        assert!(serde_json::from_value::<ElasticBudgetConfig>(invalid).is_err());
    }
}
