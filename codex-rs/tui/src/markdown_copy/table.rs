//! Retain original table-cell ranges through grid padding, wrapping, and record labels.
//!
//! Each displayed fragment keeps its source line and cell coordinates so selection
//! serialization can recover only visible, selected content.

use super::CopyLine;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::LogicalLineSource;
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
