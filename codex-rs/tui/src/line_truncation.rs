use ratatui::buffer::CellWidth;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

/// Paragraph paints each visible grapheme separately, even when the whole string
/// has a narrower Unicode width (for example, Arabic lam followed by alef).
pub(crate) fn span_width(span: &Span<'_>) -> usize {
    span.styled_graphemes(Style::default())
        .map(|g| usize::from(g.symbol.cell_width()))
        .sum()
}

pub(crate) fn line_width(line: &Line<'_>) -> usize {
    line.iter().map(span_width).sum()
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
        let span_width = span_width(&span);

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

        let mut prefix = String::new();
        for grapheme in span.styled_graphemes(Style::default()) {
            let grapheme_width = usize::from(grapheme.symbol.cell_width());
            if used + grapheme_width > max_width {
                break;
            }
            prefix.push_str(grapheme.symbol);
            used += grapheme_width;
        }

        if !prefix.is_empty() {
            spans_out.push(Span::styled(prefix, span.style));
        }

        break;
    }

    Line {
        style,
        alignment,
        spans: spans_out,
    }
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
#[path = "line_truncation_tests.rs"]
mod tests;
