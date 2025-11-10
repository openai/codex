use codex_protocol::models::FunctionCallOutputContentItem;
use codex_utils_string::take_bytes_at_char_boundary;
use codex_utils_string::take_last_bytes_at_char_boundary;

// Model-formatting limits: clients get full streams; only content sent to the model is truncated.
pub(crate) const MODEL_FORMAT_MAX_BYTES: usize = 10 * 1024; // 10 KiB
pub(crate) const MODEL_FORMAT_MAX_LINES: usize = 256; // lines
pub(crate) const MODEL_FORMAT_HEAD_LINES: usize = MODEL_FORMAT_MAX_LINES / 2;
pub(crate) const MODEL_FORMAT_TAIL_LINES: usize = MODEL_FORMAT_MAX_LINES - MODEL_FORMAT_HEAD_LINES; // 128
pub(crate) const MODEL_FORMAT_HEAD_BYTES: usize = MODEL_FORMAT_MAX_BYTES / 2;

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

pub(crate) fn format_output_for_model_body(content: &str) -> String {
    // Head+tail truncation for the model: show the beginning and end with an elision.
    // Clients still receive full streams; only this formatted summary is capped.
    let total_lines = content.lines().count();
    if content.len() <= MODEL_FORMAT_MAX_BYTES && total_lines <= MODEL_FORMAT_MAX_LINES {
        return content.to_string();
    }
    let output = truncate_formatted_exec_output(content, total_lines);
    format!("Total output lines: {total_lines}\n\n{output}")
}

fn truncate_formatted_exec_output(content: &str, total_lines: usize) -> String {
    let truncated_by_bytes = content.len() > MODEL_FORMAT_MAX_BYTES;

    // When byte truncation is needed, use byte-based head/tail slicing directly
    // to ensure both head and tail are preserved (important for error messages at the end)
    if truncated_by_bytes {
        let marker =
            format!("\n[... output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes ...]\n\n");
        let marker_len = marker.len();

        let head_budget =
            MODEL_FORMAT_HEAD_BYTES.min(MODEL_FORMAT_MAX_BYTES.saturating_sub(marker_len));
        let head_part = take_bytes_at_char_boundary(content, head_budget);

        let mut result = String::with_capacity(MODEL_FORMAT_MAX_BYTES.min(content.len()));
        result.push_str(head_part);
        result.push_str(&marker);

        let remaining = MODEL_FORMAT_MAX_BYTES.saturating_sub(result.len());
        if remaining > 0 {
            let tail_part = take_last_bytes_at_char_boundary(content, remaining);
            result.push_str(tail_part);
        }

        return result;
    }

    // Line-based truncation for cases where we exceed line limits but not byte limits
    let segments: Vec<&str> = content.split_inclusive('\n').collect();
    let head_take = MODEL_FORMAT_HEAD_LINES.min(segments.len());
    let tail_take = MODEL_FORMAT_TAIL_LINES.min(segments.len().saturating_sub(head_take));
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

    // this is a bit wrong. We are counting metadata lines and not just shell output lines.
    let marker = if omitted > 0 {
        Some(format!(
            "\n[... omitted {omitted} of {total_lines} lines ...]\n\n"
        ))
    } else {
        None
    };

    let marker_len = marker.as_ref().map_or(0, String::len);
    let base_head_budget = MODEL_FORMAT_HEAD_BYTES.min(MODEL_FORMAT_MAX_BYTES);
    let head_budget = base_head_budget.min(MODEL_FORMAT_MAX_BYTES.saturating_sub(marker_len));
    let head_part = take_bytes_at_char_boundary(head_slice, head_budget);
    let mut result = String::with_capacity(MODEL_FORMAT_MAX_BYTES.min(content.len()));

    result.push_str(head_part);
    if let Some(marker_text) = marker.as_ref() {
        result.push_str(marker_text);
    }

    let remaining = MODEL_FORMAT_MAX_BYTES.saturating_sub(result.len());
    if remaining == 0 {
        return result;
    }

    let tail_part = take_last_bytes_at_char_boundary(tail_slice, remaining);
    result.push_str(tail_part);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_truncation_preserves_tail() {
        // Simulate cargo-like output: few lines but very long lines
        // 5 lines of ~5000 bytes each = 25KB total, exceeds 10KB limit
        let line1 = format!("Compiling project v1.0.0{}\n", "x".repeat(4970));
        let line2 = format!("Building dependencies{}\n", "y".repeat(4975));
        let line3 = format!("Running tests{}\n", "z".repeat(4985));
        let line4 = format!("Warning: unused import{}\n", "w".repeat(4973));
        let line5 = "error: compilation failed\n";

        let content = format!("{line1}{line2}{line3}{line4}{line5}");
        assert!(content.len() > MODEL_FORMAT_MAX_BYTES);

        let total_lines = content.lines().count();
        let result = truncate_formatted_exec_output(&content, total_lines);

        // Verify the result is within byte limit
        assert!(result.len() <= MODEL_FORMAT_MAX_BYTES);

        // Verify the truncation marker is present
        assert!(result.contains("output truncated to fit"));

        // CRITICAL: Verify tail is preserved - should contain error message
        assert!(
            result.contains("error: compilation failed"),
            "Tail content (error message) should be preserved but was not found in result"
        );

        // Verify head is also preserved
        assert!(result.contains("Compiling project"));
    }

    #[test]
    fn test_line_truncation_still_works() {
        // Many short lines exceeding line limit but not byte limit
        let mut content = String::new();
        for i in 0..300 {
            content.push_str(&format!("Line {i}\n"));
        }

        let total_lines = content.lines().count();
        let result = truncate_formatted_exec_output(&content, total_lines);

        // Should use line-based truncation
        assert!(result.contains("omitted"));
        assert!(result.contains("of 300 lines"));

        // Should preserve head and tail lines
        assert!(result.contains("Line 0"));
        assert!(result.contains("Line 299"));
    }

    #[test]
    fn test_no_truncation_needed() {
        let content = "Short output\nJust a few lines\nNo truncation needed\n";
        let total_lines = content.lines().count();
        let result = truncate_formatted_exec_output(content, total_lines);

        // Should return content as-is
        assert_eq!(result, content);
        assert!(!result.contains("truncated"));
        assert!(!result.contains("omitted"));
    }
}
