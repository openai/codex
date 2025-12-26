use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

pub(crate) fn capitalize_first(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        Some(first) => {
            let mut capitalized = first.to_uppercase().collect::<String>();
            capitalized.push_str(chars.as_str());
            capitalized
        }
        None => String::new(),
    }
}

/// Truncate a tool result to fit within the given height and width. If the text is valid JSON, we format it in a compact way before truncating.
/// This is a best-effort approach that may not work perfectly for text where 1 grapheme is rendered as multiple terminal cells.
pub(crate) fn format_and_truncate_tool_result(
    text: &str,
    max_lines: usize,
    line_width: usize,
) -> String {
    // Work out the maximum number of graphemes we can display for a result.
    // It's not guaranteed that 1 grapheme = 1 cell, so we subtract 1 per line as a fudge factor.
    // It also won't handle future terminal resizes properly, but it's an OK approximation for now.
    let max_graphemes = (max_lines * line_width).saturating_sub(max_lines);

    if let Some(formatted_json) = format_json_compact(text) {
        truncate_text(&formatted_json, max_graphemes)
    } else {
        truncate_text(text, max_graphemes)
    }
}

/// Format JSON text in a compact single-line format with spaces for better Ratatui wrapping.
/// Ex: `{"a":"b",c:["d","e"]}` -> `{"a": "b", "c": ["d", "e"]}`
/// Returns the formatted JSON string if the input is valid JSON, otherwise returns None.
/// This is a little complicated, but it's necessary because Ratatui's wrapping is *very* limited
/// and can only do line breaks at whitespace. If we use the default serde_json format, we get lines
/// without spaces that Ratatui can't wrap nicely. If we use the serde_json pretty format as-is,
/// it's much too sparse and uses too many terminal rows.
/// Relevant issue: https://github.com/ratatui/ratatui/issues/293
pub(crate) fn format_json_compact(text: &str) -> Option<String> {
    let json = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let json_pretty = serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string());

    // Convert multi-line pretty JSON to compact single-line format by removing newlines and excess whitespace
    let mut result = String::new();
    let mut chars = json_pretty.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    // Iterate over the characters in the JSON string, adding spaces after : and , but only when not in a string
    while let Some(ch) = chars.next() {
        match ch {
            '"' if !escape_next => {
                in_string = !in_string;
                result.push(ch);
            }
            '\\' if in_string => {
                escape_next = !escape_next;
                result.push(ch);
            }
            '\n' | '\r' if !in_string => {
                // Skip newlines when not in a string
            }
            ' ' | '\t' if !in_string => {
                // Add a space after : and , but only when not in a string
                if let Some(&next_ch) = chars.peek()
                    && let Some(last_ch) = result.chars().last()
                    && (last_ch == ':' || last_ch == ',')
                    && !matches!(next_ch, '}' | ']')
                {
                    result.push(' ');
                }
            }
            _ => {
                if escape_next && in_string {
                    escape_next = false;
                }
                result.push(ch);
            }
        }
    }

    Some(result)
}

/// Truncate `text` to `max_graphemes` graphemes. Using graphemes to avoid accidentally truncating in the middle of a multi-codepoint character.
pub(crate) fn truncate_text(text: &str, max_graphemes: usize) -> String {
    let mut graphemes = text.grapheme_indices(true);

    // Check if there's a grapheme at position max_graphemes (meaning there are more than max_graphemes total)
    if let Some((byte_index, _)) = graphemes.nth(max_graphemes) {
        // There are more than max_graphemes, so we need to truncate
        if max_graphemes >= 3 {
            // Truncate to max_graphemes - 3 and add "..." to stay within limit
            let mut truncate_graphemes = text.grapheme_indices(true);
            if let Some((truncate_byte_index, _)) = truncate_graphemes.nth(max_graphemes - 3) {
                let truncated = &text[..truncate_byte_index];
                format!("{truncated}...")
            } else {
                text.to_string()
            }
        } else {
            // max_graphemes < 3, so just return first max_graphemes without "..."
            let truncated = &text[..byte_index];
            truncated.to_string()
        }
    } else {
        // There are max_graphemes or fewer graphemes, return original text
        text.to_string()
    }
}

