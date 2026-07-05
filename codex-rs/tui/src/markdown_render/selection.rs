//! Semantic plain-text projection for mouse selection.
//!
//! This deliberately follows Markdown structure rather than removing characters from rendered
//! terminal rows. Styling delimiters and table chrome never enter the canonical copy string.

use std::ops::Range;
use std::path::Path;

use crate::conversation_selection::CellSelectionProjection;
use crate::conversation_selection::SelectionRow;
#[cfg(test)]
use pulldown_cmark::Event;
#[cfg(test)]
use pulldown_cmark::Options;
#[cfg(test)]
use pulldown_cmark::Parser;
#[cfg(test)]
use pulldown_cmark::Tag;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

#[derive(Clone, Debug)]
struct MarkdownSelectionTableRow {
    source: Range<usize>,
    document: Range<usize>,
}

#[derive(Clone, Debug)]
struct MarkdownSelectionTable {
    text: String,
    document: Range<usize>,
    rows: Vec<MarkdownSelectionTableRow>,
}

impl MarkdownSelectionTable {
    fn map_source_range(&self, source: &Range<usize>) -> Option<Range<usize>> {
        let row = self
            .rows
            .iter()
            .find(|row| row.source.start <= source.start && source.end <= row.source.end)?;
        let start = row
            .document
            .start
            .checked_add(source.start.checked_sub(row.source.start)?)?;
        let end = row
            .document
            .start
            .checked_add(source.end.checked_sub(row.source.start)?)?;
        Some(start..end)
    }
}

struct MarkdownSelectionDocument {
    text: String,
    tables: Option<Vec<MarkdownSelectionTable>>,
}

#[cfg(test)]
pub(crate) fn render_markdown_selection_text(input: &str, cwd: Option<&Path>) -> String {
    render_markdown_selection_document(input, cwd).text
}

fn render_markdown_selection_document(
    input: &str,
    cwd: Option<&Path>,
) -> MarkdownSelectionDocument {
    let (lines, table_layouts) = super::render_markdown_selection_lines_with_cwd(input, cwd);
    let mut text = String::new();
    let mut line_ranges = Vec::with_capacity(lines.len());
    for (line_index, line) in lines.into_iter().enumerate() {
        if line_index > 0 {
            text.push('\n');
        }
        let start = text.len();
        for span in line.line.spans {
            text.push_str(&span.content);
        }
        line_ranges.push(start..text.len());
    }

    let tables = table_layouts
        .into_iter()
        .map(|layout| -> Option<MarkdownSelectionTable> {
            let mut rows = Vec::with_capacity(layout.source_rows.len());
            let document_start = line_ranges.get(layout.row_start)?.start;
            let mut document_end = document_start;
            for (row_offset, source) in layout.source_rows.into_iter().enumerate() {
                let line_index = layout.row_start.checked_add(row_offset)?;
                let line_range = line_ranges.get(line_index)?.clone();
                let line = text.get(line_range.clone())?;
                let source_text = layout.text.get(source.clone())?;
                let prefix = line.strip_suffix(source_text)?;
                let row_start = line_range.start.checked_add(prefix.len())?;
                let row_end = row_start.checked_add(source_text.len())?;
                rows.push(MarkdownSelectionTableRow {
                    source,
                    document: row_start..row_end,
                });
                document_end = line_range.end;
            }
            Some(MarkdownSelectionTable {
                text: layout.text,
                document: document_start..document_end,
                rows,
            })
        })
        .collect();

    MarkdownSelectionDocument { text, tables }
}

#[cfg(test)]
pub(crate) fn selection_text_contains_table(input: &str) -> bool {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    Parser::new_ext(input, options).any(|event| matches!(event, Event::Start(Tag::Table(_))))
}

