use codex_protocol::models::FunctionCallOutputContentItem;
use codex_utils_string::take_bytes_at_char_boundary;
use codex_utils_string::take_last_bytes_at_char_boundary;

use crate::util::error_or_panic;

// Model-formatting limits: clients get full streams; only content sent to the model is truncated.
pub const MODEL_FORMAT_MAX_BYTES: usize = 10 * 1024; // 10 KiB
pub const MODEL_FORMAT_MAX_LINES: usize = 256; // lines

pub(crate) fn globally_truncate_function_output_items(
    items: &[FunctionCallOutputContentItem],
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining = MODEL_FORMAT_MAX_BYTES;
    let mut omitted_text_items = 0usize;

    for it in items {
        match it {
            FunctionCallOutputContentItem::InputText { text } => {
                if remaining == 0 {
                    omitted_text_items += 1;
                    continue;
                }

                let len = text.len();
                if len <= remaining {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                    remaining -= len;
                } else {
                    let slice = take_bytes_at_char_boundary(text, remaining);
                    if !slice.is_empty() {
                        out.push(FunctionCallOutputContentItem::InputText {
                            text: slice.to_string(),
                        });
                    }
                    remaining = 0;
                }
            }
            // todo(aibrahim): handle input images; resize
            FunctionCallOutputContentItem::InputImage { image_url } => {
                out.push(FunctionCallOutputContentItem::InputImage {
                    image_url: image_url.clone(),
                });
            }
        }
    }

    if omitted_text_items > 0 {
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!("[omitted {omitted_text_items} text items ...]"),
        });
    }

    out
}

pub(crate) fn format_output_for_model_body(
    content: &str,
    limit_bytes: usize,
    limit_lines: usize,
) -> String {
    // Head+tail truncation for the model: show the beginning and end with an elision.
    // Clients still receive full streams; only this formatted summary is capped.
    let total_lines = content.lines().count();
    if content.len() <= limit_bytes && total_lines <= limit_lines {
        return content.to_string();
    }
    let output = truncate_formatted_exec_output(content, total_lines, limit_bytes, limit_lines);
    format!("Total output lines: {total_lines}\n\n{output}")
}

fn truncate_formatted_exec_output(
    content: &str,
    total_lines: usize,
    limit_bytes: usize,
    limit_lines: usize,
) -> String {
    debug_panic_on_double_truncation(content);
    let head_lines: usize = limit_lines / 2;
    let tail_lines: usize = limit_lines - head_lines; // 128
    let head_bytes: usize = limit_bytes / 2;
    let segments: Vec<&str> = content.split_inclusive('\n').collect();
    let head_take = head_lines.min(segments.len());
    let tail_take = tail_lines.min(segments.len().saturating_sub(head_take));
    let omitted = segments.len().saturating_sub(head_take + tail_take);

    let head_slice_end: usize = segments
        .iter()
        .take(head_take)
        .map(|segment| segment.len())
        .sum();
    let tail_slice_start: usize = if tail_take == 0 {
        content.len()
    } else {
        content.len()
            - segments
                .iter()
                .rev()
                .take(tail_take)
                .map(|segment| segment.len())
                .sum::<usize>()
    };
    let head_slice = &content[..head_slice_end];
    let tail_slice = &content[tail_slice_start..];
    let truncated_by_bytes = content.len() > limit_bytes;
    // this is a bit wrong. We are counting metadata lines and not just shell output lines.
    let marker = if omitted > 0 {
        Some(format!(
            "\n[... omitted {omitted} of {total_lines} lines ...]\n\n"
        ))
    } else if truncated_by_bytes {
        Some(format!(
            "\n[... output truncated to fit {limit_bytes} bytes ...]\n\n"
        ))
    } else {
        None
    };

    let marker_len = marker.as_ref().map_or(0, String::len);
    let base_head_budget = head_bytes.min(limit_bytes);
    let head_budget = base_head_budget.min(limit_bytes.saturating_sub(marker_len));
    let head_part = take_bytes_at_char_boundary(head_slice, head_budget);
    let mut result = String::with_capacity(limit_bytes.min(content.len()));

    result.push_str(head_part);
    if let Some(marker_text) = marker.as_ref() {
        result.push_str(marker_text);
    }

    let remaining = limit_bytes.saturating_sub(result.len());
    if remaining == 0 {
        return result;
    }

    let tail_part = take_last_bytes_at_char_boundary(tail_slice, remaining);
    result.push_str(tail_part);

    result
}

fn debug_panic_on_double_truncation(content: &str) {
    if content.contains("Total output lines:") && content.contains("omitted") {
        error_or_panic(format!(
            "FunctionCallOutput content was already truncated before ContextManager::record_items; this would cause double truncation {content}"
        ));
    }
}
