//! Applies the outer Code Mode response budget at an observation boundary.

use codex_code_mode_protocol::DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL;
use codex_code_mode_protocol::FunctionCallOutputContentItem as OutputItem;
use codex_code_mode_protocol::ImageDetail;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail as ProtocolImageDetail;
use codex_protocol::models::ImageReference;
use codex_utils_audio::estimate_audio_token_count_without_cache;
use codex_utils_output_truncation::OutputBoundary;
use codex_utils_output_truncation::formatted_output_at_token_boundaries;
use codex_utils_output_truncation::mixed_output_at_token_boundaries;

use crate::output::CollectedOutput;
use crate::output::OutputGap;
use crate::output::OutputOmission;

// Even with four full-width u64 counts per notice, this adds less than 1 KiB.
const MAX_POSITIONED_NOTICES: usize = 4;

/// Renders the output since the preceding observation, using that request's
/// own budget. The terminal error participates in the same budget as text
/// emitted by the script; the response still carries its error status separately.
///
/// Image fidelity is preserved here. Model-specific image detail selection
/// still belongs to the caller and does not affect the truncation budget.
///
/// This is the same output Core would render as long as the session collector
/// has retained the whole observation. Collector omissions stay outside this
/// budget: Core's warning describes the retained text alone. The shared
/// formatter moves collector boundaries along with their source content.
/// If losses are too scattered for individual notices, report their totals once.
pub(crate) fn render_output(
    output: CollectedOutput,
    max_output_tokens: Option<usize>,
) -> Vec<OutputItem> {
    let CollectedOutput { items, gaps } = output;
    let mut boundaries: Vec<_> = gaps.iter().map(|gap| gap.boundary).collect();
    let mut content = Vec::with_capacity(items.len());
    for item in items {
        match item {
            OutputItem::InputText { text } => {
                content.push(FunctionCallOutputContentItem::InputText { text });
            }
            OutputItem::InputImage { image_url, detail } => {
                content.push(FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    detail: detail.map(|detail| match detail {
                        ImageDetail::Auto => ProtocolImageDetail::Auto,
                        ImageDetail::Low => ProtocolImageDetail::Low,
                        ImageDetail::High => ProtocolImageDetail::High,
                        ImageDetail::Original => ProtocolImageDetail::Original,
                    }),
                });
            }
            OutputItem::InputAudio { audio_url } => {
                content.push(FunctionCallOutputContentItem::InputAudio { audio_url });
            }
        }
    }
    let max_tokens = max_output_tokens.unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL);
    // Core budgets mixed output in order. Keep that policy even if the
    // collector removed every image or audio item in the observation.
    let formatted = if gaps
        .iter()
        .any(|gap| gap.omitted.image_items > 0 || gap.omitted.audio_items > 0)
    {
        mixed_output_at_token_boundaries(
            content,
            max_tokens,
            estimate_audio_token_count_without_cache,
            &mut boundaries,
        )
    } else {
        formatted_output_at_token_boundaries(
            content,
            max_tokens,
            estimate_audio_token_count_without_cache,
            &mut boundaries,
        )
    };
    insert_loss_notices(formatted, boundaries, gaps)
}

fn insert_loss_notices(
    mut formatted: Vec<FunctionCallOutputContentItem>,
    boundaries: Vec<OutputBoundary>,
    gaps: Vec<OutputGap>,
) -> Vec<OutputItem> {
    if gaps.is_empty() {
        return formatted.into_iter().map(into_code_mode_output).collect();
    }
    // One extra slot detects when an observation-wide summary is needed.
    let mut notices = [((0, 0), OutputOmission::default()); MAX_POSITIONED_NOTICES + 1];
    let mut notice_count = 0;
    let mut total = OutputOmission::default();
    let mut summary_position = (formatted.len(), 0);
    for (gap, boundary) in gaps.into_iter().zip(boundaries) {
        total.combine(gap.omitted);
        let OutputBoundary {
            item_index,
            text_offset,
        } = boundary;
        // The end of a text item and the start of its successor are one position.
        let position = if text_offset != 0
            && let Some(FunctionCallOutputContentItem::InputText { text }) =
                formatted.get(item_index)
            && text_offset == text.len()
        {
            (item_index + 1, 0)
        } else {
            (item_index, text_offset)
        };
        // The collector can split one original text item. Even an observation-wide
        // summary must keep its two fragments from reading as contiguous text.
        if gap.boundary.text_offset != 0 {
            summary_position = position;
        }
        if notice_count == notices.len() {
            continue;
        }
        if let Some((last, omitted)) = notices[..notice_count].last_mut()
            && *last == position
        {
            omitted.combine(gap.omitted);
        } else {
            notices[notice_count] = (position, gap.omitted);
            notice_count += 1;
        }
    }
    let scope = if notice_count == notices.len() {
        notices[0] = (summary_position, total);
        notice_count = 1;
        "across this observation, "
    } else {
        ""
    };
    // Drain from the end so earlier positions stay valid as text is split.
    let mut reversed = Vec::with_capacity(formatted.len() + 2 * notice_count);
    for &((index, offset), omission) in notices[..notice_count].iter().rev() {
        let after = index + usize::from(offset != 0);
        reversed.extend(formatted.drain(after..).rev().map(into_code_mode_output));
        if offset != 0 {
            let FunctionCallOutputContentItem::InputText { text } = &mut formatted[index] else {
                unreachable!("only text has an inner byte position");
            };
            if offset < text.len() {
                reversed.push(OutputItem::InputText {
                    text: text.split_off(offset),
                });
            }
        }
        reversed.push(OutputItem::InputText {
            text: format!(
                "[Code Mode session output limit reached: {scope}omitted {} text bytes; discarded {} text segments, {} images, {} audio items]",
                omission.text_bytes,
                omission.text_items,
                omission.image_items,
                omission.audio_items
            ),
        });
    }
    reversed.extend(formatted.into_iter().rev().map(into_code_mode_output));
    reversed.reverse();
    reversed
}

fn into_code_mode_output(item: FunctionCallOutputContentItem) -> OutputItem {
    match item {
        FunctionCallOutputContentItem::InputText { text } => OutputItem::InputText { text },
        FunctionCallOutputContentItem::InputImage {
            image: ImageReference::Inline { image_url },
            detail,
        } => OutputItem::InputImage {
            image_url,
            detail: detail.map(|detail| match detail {
                ProtocolImageDetail::Auto => ImageDetail::Auto,
                ProtocolImageDetail::Low => ImageDetail::Low,
                ProtocolImageDetail::High => ImageDetail::High,
                ProtocolImageDetail::Original => ImageDetail::Original,
            }),
        },
        FunctionCallOutputContentItem::InputAudio { audio_url } => {
            OutputItem::InputAudio { audio_url }
        }
        FunctionCallOutputContentItem::EncryptedContent { .. }
        | FunctionCallOutputContentItem::InputImage {
            image: ImageReference::File { .. },
            ..
        } => {
            unreachable!("output truncation must not introduce new content types")
        }
    }
}

#[cfg(test)]
#[path = "output_render_tests.rs"]
mod tests;
