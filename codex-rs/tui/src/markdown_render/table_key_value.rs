//! Vertical key/value rendering for markdown tables that no longer scan well as grids.

use super::TABLE_BODY_SEPARATOR_CHAR;
use super::TableCell;
use super::TableColumnKind;
use super::TableColumnMetrics;
use crate::render::line_utils::line_to_static;
use crate::render::line_utils::push_owned_lines;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::remap_wrapped_line;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

const FIELD_LEADING_PADDING: usize = 1;
const FIELD_GAP: usize = 2;
const MIN_VALUE_WIDTH: usize = 3;
const MIN_ALIGNED_COMPACT_VALUE_WIDTH: usize = 12;
const MIN_ALIGNED_EXPANSIVE_VALUE_WIDTH: usize = 24;
const MIN_SCANNABLE_NARRATIVE_WIDTH: usize = 12;
const MIN_SCANNABLE_TOKEN_HEAVY_WIDTH: usize = 12;
const CRAMPED_EXPANSIVE_CELL_LINES: usize = 4;
const CATASTROPHIC_NARRATIVE_CELL_LINES: usize = 7;
const STACKED_VALUE_INDENT: usize = 2;

/// Switch modes after enough records contain values the grid can no longer
/// present in useful chunks or expansive content collapses into tall strips.
pub(super) fn should_render_records(
    rows: &[Vec<TableCell>],
    column_widths: &[usize],
    metrics: &[TableColumnMetrics],
) -> bool {
    if rows.is_empty() {
        return false;
    }

    let affected_rows = rows
        .iter()
        .filter(|row| {
            let contains_fragmented_value =
                row.iter()
                    .zip(column_widths)
                    .zip(metrics)
                    .any(|((cell, width), metrics)| {
                        let has_fragmented_token = cell
                            .plain_text()
                            .split_whitespace()
                            .any(|token| token.width() > *width);
                        match metrics.kind {
                            TableColumnKind::Compact => has_fragmented_token,
                            TableColumnKind::TokenHeavy => {
                                *width < MIN_SCANNABLE_TOKEN_HEAVY_WIDTH && has_fragmented_token
                            }
                            TableColumnKind::Narrative => false,
                        }
                    });

            contains_fragmented_value || expansive_cells_are_starved(row, column_widths, metrics)
        })
        .count();
    let threshold = if rows.len() == 1 {
        1
    } else {
        2.max(rows.len().div_ceil(3))
    };

    affected_rows >= threshold
}

fn expansive_cells_are_starved(
    row: &[TableCell],
    column_widths: &[usize],
    metrics: &[TableColumnMetrics],
) -> bool {
    let expansive_cells: Vec<(TableColumnKind, usize, usize)> = row
        .iter()
        .zip(column_widths)
        .zip(metrics)
        .filter(|&((_cell, _width), metrics)| metrics.kind != TableColumnKind::Compact)
        .map(|((cell, width), metrics)| (metrics.kind, *width, wrap_cell(cell, *width).len()))
        .collect();

    expansive_cells
        .iter()
        .filter(|(_, _, height)| *height >= CRAMPED_EXPANSIVE_CELL_LINES)
        .count()
        >= 2
        || expansive_cells.iter().any(|(kind, width, height)| {
            *kind == TableColumnKind::Narrative
                && *width < MIN_SCANNABLE_NARRATIVE_WIDTH
                && *height >= CATASTROPHIC_NARRATIVE_CELL_LINES
        })
}

pub(super) fn render_records(
    headers: &[TableCell],
    rows: &[Vec<TableCell>],
    metrics: &[TableColumnMetrics],
    available_width: Option<usize>,
    label_style: Style,
    separator_style: Style,
) -> Vec<HyperlinkLine> {
    let label_width = headers
        .iter()
        .map(|header| header.plain_text().width())
        .max()
        .unwrap_or(0);
    let minimum_value_width = if metrics
        .iter()
        .any(|metrics| metrics.kind != TableColumnKind::Compact)
    {
        MIN_ALIGNED_EXPANSIVE_VALUE_WIDTH
    } else {
        MIN_ALIGNED_COMPACT_VALUE_WIDTH
    };
    let aligned_fields = available_width.is_none_or(|width| {
        FIELD_LEADING_PADDING + label_width + FIELD_GAP + minimum_value_width <= width
    });
    let mut out = Vec::new();

    for (row_index, row) in rows.iter().enumerate() {
        for (header, value) in headers.iter().zip(row) {
            if aligned_fields {
                render_aligned_field(
                    &mut out,
                    header,
                    value,
                    label_width,
                    available_width,
                    label_style,
                );
            } else {
                render_stacked_field(&mut out, header, value, available_width, label_style);
            }
        }
        if row_index + 1 < rows.len() {
            let width = available_width.unwrap_or_else(|| widest_line_width(&out));
            out.push(HyperlinkLine::new(Line::from(Span::styled(
                TABLE_BODY_SEPARATOR_CHAR.to_string().repeat(width),
                separator_style,
            ))));
        }
    }

    out
}

