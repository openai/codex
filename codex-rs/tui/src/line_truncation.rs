use ratatui::text::Line;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::terminal_wrappers::parse_zero_width_terminal_wrapper;
use crate::terminal_wrappers::strip_zero_width_terminal_wrappers;
use crate::terminal_wrappers::visible_width as wrapped_visible_width;

pub(crate) fn line_width(line: &Line<'_>) -> usize {
    line.iter()
        .map(|span| visible_width(span.content.as_ref()))
        .sum()
}

pub(crate) fn truncate_line_to_width(line: Line<'static>, max_width: usize) -> Line<'static> {
    if max_width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let Line {
        style,
        alignment,
        spans,
    } = line;
    let mut used = 0usize;
    let mut spans_out: Vec<Span<'static>> = Vec::with_capacity(spans.len());

    for span in spans {
        let span_width = visible_width(span.content.as_ref());

        if span_width == 0 {
            spans_out.push(span);
            continue;
        }

        if used >= max_width {
            break;
        }

        if used + span_width <= max_width {
            used += span_width;
            spans_out.push(span);
            continue;
        }

        let style = span.style;
        let text = span.content.as_ref();
        let parsed_wrapper = parse_zero_width_terminal_wrapper(text);
        let visible_text = parsed_wrapper.map_or_else(
            || strip_zero_width_terminal_wrappers(text),
            |wrapper| wrapper.text.to_string(),
        );
        // Truncate by visible grapheme clusters, not scalar values. This keeps
        // multi-codepoint emoji intact and lets zero-width wrappers stay
        // attached to the truncated visible prefix.
        let mut end_idx = 0usize;
        for grapheme in UnicodeSegmentation::graphemes(visible_text.as_str(), true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if used + grapheme_width > max_width {
                break;
            }
            end_idx += grapheme.len();
            used += grapheme_width;
        }

        if end_idx > 0 {
            let truncated_text = &visible_text[..end_idx];
            let content = parsed_wrapper.map_or_else(
                || truncated_text.to_string(),
                |wrapper| format!("{}{}{}", wrapper.prefix, truncated_text, wrapper.suffix),
            );
            spans_out.push(Span::styled(content, style));
        }

        break;
    }

    Line {
        style,
        alignment,
        spans: spans_out,
    }
}

fn visible_width(text: &str) -> usize {
    wrapped_visible_width(text)
}

/// Truncate a styled line to `max_width` and append an ellipsis on overflow.
///
/// Intended for short UI rows. This preserves a fast no-overflow path (width
/// pre-scan + return original line unchanged) and uses `truncate_line_to_width`
/// for the overflow case.
/// Performance should be reevaluated if using this method in loops/over larger content in the future.
pub(crate) fn truncate_line_with_ellipsis_if_overflow(
    line: Line<'static>,
    max_width: usize,
) -> Line<'static> {
    if max_width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    if line_width(&line) <= max_width {
        return line;
    }

    let truncated = truncate_line_to_width(line, max_width.saturating_sub(1));
    let Line {
        style,
        alignment,
        mut spans,
    } = truncated;
    let ellipsis_style = spans.last().map(|span| span.style).unwrap_or_default();
    spans.push(Span::styled("…", ellipsis_style));
    Line {
        style,
        alignment,
        spans,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use ratatui::style::Stylize;
    use ratatui::text::Line;
    use ratatui::text::Span;

    use crate::osc8::osc8_hyperlink;

    use super::*;

    #[test]
    fn line_width_counts_osc8_wrapped_text_as_visible_text_only() {
        let line = Line::from(vec![
            "See ".into(),
            Span::from(osc8_hyperlink("https://example.com/docs", "docs")).underlined(),
        ]);

        assert_eq!(line_width(&line), 8);
    }

    #[test]
    fn truncate_line_to_width_preserves_osc8_wrapped_prefix() {
        let line = Line::from(vec![
            "See ".into(),
            Span::from(osc8_hyperlink("https://example.com/docs", "docs")).underlined(),
        ]);

        let truncated = truncate_line_to_width(line, 6);

        let expected = Line::from(vec![
            "See ".into(),
            Span::from(osc8_hyperlink("https://example.com/docs", "do")).underlined(),
        ]);
        assert_eq!(truncated, expected);
    }

    #[test]
    fn truncate_line_to_width_preserves_osc8_between_ascii_spans() {
        let line = Line::from(vec![
            "A".into(),
            Span::from(osc8_hyperlink("https://example.com/docs", "BC"))
                .cyan()
                .underlined(),
            "DE".into(),
        ]);

        let truncated = truncate_line_to_width(line, 4);

        let expected = Line::from(vec![
            "A".into(),
            Span::from(osc8_hyperlink("https://example.com/docs", "BC"))
                .cyan()
                .underlined(),
            "D".into(),
        ]);
        assert_eq!(truncated, expected);
    }

    #[test]
    fn truncate_line_with_ellipsis_if_overflow_preserves_osc8_wrapped_prefix() {
        let line = Line::from(vec![
            "See ".into(),
            Span::from(osc8_hyperlink("https://example.com/docs", "docs")).underlined(),
        ]);

        let truncated = truncate_line_with_ellipsis_if_overflow(line, 7);

        let expected = Line::from(vec![
            "See ".into(),
            Span::from(osc8_hyperlink("https://example.com/docs", "do")).underlined(),
            "…".underlined(),
        ]);
        assert_eq!(truncated, expected);
    }

    #[test]
    fn truncate_line_to_width_preserves_st_terminated_wrapper_with_params() {
        let wrapped = "\u{1b}]8;id=abc;https://example.com\u{1b}\\docs\u{1b}]8;;\u{1b}\\";
        let line = Line::from(vec!["See ".into(), Span::from(wrapped).cyan().underlined()]);

        let truncated = truncate_line_to_width(line, 6);

        let expected = Line::from(vec![
            "See ".into(),
            Span::from("\u{1b}]8;id=abc;https://example.com\u{1b}\\do\u{1b}]8;;\u{1b}\\")
                .cyan()
                .underlined(),
        ]);
        assert_eq!(truncated, expected);
    }

    #[test]
    fn truncate_line_to_width_cuts_by_grapheme_not_scalar_value() {
        let line = Line::from(vec![
            Span::from(osc8_hyperlink(
                "https://example.com/docs",
                "👨\u{200d}👩\u{200d}👧\u{200d}👦x",
            ))
            .underlined(),
        ]);

        let truncated = truncate_line_to_width(line, 2);

        let expected = Line::from(vec![
            Span::from(osc8_hyperlink(
                "https://example.com/docs",
                "👨\u{200d}👩\u{200d}👧\u{200d}👦",
            ))
            .underlined(),
        ]);
        assert_eq!(truncated, expected);
    }

    #[test]
    fn truncate_line_to_width_preserves_malformed_unterminated_wrapper_verbatim_until_limit() {
        let malformed = "See \u{1b}]8;;https://example.com\u{7}docs";
        let line = Line::from(malformed);

        let truncated = truncate_line_to_width(line, 7);

        assert_eq!(truncated, Line::from("See \u{1b}]"));
    }
}
