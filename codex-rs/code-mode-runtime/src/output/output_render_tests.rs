#![cfg(test)]

use super::render_output;
use crate::output::CollectedOutput;
use crate::output::OutputGap;
use crate::output::OutputOmission;
use crate::output::output_collection::BoundedOutput;
use crate::output::output_collection::MAX_AUDIO_BYTES;
use crate::output::output_collection::MAX_OUTPUT_BYTES;
use codex_code_mode_protocol::DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL;
use codex_code_mode_protocol::FunctionCallOutputContentItem as Item;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::host::BOUNDED_OUTPUT_MAX_IMAGE_BYTES;
use codex_utils_output_truncation::OutputBoundary;
use pretty_assertions::assert_eq;

fn text(value: &str) -> Item {
    Item::InputText {
        text: value.to_owned(),
    }
}

#[test]
fn missing_budget_uses_the_protocol_default() {
    let limit = DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL;
    for (extra, count) in [(0, 2), (1, 1)] {
        let items = vec![text("a"), text(&"b".repeat(4 * limit - 2 + extra))];
        let rendered = render_output(items.clone().into(), /*max_output_tokens*/ None);
        assert_eq!(rendered.len(), count);
        assert_eq!(rendered, render_output(items.into(), Some(limit)));
    }
}

#[tokio::test]
async fn typed_media_round_trips_with_the_audio_estimate() {
    let mut images: Vec<_> = [
        None,
        Some(ImageDetail::Auto),
        Some(ImageDetail::Low),
        Some(ImageDetail::High),
        Some(ImageDetail::Original),
    ]
    .map(|detail| Item::InputImage {
        image_url: "img:one".to_owned(),
        detail,
    })
    .into();
    // One 16-bit PCM sample at 24 kHz costs one token.
    let wav =
        "data:audio/wav;base64,UklGRiYAAABXQVZFZm10IBAAAAABAAEAwF0AAIC7AAACABAAZGF0YQIAAAAAAA==";
    let mut items = images.clone();
    items.push(Item::InputAudio {
        audio_url: wav.to_owned(),
    });
    assert_eq!(render_output(items.clone().into(), Some(1)), items);
    images.push(text("[omitted 1 audio items ...]"));
    assert_eq!(render_output(items.into(), Some(0)), images);
}

#[test]
fn prior_image_or_audio_loss_forces_mixed_formatting_without_surviving_media() {
    for (images, audio) in [(1, 0), (0, 1)] {
        let collected = CollectedOutput {
            items: vec![text("1234"), text("5678")],
            gaps: vec![OutputGap {
                boundary: OutputBoundary {
                    item_index: 1,
                    text_offset: 0,
                },
                omitted: OutputOmission {
                    text_bytes: 11,
                    text_items: 2,
                    image_items: images,
                    audio_items: audio,
                },
            }],
        };
        let notice = text(&format!(
            "[Code Mode session output limit reached: omitted 11 text bytes; discarded 2 text segments, {images} images, {audio} audio items]"
        ));
        assert_eq!(
            render_output(collected, Some(1)),
            [text("1234"), notice, text("[omitted 1 text items ...]")]
        );
    }
}

#[test]
fn alternating_lost_audio_and_empty_text_yield_one_notice_at_zero_budget() {
    // Start at the renderer boundary so the regression doesn't allocate 16 GiB of rejected audio.
    let count = 16_384;
    let collected = CollectedOutput {
        items: vec![text(""); count],
        gaps: (0..count)
            .map(|item_index| OutputGap {
                boundary: OutputBoundary {
                    item_index,
                    text_offset: 0,
                },
                omitted: OutputOmission {
                    audio_items: 1,
                    ..OutputOmission::default()
                },
            })
            .collect(),
    };
    assert_eq!(
        render_output(collected, Some(0)),
        [text(
            "[Code Mode session output limit reached: omitted 0 text bytes; discarded 0 text segments, 0 images, 16384 audio items]"
        )]
    );
}

#[test]
fn scattered_loss_is_summarized_once_without_joining_text_across_a_gap() {
    let items: Vec<_> = (0..128)
        .map(|n| Item::InputImage {
            image_url: format!("img:{n}"),
            detail: None,
        })
        .collect();
    let collected = CollectedOutput {
        gaps: (0..items.len())
            .map(|item_index| OutputGap {
                boundary: OutputBoundary {
                    item_index,
                    text_offset: 0,
                },
                omitted: OutputOmission {
                    text_bytes: 5,
                    text_items: 1,
                    image_items: 2,
                    audio_items: 3,
                },
            })
            .collect(),
        items: items.clone(),
    };
    let mut expected = items.clone();
    expected.push(text(
        "[Code Mode session output limit reached: across this observation, omitted 640 text bytes; discarded 128 text segments, 256 images, 384 audio items]",
    ));
    assert_eq!(render_output(collected.clone(), Some(0)), expected);

    let mut collected = collected;
    collected.gaps.push(OutputGap {
        boundary: OutputBoundary {
            item_index: collected.items.len(),
            text_offset: "left".len(),
        },
        omitted: OutputOmission {
            text_bytes: 9,
            ..OutputOmission::default()
        },
    });
    collected.items.push(text("leftright"));
    let mut expected = items;
    expected.extend([
        text("left"),
        text(
            "[Code Mode session output limit reached: across this observation, omitted 649 text bytes; discarded 128 text segments, 256 images, 384 audio items]",
        ),
        text("right"),
    ]);
    assert_eq!(render_output(collected, Some(16)), expected);
}