fn render_aligned_field(
    out: &mut Vec<HyperlinkLine>,
    header: &TableCell,
    value: &TableCell,
    label_width: usize,
    available_width: Option<usize>,
    label_style: Style,
) {
    let value_indent = FIELD_LEADING_PADDING + label_width + FIELD_GAP;
    let value_width = available_width
        .map(|width| width.saturating_sub(value_indent).max(MIN_VALUE_WIDTH))
        .unwrap_or_else(|| cell_width(value).max(MIN_VALUE_WIDTH));
    let wrapped_value = wrap_cell(value, value_width);
    for (line_index, value_line) in wrapped_value.into_iter().enumerate() {
        let mut spans = Vec::new();
        if line_index == 0 {
            let label = header.plain_text();
            spans.push(Span::raw(" ".repeat(FIELD_LEADING_PADDING)));
            spans.push(Span::styled(label.clone(), label_style));
            spans.push(Span::raw(
                " ".repeat(label_width.saturating_sub(label.width()) + FIELD_GAP),
            ));
        } else {
            spans.push(Span::raw(" ".repeat(value_indent)));
        }
        push_prefixed_value_line(out, spans, value_line);
    }
}

fn render_stacked_field(
    out: &mut Vec<HyperlinkLine>,
    header: &TableCell,
    value: &TableCell,
    available_width: Option<usize>,
    label_style: Style,
) {
    let label_width = available_width
        .map(|width| width.saturating_sub(FIELD_LEADING_PADDING).max(1))
        .unwrap_or_else(|| header.plain_text().width().max(1));
    let label = Line::from(Span::styled(header.plain_text(), label_style));
    let mut wrapped_labels = Vec::new();
    push_owned_lines(
        &word_wrap_line(&label, RtOptions::new(label_width)),
        &mut wrapped_labels,
    );
    for label_line in wrapped_labels {
        let mut spans = vec![Span::raw(" ".repeat(FIELD_LEADING_PADDING))];
        spans.extend(label_line.spans);
        out.push(HyperlinkLine::new(Line::from(spans)));
    }

    let value_width = available_width
        .map(|width| width.saturating_sub(STACKED_VALUE_INDENT).max(1))
        .unwrap_or_else(|| cell_width(value).max(1));
    for value_line in wrap_cell(value, value_width) {
        push_prefixed_value_line(
            out,
            vec![Span::raw(" ".repeat(STACKED_VALUE_INDENT))],
            value_line,
        );
    }
}

fn push_prefixed_value_line(
    out: &mut Vec<HyperlinkLine>,
    mut prefix: Vec<Span<'static>>,
    mut value_line: HyperlinkLine,
) {
    let shift = prefix
        .iter()
        .map(|span| span.content.width())
        .sum::<usize>();
    prefix.append(&mut value_line.line.spans);
    let mut output_line = HyperlinkLine::new(Line::from(prefix));
    output_line
        .hyperlinks
        .extend(value_line.hyperlinks.into_iter().map(|mut link| {
            link.columns = link.columns.start + shift..link.columns.end + shift;
            link
        }));
    out.push(output_line);
}

fn wrap_cell(cell: &TableCell, width: usize) -> Vec<HyperlinkLine> {
    if cell.lines.is_empty() {
        return vec![HyperlinkLine::new(Line::default())];
    }

    let mut wrapped = Vec::new();
    for source_line in &cell.lines {
        let rendered = word_wrap_line(&source_line.line, RtOptions::new(width.max(1)))
            .into_iter()
            .map(|line| line_to_static(&line))
            .collect::<Vec<_>>();
        if rendered.is_empty() {
            wrapped.push(HyperlinkLine::new(Line::default()));
        } else {
            wrapped.extend(remap_wrapped_line(source_line, rendered));
        }
    }
    if wrapped.is_empty() {
        wrapped.push(HyperlinkLine::new(Line::default()));
    }
    wrapped
}

fn cell_width(cell: &TableCell) -> usize {
    cell.lines
        .iter()
        .map(|line| {
            line.line
                .spans
                .iter()
                .map(|span| span.content.width())
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0)
}

fn widest_line_width(lines: &[HyperlinkLine]) -> usize {
    lines
        .iter()
        .map(|line| {
            line.line
                .spans
                .iter()
                .map(|span| span.content.width())
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0)
}
