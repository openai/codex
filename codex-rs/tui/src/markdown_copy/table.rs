//! Copy selected table cells independently of grid padding, wrapping, and record labels.
//!
//! Each visual fragment refers to an original cell line. Only selected fragments are
//! serialized; repeated record labels and whitespace removed by wrapping are reconciled
//! by their shared source identity. Unselected cells never supply hidden text.

use super::CopyLine;
use super::SelectedLine;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::LogicalLineSource;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TableLine {
    pub(crate) table: Arc<[&'static str]>,
    fragments: Vec<Fragment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Fragment {
    output: Range<usize>,
    row: usize,
    column: usize,
    source: LogicalLineSource,
}

impl TableLine {
    pub(crate) fn empty(table: &Arc<[&'static str]>) -> Self {
        Self {
            table: Arc::clone(table),
            fragments: Vec::new(),
        }
    }
}

pub(crate) fn annotate_cell(
    lines: &mut [HyperlinkLine],
    copies: Vec<CopyLine>,
    table: &Arc<[&'static str]>,
    row: usize,
    column: usize,
) {
    for (line, copy) in lines.iter_mut().zip(copies) {
        let mut source = LogicalLineSource::from_line(&line.line);
        source.copy = Some(Arc::new(copy));
        attach(
            line,
            Some(TableLine {
                table: Arc::clone(table),
                fragments: vec![Fragment {
                    output: source.range.clone(),
                    row,
                    column,
                    source,
                }],
            }),
        );
    }
}

pub(crate) fn attach(line: &mut HyperlinkLine, table: Option<TableLine>) {
    if let Some(table) = table {
        let mut source = LogicalLineSource::from_line(&line.line);
        source.copy = Some(Arc::new(CopyLine {
            table: Some(table),
            ..Default::default()
        }));
        line.source = Some(source);
    }
}

/// Project a displayed cell fragment into the byte coordinates of its composed row.
pub(crate) fn append(output: &mut Option<TableLine>, source: &LogicalLineSource, offset: usize) {
    let Some(table) = source.copy.as_ref().and_then(|copy| copy.table.as_ref()) else {
        return;
    };
    let output = output.get_or_insert_with(|| TableLine::empty(&table.table));
    for fragment in &table.fragments {
        let start = source.range.start.max(fragment.output.start);
        let end = source.range.end.min(fragment.output.end);
        if start > end || start == end && !fragment.source.text.is_empty() {
            continue;
        }
        let mut cell = fragment.source.clone();
        cell.range = cell.range.start + start - fragment.output.start
            ..cell.range.start + end - fragment.output.start;
        output.fragments.push(Fragment {
            output: offset + source.prefix_bytes + start - source.range.start
                ..offset + source.prefix_bytes + end - source.range.start,
            row: fragment.row,
            column: fragment.column,
            source: cell,
        });
    }
}

pub(super) fn render(lines: &[&SelectedLine], table: &TableLine) -> String {
    let mut cells: BTreeMap<(usize, usize), Vec<LogicalLineSource>> = BTreeMap::new();
    for line in lines {
        let Some(copy) = line
            .source
            .copy
            .as_ref()
            .and_then(|copy| copy.table.as_ref())
        else {
            continue;
        };
        for fragment in &copy.fragments {
            // A selected blank row may have lost all display padding during wrapping.
            let blank = line.source.range.is_empty()
                && line.source.text.trim().is_empty()
                && fragment.source.text.trim().is_empty();
            let start = line.range.start.max(fragment.output.start);
            let end = line.range.end.min(fragment.output.end);
            if !blank && (start > end || start == end && !fragment.source.text.is_empty()) {
                continue;
            }
            let mut source = fragment.source.clone();
            if !blank {
                source.range = source.range.start + start - fragment.output.start
                    ..source.range.start + end - fragment.output.start;
            }
            let parts = cells.entry((fragment.row, fragment.column)).or_default();
            let mut insert_at = parts.len();
            while let Some(index) = parts.iter().position(|previous| {
                Arc::ptr_eq(&previous.text, &source.text)
                    && (previous.range.start <= source.range.end
                        && source.range.start <= previous.range.end
                        || previous.text[previous.range.end.min(source.range.end)
                            ..previous.range.start.max(source.range.start)]
                            .trim()
                            .is_empty())
            }) {
                let previous = parts.remove(index);
                insert_at = insert_at.min(index);
                source.range.start = previous.range.start.min(source.range.start);
                source.range.end = previous.range.end.max(source.range.end);
            }
            parts.insert(insert_at.min(parts.len()), source);
        }
    }
    let includes_separator = lines.iter().any(|line| {
        !line.range.is_empty()
            && line
                .source
                .copy
                .as_ref()
                .and_then(|copy| copy.table.as_ref())
                .is_some_and(|table| table.fragments.is_empty())
    });
    let standalone = cells.len() == 1 && !includes_separator;
    let cells: BTreeMap<_, _> = cells
        .into_iter()
        .map(|(key, parts)| {
            let text = parts
                .into_iter()
                .map(|source| {
                    source.copy.as_ref().map_or_else(
                        || super::escape(&source.text[source.range.clone()]),
                        |copy| {
                            let mut copy = (**copy).clone();
                            copy.table_cell = !standalone;
                            copy.render(&source.text, source.range.clone(), /*depth*/ 0)
                        },
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            (key, text)
        })
        .collect();
    if standalone {
        return cells.into_values().next().unwrap_or_default();
    }
    if cells.is_empty() {
        return lines
            .iter()
            .map(|line| super::escape(&line.source.text[line.range.clone()]))
            .collect::<Vec<_>>()
            .join("\n");
    }
    let mut rows: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    rows.insert(/*key*/ 0, vec![""; table.table.len()]);
    for ((row, column), text) in &cells {
        rows.entry(*row)
            .or_insert_with(|| vec![""; table.table.len()])[*column] = text;
    }
    let mut out = String::new();
    for (row, values) in rows {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("| {} |", values.join(" | ")));
        if row == 0 {
            out.push_str("\n|");
            for alignment in table.table.iter() {
                out.push_str(alignment);
                out.push('|');
            }
        }
    }
    out
}