/// Truncate a path-like string to the given display width, keeping leading and trailing segments
/// where possible and inserting a single Unicode ellipsis between them. If an individual segment
/// cannot fit, it is front-truncated with an ellipsis.
pub(crate) fn center_truncate_path(path: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(path) <= max_width {
        return path.to_string();
    }

    let sep = std::path::MAIN_SEPARATOR;
    let has_leading_sep = path.starts_with(sep);
    let has_trailing_sep = path.ends_with(sep);
    let mut raw_segments: Vec<&str> = path.split(sep).collect();
    if has_leading_sep && !raw_segments.is_empty() && raw_segments[0].is_empty() {
        raw_segments.remove(0);
    }
    if has_trailing_sep
        && !raw_segments.is_empty()
        && raw_segments.last().is_some_and(|last| last.is_empty())
    {
        raw_segments.pop();
    }

    if raw_segments.is_empty() {
        if has_leading_sep {
            let root = sep.to_string();
            if UnicodeWidthStr::width(root.as_str()) <= max_width {
                return root;
            }
        }
        return "…".to_string();
    }

    struct Segment<'a> {
        original: &'a str,
        text: String,
        truncatable: bool,
        is_suffix: bool,
    }

    let assemble = |leading: bool, segments: &[Segment<'_>]| -> String {
        let mut result = String::new();
        if leading {
            result.push(sep);
        }
        for segment in segments {
            if !result.is_empty() && !result.ends_with(sep) {
                result.push(sep);
            }
            result.push_str(segment.text.as_str());
        }
        result
    };

    let front_truncate = |original: &str, allowed_width: usize| -> String {
        if allowed_width == 0 {
            return String::new();
        }
        if UnicodeWidthStr::width(original) <= allowed_width {
            return original.to_string();
        }
        if allowed_width == 1 {
            return "…".to_string();
        }

        let mut kept: Vec<char> = Vec::new();
        let mut used_width = 1; // reserve space for leading ellipsis
        for ch in original.chars().rev() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if used_width + ch_width > allowed_width {
                break;
            }
            used_width += ch_width;
            kept.push(ch);
        }
        kept.reverse();
        let mut truncated = String::from("…");
        for ch in kept {
            truncated.push(ch);
        }
        truncated
    };

    let mut combos: Vec<(usize, usize)> = Vec::new();
    let segment_count = raw_segments.len();
    for left in 1..=segment_count {
        let min_right = if left == segment_count { 0 } else { 1 };
        for right in min_right..=(segment_count - left) {
            combos.push((left, right));
        }
    }
    let desired_suffix = if segment_count > 1 {
        std::cmp::min(2, segment_count - 1)
    } else {
        0
    };
    let mut prioritized: Vec<(usize, usize)> = Vec::new();
    let mut fallback: Vec<(usize, usize)> = Vec::new();
    for combo in combos {
        if combo.1 >= desired_suffix {
            prioritized.push(combo);
        } else {
            fallback.push(combo);
        }
    }
    let sort_combos = |items: &mut Vec<(usize, usize)>| {
        items.sort_by(|(left_a, right_a), (left_b, right_b)| {
            left_b
                .cmp(left_a)
                .then_with(|| right_b.cmp(right_a))
                .then_with(|| (left_b + right_b).cmp(&(left_a + right_a)))
        });
    };
    sort_combos(&mut prioritized);
    sort_combos(&mut fallback);

    let fit_segments =
        |segments: &mut Vec<Segment<'_>>, allow_front_truncate: bool| -> Option<String> {
            loop {
                let candidate = assemble(has_leading_sep, segments);
                let width = UnicodeWidthStr::width(candidate.as_str());
                if width <= max_width {
                    return Some(candidate);
                }

                if !allow_front_truncate {
                    return None;
                }

                let mut indices: Vec<usize> = Vec::new();
                for (idx, seg) in segments.iter().enumerate().rev() {
                    if seg.truncatable && seg.is_suffix {
                        indices.push(idx);
                    }
                }
                for (idx, seg) in segments.iter().enumerate().rev() {
                    if seg.truncatable && !seg.is_suffix {
                        indices.push(idx);
                    }
                }

                if indices.is_empty() {
                    return None;
                }

                let mut changed = false;
                for idx in indices {
                    let original_width = UnicodeWidthStr::width(segments[idx].original);
                    if original_width <= max_width && segment_count > 2 {
                        continue;
                    }
                    let seg_width = UnicodeWidthStr::width(segments[idx].text.as_str());
                    let other_width = width.saturating_sub(seg_width);
                    let allowed_width = max_width.saturating_sub(other_width).max(1);
                    let new_text = front_truncate(segments[idx].original, allowed_width);
                    if new_text != segments[idx].text {
                        segments[idx].text = new_text;
                        changed = true;
                        break;
                    }
                }

                if !changed {
                    return None;
                }
            }
        };

    for (left_count, right_count) in prioritized.into_iter().chain(fallback.into_iter()) {
        let mut segments: Vec<Segment<'_>> = raw_segments[..left_count]
            .iter()
            .map(|seg| Segment {
                original: seg,
                text: (*seg).to_string(),
                truncatable: true,
                is_suffix: false,
            })
            .collect();

        let need_ellipsis = left_count + right_count < segment_count;
        if need_ellipsis {
            segments.push(Segment {
                original: "…",
                text: "…".to_string(),
                truncatable: false,
                is_suffix: false,
            });
        }

        if right_count > 0 {
            segments.extend(
                raw_segments[segment_count - right_count..]
                    .iter()
                    .map(|seg| Segment {
                        original: seg,
                        text: (*seg).to_string(),
                        truncatable: true,
                        is_suffix: true,
                    }),
            );
        }

        let allow_front_truncate = need_ellipsis || segment_count <= 2;
        if let Some(candidate) = fit_segments(&mut segments, allow_front_truncate) {
            return candidate;
        }
    }

    front_truncate(path, max_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_truncate_text() {
        let text = "Hello, world!";
        let truncated = truncate_text(text, 8);
        assert_eq!(truncated, "Hello...");
    }

    #[test]
    fn test_truncate_empty_string() {
        let text = "";
        let truncated = truncate_text(text, 5);
        assert_eq!(truncated, "");
    }

    #[test]
    fn test_truncate_max_graphemes_zero() {
        let text = "Hello";
        let truncated = truncate_text(text, 0);
        assert_eq!(truncated, "");
    }

    #[test]
    fn test_truncate_max_graphemes_one() {
        let text = "Hello";
        let truncated = truncate_text(text, 1);
        assert_eq!(truncated, "H");
    }

    #[test]
    fn test_truncate_max_graphemes_two() {
        let text = "Hello";
        let truncated = truncate_text(text, 2);
        assert_eq!(truncated, "He");
    }

    #[test]
    fn test_truncate_max_graphemes_three_boundary() {
        let text = "Hello";
        let truncated = truncate_text(text, 3);
        assert_eq!(truncated, "...");
    }

    #[test]
    fn test_truncate_text_shorter_than_limit() {
        let text = "Hi";
        let truncated = truncate_text(text, 10);
        assert_eq!(truncated, "Hi");
    }

    #[test]
    fn test_truncate_text_exact_length() {
        let text = "Hello";
        let truncated = truncate_text(text, 5);
        assert_eq!(truncated, "Hello");
    }

    #[test]
    fn test_truncate_emoji() {
        let text = "👋🌍🚀✨💫";
        let truncated = truncate_text(text, 3);
        assert_eq!(truncated, "...");

        let truncated_longer = truncate_text(text, 4);
        assert_eq!(truncated_longer, "👋...");
    }

    #[test]
    fn test_truncate_unicode_combining_characters() {
        let text = "é́ñ̃"; // Characters with combining marks
        let truncated = truncate_text(text, 2);
        assert_eq!(truncated, "é́ñ̃");
    }

    #[test]
    fn test_truncate_very_long_text() {
        let text = "a".repeat(1000);
        let truncated = truncate_text(&text, 10);
        assert_eq!(truncated, "aaaaaaa...");
        assert_eq!(truncated.len(), 10); // 7 'a's + 3 dots
    }

    #[test]
    fn test_format_json_compact_simple_object() {
        let json = r#"{ "name": "John", "age": 30 }"#;
        let result = format_json_compact(json).unwrap();
        assert_eq!(result, r#"{"name": "John", "age": 30}"#);
    }

    #[test]
    fn test_format_json_compact_nested_object() {
        let json = r#"{ "user": { "name": "John", "details": { "age": 30, "city": "NYC" } } }"#;
        let result = format_json_compact(json).unwrap();
        assert_eq!(
            result,
            r#"{"user": {"name": "John", "details": {"age": 30, "city": "NYC"}}}"#
        );
    }

    #[test]
    fn test_center_truncate_doesnt_truncate_short_path() {
        let sep = std::path::MAIN_SEPARATOR;
        let path = format!("{sep}Users{sep}codex{sep}Public");
        let truncated = center_truncate_path(&path, 40);

        assert_eq!(truncated, path);
    }

    #[test]
    fn test_center_truncate_truncates_long_path() {
        let sep = std::path::MAIN_SEPARATOR;
        let path = format!("~{sep}hello{sep}the{sep}fox{sep}is{sep}very{sep}fast");
        let truncated = center_truncate_path(&path, 24);

        assert_eq!(
            truncated,
            format!("~{sep}hello{sep}the{sep}…{sep}very{sep}fast")
        );
    }

    #[test]
    fn test_center_truncate_truncates_long_windows_path() {
        let sep = std::path::MAIN_SEPARATOR;
        let path = format!(
            "C:{sep}Users{sep}codex{sep}Projects{sep}super{sep}long{sep}windows{sep}path{sep}file.txt"
        );
        let truncated = center_truncate_path(&path, 36);

        let expected = format!("C:{sep}Users{sep}codex{sep}…{sep}path{sep}file.txt");

        assert_eq!(truncated, expected);
    }

    #[test]
    fn test_center_truncate_handles_long_segment() {
        let sep = std::path::MAIN_SEPARATOR;
        let path = format!("~{sep}supercalifragilisticexpialidocious");
        let truncated = center_truncate_path(&path, 18);

        assert_eq!(truncated, format!("~{sep}…cexpialidocious"));
    }

    #[test]
    fn test_format_json_compact_array() {
        let json = r#"[ 1, 2, { "key": "value" }, "string" ]"#;
        let result = format_json_compact(json).unwrap();
        assert_eq!(result, r#"[1, 2, {"key": "value"}, "string"]"#);
    }

    #[test]
    fn test_format_json_compact_already_compact() {
        let json = r#"{"compact":true}"#;
        let result = format_json_compact(json).unwrap();
        assert_eq!(result, r#"{"compact": true}"#);
    }

    #[test]
    fn test_format_json_compact_with_whitespace() {
        let json = r#"
        {
            "name": "John",
            "hobbies": [
                "reading",
                "coding"
            ]
        }
        "#;
        let result = format_json_compact(json).unwrap();
        assert_eq!(
            result,
            r#"{"name": "John", "hobbies": ["reading", "coding"]}"#
        );
    }

    #[test]
    fn test_format_json_compact_invalid_json() {
        let invalid_json = r#"{"invalid": json syntax}"#;
        let result = format_json_compact(invalid_json);
        assert!(result.is_none());
    }

    #[test]
    fn test_format_json_compact_empty_object() {
        let json = r#"{}"#;
        let result = format_json_compact(json).unwrap();
        assert_eq!(result, "{}");
    }

    #[test]
    fn test_format_json_compact_empty_array() {
        let json = r#"[]"#;
        let result = format_json_compact(json).unwrap();
        assert_eq!(result, "[]");
    }

    #[test]
    fn test_format_json_compact_primitive_values() {
        assert_eq!(format_json_compact("42").unwrap(), "42");
        assert_eq!(format_json_compact("true").unwrap(), "true");
        assert_eq!(format_json_compact("false").unwrap(), "false");
        assert_eq!(format_json_compact("null").unwrap(), "null");
        assert_eq!(format_json_compact(r#""string""#).unwrap(), r#""string""#);
    }
}
