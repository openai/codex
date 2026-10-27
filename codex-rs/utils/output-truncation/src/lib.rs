//! Shared byte/token truncation for tool and exec output.

use std::borrow::Cow;

use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_utils_string::TextTruncation;
pub use codex_utils_string::approx_bytes_for_tokens;
pub use codex_utils_string::approx_token_count;
pub use codex_utils_string::approx_tokens_from_byte_count;
use codex_utils_string::plan_token_truncation;
use codex_utils_string::truncate_middle_chars;

pub use codex_protocol::protocol::TruncationPolicy;

/// Adds the existing 20% allowance for serialization and headers.
/// Saved history budgets already include this allowance.
pub fn with_serialization_allowance(policy: TruncationPolicy) -> TruncationPolicy {
    policy * 1.2
}

pub fn formatted_truncate_text(content: &str, policy: TruncationPolicy) -> String {
    if content.len() <= policy.byte_budget() {
        return content.to_string();
    }

    format_truncated_text(content, policy, &mut [])
}

fn format_truncated_text(
    content: &str,
    policy: TruncationPolicy,
    boundaries: &mut [OutputBoundary],
) -> String {
    let original_token_count = approx_token_count(content);
    let total_lines = content.lines().count();
    let header = format!(
        "Warning: truncated output (original token count: {original_token_count})\nTotal output lines: {total_lines}\n\n"
    );
    let (snippet, edit) = truncate_text_with_plan(content, policy);
    if let Some(edit) = edit {
        for boundary in boundaries {
            boundary.text_offset = rendered_offset(&edit, boundary.text_offset, header.len());
        }
    }
    header + &snippet
}

pub fn truncate_text(content: &str, policy: TruncationPolicy) -> String {
    truncate_text_with_plan(content, policy).0
}

// Only token-budget callers supply boundaries that need remapping.
fn truncate_text_with_plan(
    content: &str,
    policy: TruncationPolicy,
) -> (String, Option<TextTruncation>) {
    match policy {
        TruncationPolicy::Bytes(bytes) => (truncate_middle_chars(content, bytes), None),
        TruncationPolicy::Tokens(tokens) => match plan_token_truncation(content, tokens) {
            Some(edit) => (edit.apply(content), Some(edit)),
            None => (content.to_owned(), None),
        },
    }
}

/// Replaces an oversized serialized MCP result with a bounded text preview while preserving its
/// error status and supported bounded metadata.
pub fn truncate_mcp_tool_result(
    result: &CallToolResult,
    max_bytes: usize,
) -> Cow<'_, CallToolResult> {
    let Ok(serialized) = serde_json::to_string(result) else {
        return Cow::Borrowed(result);
    };
    if serialized.len() <= max_bytes {
        return Cow::Borrowed(result);
    }

    let meta = None;

    let mut preview_budget = max_bytes;
    loop {
        let preview = if preview_budget == 0 {
            String::new()
        } else {
            truncate_text(&serialized, TruncationPolicy::Bytes(preview_budget))
        };
        let truncated = CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": preview,
            })],
            structured_content: None,
            is_error: result.is_error,
            meta: meta.clone(),
        };
        let Ok(truncated_serialized) = serde_json::to_string(&truncated) else {
            return Cow::Borrowed(result);
        };
        let truncated_len = truncated_serialized.len();
        if truncated_len <= max_bytes || preview_budget == 0 {
            return Cow::Owned(truncated);
        }

        // The preview is serialized as a JSON string, so escaping can make it exceed its raw byte
        // budget. Scale the next budget by the observed size of the complete replacement.
        preview_budget = (preview_budget.saturating_mul(max_bytes) / truncated_len)
            .min(preview_budget.saturating_sub(1));
    }
}

/// Applies the existing byte/token budget without changing success metadata or media ordering.
pub fn truncate_function_output_payload(
    output: &mut FunctionCallOutputPayload,
    policy: TruncationPolicy,
    estimate_audio_token_count: impl Fn(&str) -> usize,
) {
    match &mut output.body {
        FunctionCallOutputBody::Text(text) => *text = truncate_text(text, policy),
        FunctionCallOutputBody::ContentItems(items) => {
            *items = truncate_function_output_items_with_policy(
                items,
                policy,
                estimate_audio_token_count,
            );
        }
    }
}

/// An uncharged boundary in the original logical output. Token-boundary
/// formatters update the item index and UTF-8 text offset to the rendered
/// position, including when content at that position was omitted.
///
/// At entry, item indices are clamped to the end of the input, text offsets
/// round down to an in-bounds UTF-8 boundary, and offsets on non-text items
/// are zeroed. Positions preceding an earlier boundary move to that position,
/// preserving caller order. On return, boundaries refer to the rendered items;
/// an end boundary precedes any generated omission notices.
pub struct OutputBoundary {
    pub item_index: usize,
    pub text_offset: usize,
}