#[test]
fn text_only_split_keeps_the_loss_notice_inside_the_rendered_text() {
    let mut out = BoundedOutput::default();
    let before = "🦀".repeat(MAX_OUTPUT_BYTES / 8);
    let after = "🎻".repeat(MAX_OUTPUT_BYTES / 8);
    out.push(text(&format!("{before}lost{after}")));
    let rendered = render_output(out.into_collected_output(), Some(2));
    let [Item::InputText { text: prefix }, notice, suffix] = rendered.as_slice() else {
        panic!("expected a split")
    };
    assert!(
        prefix.starts_with("Warning: truncated output")
            && prefix.contains('🦀')
            && !prefix.contains('🎻')
    );
    assert_eq!(
        *notice,
        text(
            "[Code Mode session output limit reached: omitted 4 text bytes; discarded 0 text segments, 0 images, 0 audio items]"
        )
    );
    assert_eq!(*suffix, text("🎻"));
}

#[test]
fn loss_notices_remain_in_source_order_across_budgets_and_unrelated_text() {
    let before = "🦀".repeat(MAX_OUTPUT_BYTES / 8);
    let after = "🎻".repeat(MAX_OUTPUT_BYTES / 8);
    let original = format!("{before}{}{after}", "x".repeat(10_000));
    let image = Item::InputImage {
        image_url: "data:image/png;base64,AA==".to_owned(),
        detail: Some(ImageDetail::Original),
    };
    let audio_notice = text(
        "[Code Mode session output limit reached: omitted 0 text bytes; discarded 0 text segments, 0 images, 1 audio items]",
    );
    let image_notice = text(
        "[Code Mode session output limit reached: omitted 0 text bytes; discarded 0 text segments, 1 images, 0 audio items]",
    );
    // Making room for the extra byte evicts one four-byte character from the suffix.
    for (neighbor, lost) in [(None, 10000), (Some(""), 10000), (Some("x"), 10004)] {
        let split_notice = text(&format!(
            "[Code Mode session output limit reached: omitted {lost} text bytes; discarded 0 text segments, 0 images, 0 audio items]"
        ));
        let mut out = BoundedOutput::default();
        out.push(Item::InputAudio {
            audio_url: "a".repeat(MAX_AUDIO_BYTES + 1),
        });
        out.push(text(&original));
        out.push(Item::InputImage {
            image_url: "i".repeat(BOUNDED_OUTPUT_MAX_IMAGE_BYTES + 1),
            detail: None,
        });
        out.push(image.clone());
        if let Some(neighbor) = neighbor {
            out.push(text(neighbor));
        }
        let collected = out.into_collected_output();
        let positions: Vec<_> = collected
            .gaps
            .iter()
            .map(|gap| (gap.boundary.item_index, gap.boundary.text_offset))
            .collect();
        assert_eq!(positions, [(0, 0), (0, before.len()), (1, 0)]);
        for budget in [0, 1, 2, MAX_OUTPUT_BYTES] {
            let rendered = render_output(collected.clone(), Some(budget));
            if budget == 0 {
                assert_eq!(
                    &rendered[..2],
                    &[
                        text(&format!(
                            "[Code Mode session output limit reached: omitted {lost} text bytes; discarded 0 text segments, 1 images, 1 audio items]"
                        )),
                        image.clone()
                    ]
                );
                continue;
            }
            if budget == 1 {
                // Neither four-byte character fits its two-byte half. The split after
                // Core's marker and the loss before the image are now the same position.
                assert_eq!(&rendered[0], &audio_notice);
                assert_eq!(
                    &rendered[2..4],
                    &[
                        text(&format!(
                            "[Code Mode session output limit reached: omitted {lost} text bytes; discarded 0 text segments, 1 images, 0 audio items]"
                        )),
                        image.clone()
                    ]
                );
                continue;
            }
            let [
                first,
                Item::InputText { text: left },
                between,
                Item::InputText { text: right },
                last,
                preserved,
                tail @ ..,
            ] = rendered.as_slice()
            else {
                panic!(
                    "expected three notices around split text, neighbor={neighbor:?}, budget={budget}"
                )
            };
            assert_eq!(
                (first, between, last, preserved),
                (&audio_notice, &split_notice, &image_notice, &image)
            );
            assert!(left.starts_with('🦀') && !left.contains('🎻'));
            assert!(right.ends_with('🎻') && !right.contains('🦀'));
            assert_eq!(tail.len(), usize::from(neighbor == Some("x")));
        }
    }
}