/// Builds a source-backed projection for width-aware markdown output.
///
/// Normal markdown uses the shared grapheme aligner. Tables replace those rows with explicit cell
/// mappings while retaining stable logical row/cell order in canonical clipboard text.
pub(crate) fn render_markdown_selection_projection(
    input: &str,
    markdown_width: usize,
    cwd: Option<&Path>,
    display_lines: Vec<Line<'static>>,
    display_width: u16,
    outer_prefix_columns: u16,
) -> Option<CellSelectionProjection> {
    if display_lines.is_empty() || display_width == 0 {
        return None;
    }
    let MarkdownSelectionDocument {
        text,
        tables: source_tables,
    } = render_markdown_selection_document(input, cwd);
    let (_, table_layouts) =
        super::render_markdown_lines_with_table_selection_and_cwd(input, Some(markdown_width), cwd);
    if table_layouts.is_empty() {
        return CellSelectionProjection::from_display_lines(
            text,
            display_lines,
            display_width,
            outer_prefix_columns,
        );
    }
    let Some(source_tables) = source_tables else {
        return CellSelectionProjection::from_display_lines(
            text,
            display_lines,
            display_width,
            outer_prefix_columns,
        );
    };
    if source_tables.len() != table_layouts.len() {
        return CellSelectionProjection::from_display_lines(
            text,
            display_lines,
            display_width,
            outer_prefix_columns,
        );
    }

    let mut rows = Vec::new();
    let mut text_cursor = 0usize;
    let mut display_cursor = 0usize;
    for (layout, source_table) in table_layouts.into_iter().zip(source_tables) {
        if layout.text != source_table.text
            || source_table.document.start < text_cursor
            || layout.row_start < display_cursor
        {
            return CellSelectionProjection::from_display_lines(
                text,
                display_lines,
                display_width,
                outer_prefix_columns,
            );
        }
        let table_start = source_table.document.start;
        let table_row_start = layout.row_start.min(display_lines.len());
        let table_row_count = layout.rows.len();
        rows.extend(aligned_rows(
            &text[text_cursor..table_start],
            &display_lines[display_cursor..table_row_start],
            display_width,
            outer_prefix_columns,
            text_cursor,
        ));
        for mut table_row in layout.rows {
            for segment in &mut table_row.segments {
                let Some(bytes) = source_table.map_source_range(&segment.bytes) else {
                    return CellSelectionProjection::from_display_lines(
                        text,
                        display_lines,
                        display_width,
                        outer_prefix_columns,
                    );
                };
                segment.columns = segment.columns.start.saturating_add(outer_prefix_columns)
                    ..segment.columns.end.saturating_add(outer_prefix_columns);
                segment.bytes = bytes;
            }
            rows.push(table_row);
        }
        text_cursor = source_table.document.end;
        display_cursor = table_row_start
            .saturating_add(table_row_count)
            .min(display_lines.len());
    }
    rows.extend(aligned_rows(
        &text[text_cursor..],
        &display_lines[display_cursor..],
        display_width,
        outer_prefix_columns,
        text_cursor,
    ));

    CellSelectionProjection::from_rows(text, rows)
}

fn aligned_rows(
    text: &str,
    display_lines: &[Line<'static>],
    display_width: u16,
    outer_prefix_columns: u16,
    byte_offset: usize,
) -> Vec<SelectionRow> {
    let Some(projection) = CellSelectionProjection::from_display_lines(
        text.to_string(),
        display_lines.to_vec(),
        display_width,
        outer_prefix_columns,
    ) else {
        return display_lines
            .iter()
            .flat_map(|line| {
                let row_count = Paragraph::new(line.clone())
                    .wrap(Wrap { trim: false })
                    .line_count(display_width)
                    .max(/*other*/ 1);
                std::iter::repeat_n(SelectionRow::default(), row_count)
            })
            .collect();
    };
    let mut rows = projection.rows().to_vec();
    for row in &mut rows {
        for segment in &mut row.segments {
            segment.bytes = segment.bytes.start.saturating_add(byte_offset)
                ..segment.bytes.end.saturating_add(byte_offset);
        }
    }
    rows
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