/// Format text-only output as a whole observation, including the truncation
/// warning. Charge mixed text and audio against the budget in order while
/// preserving images.
pub fn formatted_truncate_function_output_items_with_policy<'a>(
    items: impl Into<Cow<'a, [FunctionCallOutputContentItem]>>,
    policy: TruncationPolicy,
    estimate_audio_token_count: impl Fn(&str) -> usize,
) -> Vec<FunctionCallOutputContentItem> {
    formatted_output(items.into(), policy, estimate_audio_token_count, &mut [])
}

/// As above, with zero-width boundaries for callers that have already dropped
/// output. Input positions are normalized as documented on [`OutputBoundary`].
pub fn formatted_output_at_token_boundaries<'a>(
    items: impl Into<Cow<'a, [FunctionCallOutputContentItem]>>,
    max_tokens: usize,
    estimate_audio_token_count: impl Fn(&str) -> usize,
    boundaries: &mut [OutputBoundary],
) -> Vec<FunctionCallOutputContentItem> {
    let items = items.into();
    normalize_boundaries(&items, boundaries);
    formatted_output(
        items,
        TruncationPolicy::Tokens(max_tokens),
        estimate_audio_token_count,
        boundaries,
    )
}

fn formatted_output(
    items: Cow<'_, [FunctionCallOutputContentItem]>,
    policy: TruncationPolicy,
    estimate_audio_token_count: impl Fn(&str) -> usize,
    boundaries: &mut [OutputBoundary],
) -> Vec<FunctionCallOutputContentItem> {
    let mut combined_len = 0;
    for item in items.iter() {
        let FunctionCallOutputContentItem::InputText { text } = item else {
            return truncate_output_items(items, policy, estimate_audio_token_count, boundaries);
        };
        combined_len += usize::from(combined_len != 0) + text.len();
    }
    if combined_len <= policy.byte_budget() {
        return items.into_owned();
    }

    let mut combined = String::with_capacity(combined_len);
    let mut next_boundary = 0;
    for (index, item) in items.iter().enumerate() {
        if let FunctionCallOutputContentItem::InputText { text } = item {
            if !combined.is_empty() {
                combined.push('\n');
            }
            while let Some(boundary) = boundaries.get_mut(next_boundary)
                && boundary.item_index == index
            {
                boundary.item_index = 0;
                boundary.text_offset += combined.len();
                next_boundary += 1;
            }
            combined.push_str(text);
        }
    }
    // A boundary after the last item belongs at the end of the merged text.
    for boundary in &mut boundaries[next_boundary..] {
        boundary.item_index = 0;
        boundary.text_offset = combined.len();
    }
    vec![FunctionCallOutputContentItem::InputText {
        text: format_truncated_text(&combined, policy, boundaries),
    }]
}

pub fn truncate_function_output_items_with_policy(
    items: &[FunctionCallOutputContentItem],
    policy: TruncationPolicy,
    estimate_audio_token_count: impl Fn(&str) -> usize,
) -> Vec<FunctionCallOutputContentItem> {
    truncate_output_items(
        Cow::Borrowed(items),
        policy,
        estimate_audio_token_count,
        &mut [],
    )
}

/// Budget in input order even if a caller previously dropped every media item.
/// Input positions are normalized as documented on [`OutputBoundary`].
pub fn mixed_output_at_token_boundaries<'a>(
    items: impl Into<Cow<'a, [FunctionCallOutputContentItem]>>,
    max_tokens: usize,
    estimate_audio_token_count: impl Fn(&str) -> usize,
    boundaries: &mut [OutputBoundary],
) -> Vec<FunctionCallOutputContentItem> {
    let items = items.into();
    normalize_boundaries(&items, boundaries);
    truncate_output_items(
        items,
        TruncationPolicy::Tokens(max_tokens),
        estimate_audio_token_count,
        boundaries,
    )
}

fn normalize_boundaries(
    items: &[FunctionCallOutputContentItem],
    boundaries: &mut [OutputBoundary],
) {
    let mut previous = (0, 0);
    for boundary in boundaries {
        let item_index = boundary.item_index.min(items.len());
        let text_offset = match items.get(item_index) {
            Some(FunctionCallOutputContentItem::InputText { text }) => {
                text.floor_char_boundary(boundary.text_offset.min(text.len()))
            }
            _ => 0,
        };
        previous = previous.max((item_index, text_offset));
        (boundary.item_index, boundary.text_offset) = previous;
    }
}

