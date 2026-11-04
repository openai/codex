//! Counts retained media tokens and truncates each media part together with its labels.

use super::RetainedImageBudget;
use crate::context_manager::estimate_image_reference_bytes;
use crate::session::FileAudioDurations;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::set_annotated_content;
use codex_context_fragments::to_annotated_content;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::is_closing_media_label;
use codex_protocol::models::is_opening_media_label;
use codex_protocol::protocol::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::approx_tokens_from_byte_count_i64;
use codex_utils_output_truncation::truncate_text;

pub(super) fn content_item_token_count(
    item: &ContentItem,
    image_budget: RetainedImageBudget,
    audio_durations: &FileAudioDurations,
) -> usize {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text, .. } => {
            approx_token_count(text)
        }
        ContentItem::InputImage { .. } if image_budget == RetainedImageBudget::Disabled => 0,
        ContentItem::InputImage { image, detail } => usize::try_from(
            approx_tokens_from_byte_count_i64(estimate_image_reference_bytes(image, *detail)),
        )
        .unwrap_or(usize::MAX),
        ContentItem::InputAudio {
            audio: AudioReference::File { file_id },
        } => audio_durations.tokens(file_id),
        ContentItem::InputAudio {
            audio: AudioReference::Inline { .. },
        } => 0,
    }
}

/// Find the trailing image or audio part together with its adjacent harness labels.
fn trailing_media_start(content: &[AnnotatedContent]) -> Option<usize> {
    let last = content.len().checked_sub(1)?;
    let media = match content[last].content() {
        ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => last,
        ContentItem::InputText { text }
            if last > 0 && is_closing_media_label(text, content[last - 1].content()) =>
        {
            last - 1
        }
        _ => return None,
    };
    let has_open_tag = media > 0
        && matches!(content[media - 1].content(), ContentItem::InputText { text } if is_opening_media_label(text, content[media].content()));
    Some(media - usize::from(has_open_tag))
}

/// Keep the newest suffix. A media part and its labels must fit together even
/// when the media itself has zero cost.
pub(super) fn truncate_message_to_token_budget(
    mut envelope: ResponseItemEnvelope,
    max_tokens: usize,
    image_budget: RetainedImageBudget,
    audio_durations: &FileAudioDurations,
) -> Option<ResponseItemEnvelope> {
    let mut content = to_annotated_content(&mut envelope.item)?;
    let mut remaining = max_tokens;
    let mut retained = Vec::with_capacity(content.len());
    while remaining > 0 && !content.is_empty() {
        if let Some(start) = trailing_media_start(&content) {
            let tokens = content[start..]
                .iter()
                .map(|part| content_item_token_count(part.content(), image_budget, audio_durations))
                .fold(0, usize::saturating_add);
            if tokens > remaining {
                break;
            }
            remaining -= tokens;
            retained.extend(content.drain(start..).rev());
            continue;
        }
        let mut item = content.pop()?;
        let (ContentItem::InputText { text } | ContentItem::OutputText { text, .. }) =
            item.content_mut()
        else {
            unreachable!("media parts are handled above");
        };
        let tokens = approx_token_count(text);
        if tokens > remaining {
            *text = truncate_text(text, TruncationPolicy::Tokens(remaining));
        }
        remaining = remaining.saturating_sub(tokens);
        if !text.is_empty() {
            retained.push(item);
        }
    }
    if retained.is_empty() {
        return None;
    }
    retained.reverse();
    set_annotated_content(&mut envelope.item, retained)?;
    if let Some(metadata) = &mut envelope.metadata {
        metadata.mark_retained_sources_incomplete();
    }
    Some(envelope)
}
