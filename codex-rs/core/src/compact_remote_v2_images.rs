use crate::context_manager::estimate_image_bytes;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::is_image_close_tag_text;
use codex_protocol::models::is_image_open_tag_text;
use codex_protocol::models::is_local_image_open_tag_text;
use codex_protocol::protocol::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::approx_tokens_from_byte_count_i64;
use codex_utils_output_truncation::truncate_text;

pub(super) fn content_item_token_count(item: &ContentItem) -> usize {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
            approx_token_count(text)
        }
        ContentItem::InputImage { image_url, detail } => usize::try_from(
            approx_tokens_from_byte_count_i64(estimate_image_bytes(image_url, *detail)),
        )
        .unwrap_or(usize::MAX),
        ContentItem::InputAudio { .. } => 0,
    }
}

/// Retain later parts of an image-containing boundary message, keeping each
/// image and its adjacent harness labels atomic. Text keeps its existing middle
/// truncation policy; audio remains uncharged and is preserved as before.
pub(super) fn truncate_message_to_token_budget(
    mut envelope: ResponseItemEnvelope,
    max_tokens: usize,
) -> Option<ResponseItemEnvelope> {
    let ResponseItem::Message { content, .. } = &mut envelope.item else {
        return None;
    };
    let mut content = std::mem::take(content);
    let mut remaining = max_tokens;
    let mut retained = Vec::with_capacity(content.len());
    while !content.is_empty() {
        let last = content.len() - 1;
        let image_index = match &content[last] {
            ContentItem::InputImage { .. } => Some(last),
            ContentItem::InputText { text }
                if is_image_close_tag_text(text)
                    && last > 0
                    && matches!(&content[last - 1], ContentItem::InputImage { .. }) =>
            {
                Some(last - 1)
            }
            _ => None,
        };
        if let Some(image_index) = image_index {
            let has_open_tag = image_index > 0
                && matches!(
                    &content[image_index - 1],
                    ContentItem::InputText { text }
                        if is_local_image_open_tag_text(text) || is_image_open_tag_text(text)
                );
            let start = image_index - usize::from(has_open_tag);
            let token_count = content[start..]
                .iter()
                .map(content_item_token_count)
                .sum::<usize>();
            let fits = token_count <= remaining;
            remaining = if fits { remaining - token_count } else { 0 };
            for item in content.drain(start..).rev() {
                if fits {
                    retained.push(item);
                }
            }
            continue;
        }
        let mut item = content.pop()?;
        match &mut item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                if remaining == 0 {
                    continue;
                }
                let token_count = approx_token_count(text);
                if token_count <= remaining {
                    remaining -= token_count;
                } else {
                    *text = truncate_text(text, TruncationPolicy::Tokens(remaining));
                    remaining = 0;
                }
                if !text.is_empty() {
                    retained.push(item);
                }
            }
            ContentItem::InputAudio { .. } => retained.push(item),
            ContentItem::InputImage { .. } => unreachable!("images are handled atomically above"),
        }
    }
    if retained.is_empty() {
        return None;
    }
    retained.reverse();
    let ResponseItem::Message { content, .. } = &mut envelope.item else {
        unreachable!("message content was already extracted above");
    };
    *content = retained;
    Some(envelope)
}
