//! Vertical key/value rendering for markdown tables that no longer scan well as grids.

use super::TABLE_BODY_SEPARATOR_CHAR;
use super::TableCell;
use super::TableColumnKind;
use super::TableColumnMetrics;
use crate::render::line_utils::line_to_static;
use crate::render::line_utils::push_owned_lines;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::remap_wrapped_line;
use crate::width::display_width;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

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
                            .any(|token| display_width(token) > *width);
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
        .map(|header| display_width(&header.plain_text()))
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
        let start = out.len();
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
        for line in &mut out[start..] {
            if let Some(table) = line
                .source
                .as_mut()
                .and_then(|source| source.copy.as_mut())
                .and_then(|copy| std::sync::Arc::make_mut(copy).table.as_mut())
            {
                table.record_row = Some(row_index + 1);
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
        let prefix = if line_index == 0 {
            let mut label = label_line(header, label_style);
            let padding = label_width.saturating_sub(label.source_width()) + FIELD_GAP;
            label.line.push_span(Span::raw(" ".repeat(padding)));
            prefix_line(" ".repeat(FIELD_LEADING_PADDING).into(), label)
        } else {
            HyperlinkLine::new(Line::raw(" ".repeat(value_indent)))
        };
        out.push(join_lines(prefix, value_line));
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
        .unwrap_or_else(|| display_width(&header.plain_text()).max(1));
    let label = label_line(header, label_style);
    let mut wrapped_labels = Vec::new();
    push_owned_lines(
        &word_wrap_line(&label.line, RtOptions::new(label_width)),
        &mut wrapped_labels,
    );
    for label_line in remap_wrapped_line(&label, wrapped_labels) {
        out.push(prefix_line(
            " ".repeat(FIELD_LEADING_PADDING).into(),
            label_line,
        ));
    }

    let value_width = available_width
        .map(|width| width.saturating_sub(STACKED_VALUE_INDENT).max(1))
        .unwrap_or_else(|| cell_width(value).max(1));
    for value_line in wrap_cell(value, value_width) {
        out.push(prefix_line(
            " ".repeat(STACKED_VALUE_INDENT).into(),
            value_line,
        ));
    }
}

fn prefix_line(prefix: Span<'static>, line: HyperlinkLine) -> HyperlinkLine {
    join_lines(HyperlinkLine::new(Line::from(prefix)), line)
}

fn join_lines(mut prefix: HyperlinkLine, mut line: HyperlinkLine) -> HyperlinkLine {
    let mut copy = None;
    if let Some(source) = &prefix.source {
        crate::markdown_copy::table::append(&mut copy, source, /*offset*/ 0);
    }
    if let Some(source) = &line.source {
        let bytes = prefix
            .line
            .spans
            .iter()
            .map(|span| span.content.len())
            .sum();
        crate::markdown_copy::table::append(&mut copy, source, bytes);
    }
    let shift = prefix.source_width();
    prefix.line.spans.append(&mut line.line.spans);
    prefix
        .hyperlinks
        .extend(line.hyperlinks.into_iter().map(|mut link| {
            link.columns = link.columns.start + shift..link.columns.end + shift;
            link
        }));
    crate::markdown_copy::table::attach(&mut prefix, copy);
    prefix
}

fn label_line(header: &TableCell, style: Style) -> HyperlinkLine {
    let mut label = HyperlinkLine::new(Line::default());
    for (index, line) in header.lines.iter().enumerate() {
        if index > 0 {
            label.line.push_span(Span::raw(" "));
        }
        label = join_lines(label, line.clone());
    }
    if label.hyperlinks.is_empty() {
        label.line = Line::from(Span::styled(header.plain_text(), style));
    } else {
        for span in &mut label.line.spans {
            span.style = style;
        }
    }
    label
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
            wrapped.push(source_line.clone());
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
        .map(HyperlinkLine::width)
        .max()
        .unwrap_or(0)
}

fn widest_line_width(lines: &[HyperlinkLine]) -> usize {
    lines.iter().map(HyperlinkLine::width).max().unwrap_or(0)
}
