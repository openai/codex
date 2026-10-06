//! Preserve complete hyperlink destinations through selection-row wrapping and clipping.

use super::GenericDisplayRow;
use super::SelectionDescriptionLayout;
use super::build_full_line;
use super::should_wrap_name_in_column;
use super::wrap_indent;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::width::display_width;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

pub(super) fn full_hyperlink_row(row: &GenericDisplayRow, line: Line<'static>) -> HyperlinkLine {
    let prefix = row
        .name_prefix_spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let text = line.to_string();
    let visible_name = text.strip_prefix(&prefix).unwrap_or("");
    let common = row
        .name
        .chars()
        .zip(visible_name.chars())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch)
        .collect::<String>();
    let start = display_width(&prefix);
    let end = start + display_width(&common);
    let mut source = crate::terminal_hyperlinks::annotate_web_urls_in_line(line);
    source
        .hyperlinks
        .retain(|link| link.columns.end <= start || link.columns.start >= end);
    let name = crate::terminal_hyperlinks::annotate_web_urls_in_line(row.name.clone().into());
    source
        .hyperlinks
        .extend(name.hyperlinks.into_iter().filter_map(|link| {
            let last = (start + link.columns.end).min(end);
            let first = start + link.columns.start;
            (first < last).then(|| link.with_columns(first..last))
        }));
    source.hyperlinks.sort_by_key(|link| link.columns.start);
    source
}

fn wrap_column(text: &str, options: crate::wrapping::RtOptions<'static>) -> Vec<HyperlinkLine> {
    text.lines()
        .enumerate()
        .flat_map(|(index, text)| {
            let options = if index == 0 {
                options.clone()
            } else {
                options
                    .clone()
                    .initial_indent(options.subsequent_indent.clone())
            };
            let source =
                crate::terminal_hyperlinks::annotate_web_urls_in_line(text.to_owned().into());
            let wrapped =
                crate::wrapping::word_wrap_line_with_source(&source.line, options.clone());
            crate::terminal_hyperlinks::remap_source_wrapped_line(&source, wrapped)
        })
        .collect()
}

pub(super) fn wrap_two_column_row(
    row: &GenericDisplayRow,
    desc_col: usize,
    width: u16,
) -> Vec<HyperlinkLine> {
    use crate::wrapping::RtOptions;

    let Some(description) = row.description.as_deref() else {
        return Vec::new();
    };

    let width = width.max(1);
    let max_desc_col = width.saturating_sub(1) as usize;
    if max_desc_col == 0 {
        // No valid description column exists at this width; let callers fall
        // back to single-line wrapping path.
        return Vec::new();
    }

    let desc_col = desc_col.clamp(1, max_desc_col);
    let left_width = desc_col.saturating_sub(2).max(1);
    let right_width = width.saturating_sub(desc_col as u16).max(1) as usize;
    let name_wrap_indent = row
        .wrap_indent
        .unwrap_or(0)
        .min(left_width.saturating_sub(1));

    let name_options = RtOptions::new(left_width)
        .initial_indent(Line::from(""))
        .subsequent_indent(Line::from(" ".repeat(name_wrap_indent)));
    let name_lines = wrap_column(&row.name, name_options);

    let desc_options = RtOptions::new(right_width).initial_indent(Line::from(""));
    let desc_lines = wrap_column(description, desc_options);

    let rows = name_lines.len().max(desc_lines.len()).max(1);
    let mut out = Vec::with_capacity(rows);
    for idx in 0..rows {
        let mut hyperlinks = name_lines
            .get(idx)
            .map(|name| name.hyperlinks.clone())
            .unwrap_or_default();
        let mut spans: Vec<Span<'static>> = Vec::new();
        if let Some(name) = name_lines.get(idx) {
            spans.push(name.line.to_string().into());
        }

        if let Some(desc) = desc_lines.get(idx) {
            let left_used = spans
                .iter()
                .map(|span| display_width(span.content.as_ref()))
                .sum::<usize>();
            let gap = if left_used == 0 {
                desc_col
            } else {
                desc_col.saturating_sub(left_used).max(2)
            };
            if gap > 0 {
                spans.push(" ".repeat(gap).into());
            }
            let column = left_used + gap;
            hyperlinks.extend(desc.hyperlinks.iter().map(|link| {
                link.with_columns(column + link.columns.start..column + link.columns.end)
            }));
            spans.push(desc.line.to_string().dim());
        }

        let mut line = HyperlinkLine::new(Line::from(spans));
        line.hyperlinks = hyperlinks;
        out.push(line);
    }

    out
}

fn wrap_standard_row(
    row: &GenericDisplayRow,
    desc_col: usize,
    width: u16,
    description_layout: SelectionDescriptionLayout,
) -> Vec<HyperlinkLine> {
    use crate::wrapping::RtOptions;

    let full_line = full_hyperlink_row(
        row,
        build_full_line(row, desc_col, width, description_layout),
    );
    let continuation_indent = wrap_indent(row, desc_col, width);
    let options = RtOptions::new(width.max(1) as usize)
        .initial_indent(Line::from(""))
        .subsequent_indent(Line::from(" ".repeat(continuation_indent)));
    let wrapped = crate::wrapping::word_wrap_line_with_source(&full_line.line, options);
    crate::terminal_hyperlinks::remap_source_wrapped_line(&full_line, wrapped)
}

pub(super) fn wrap_row_lines(
    row: &GenericDisplayRow,
    desc_col: usize,
    width: u16,
    description_layout: SelectionDescriptionLayout,
) -> Vec<HyperlinkLine> {
    if desc_col > 0 && should_wrap_name_in_column(row) {
        let wrapped = wrap_two_column_row(row, desc_col, width);
        if !wrapped.is_empty() {
            return wrapped;
        }
    }

    wrap_standard_row(row, desc_col, width, description_layout)
}

#[cfg(test)]
#[path = "selection_row_links_tests.rs"]
mod tests;