fn truncate_output_items(
    items: Cow<'_, [FunctionCallOutputContentItem]>,
    policy: TruncationPolicy,
    estimate_audio_token_count: impl Fn(&str) -> usize,
    boundaries: &mut [OutputBoundary],
) -> Vec<FunctionCallOutputContentItem> {
    match items {
        Cow::Borrowed(items) => truncate_output_iter(
            items.iter().map(Cow::Borrowed),
            policy,
            estimate_audio_token_count,
            boundaries,
        ),
        Cow::Owned(items) => truncate_output_iter(
            items.into_iter().map(Cow::Owned),
            policy,
            estimate_audio_token_count,
            boundaries,
        ),
    }
}

fn truncate_output_iter<'a>(
    items: impl ExactSizeIterator<Item = Cow<'a, FunctionCallOutputContentItem>>,
    policy: TruncationPolicy,
    estimate_audio_token_count: impl Fn(&str) -> usize,
    boundaries: &mut [OutputBoundary],
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining_budget = match policy {
        TruncationPolicy::Bytes(_) => policy.byte_budget(),
        TruncationPolicy::Tokens(_) => policy.token_budget(),
    };
    let mut omitted_text_items = 0usize;
    let mut omitted_audio_items = 0usize;
    let mut next_boundary = 0;

    for (index, item) in items.enumerate() {
        let item_out = out.len();
        let item_policy = match policy {
            TruncationPolicy::Bytes(_) => TruncationPolicy::Bytes(remaining_budget),
            TruncationPolicy::Tokens(_) => TruncationPolicy::Tokens(remaining_budget),
        };
        let mut text_edit = None;
        let keep = match item.as_ref() {
            // Empty text contributes no model content but still consumes an API array slot.
            FunctionCallOutputContentItem::InputText { text } if text.is_empty() => false,
            FunctionCallOutputContentItem::InputText { .. } if remaining_budget == 0 => {
                omitted_text_items += 1;
                false
            }
            FunctionCallOutputContentItem::InputText { text } => {
                let cost = match policy {
                    TruncationPolicy::Bytes(_) => text.len(),
                    TruncationPolicy::Tokens(_) => approx_token_count(text),
                };
                if cost <= remaining_budget {
                    remaining_budget = remaining_budget.saturating_sub(cost);
                    true
                } else {
                    let (snippet, edit) = truncate_text_with_plan(text, item_policy);
                    text_edit = edit;
                    if snippet.is_empty() {
                        omitted_text_items += 1;
                    } else {
                        out.push(FunctionCallOutputContentItem::InputText { text: snippet });
                    }
                    remaining_budget = 0;
                    false
                }
            }
            FunctionCallOutputContentItem::InputImage { .. }
            | FunctionCallOutputContentItem::EncryptedContent { .. } => true,
            FunctionCallOutputContentItem::InputAudio { audio_url } => {
                let token_cost = estimate_audio_token_count(audio_url);
                let cost = match policy {
                    TruncationPolicy::Bytes(_) => approx_bytes_for_tokens(token_cost),
                    TruncationPolicy::Tokens(_) => token_cost,
                };
                if cost <= remaining_budget {
                    remaining_budget = remaining_budget.saturating_sub(cost);
                    true
                } else {
                    omitted_audio_items += 1;
                    false
                }
            }
        };
        while let Some(boundary) = boundaries.get_mut(next_boundary)
            && boundary.item_index == index
        {
            boundary.item_index = item_out;
            if let Some(edit) = &text_edit {
                boundary.text_offset =
                    rendered_offset(edit, boundary.text_offset, /*header_bytes*/ 0);
            } else if !keep {
                boundary.text_offset = 0;
            }
            next_boundary += 1;
        }
        if keep {
            out.push(item.into_owned());
        }
    }
    // End boundaries precede generated omission counts: counts are not original
    // content and must never move a collector notice away from its source.
    for boundary in &mut boundaries[next_boundary..] {
        boundary.item_index = out.len();
        boundary.text_offset = 0;
    }

    if omitted_text_items > 0 {
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!("[omitted {omitted_text_items} text items ...]"),
        });
    }
    if omitted_audio_items > 0 {
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!("[omitted {omitted_audio_items} audio items ...]"),
        });
    }
    out
}

fn rendered_offset(edit: &TextTruncation, boundary: usize, header_bytes: usize) -> usize {
    if boundary == 0 {
        0
    } else if boundary <= edit.removed.start {
        header_bytes + boundary
    } else {
        header_bytes
            + edit.removed.start
            + edit.marker.len()
            + boundary.saturating_sub(edit.removed.end)
    }
}

pub fn approx_tokens_from_byte_count_i64(bytes: i64) -> i64 {
    if bytes <= 0 {
        return 0;
    }

    let bytes = usize::try_from(bytes).unwrap_or(usize::MAX);
    i64::try_from(approx_tokens_from_byte_count(bytes)).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod truncate_tests;
