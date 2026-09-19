//! Render inline warnings and retain diagnostic identities for the later warning footer.
//! Message identities deduplicate replay; MCP identities count affected servers, not summary rows.

use super::*;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) enum WarningId {
    Message(String),
    McpServer(String),
}

/// A retained diagnostic. Identity is independent of wrapping and duplicate delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) struct WarningEntry {
    pub(crate) id: WarningId,
    pub(crate) source: String,
    pub(crate) details: String,
}

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn warning_entries(cells: &[Arc<dyn HistoryCell>]) -> Vec<WarningEntry> {
    let mut entries: Vec<WarningEntry> = Vec::new();
    for entry in cells.iter().flat_map(|cell| cell.warning_entries()) {
        if let Some(existing) = entries.iter_mut().find(|existing| existing.id == entry.id) {
            if !existing.details.contains(&entry.details) {
                existing.details.push_str("\n\n");
                existing.details.push_str(&entry.details);
            }
        } else {
            entries.push(entry);
        }
    }
    entries
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) enum WarningKey<'a> {
    Message(&'a str),
    McpServer(&'a str),
}

#[derive(Debug)]
pub(crate) struct WarningHistoryCell {
    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    pub(super) key: String,
    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    pub(super) diagnostic: String,
    pub(super) details: PrefixedWrappedHistoryCell,
}

impl HistoryCell for WarningHistoryCell {
    fn live_raw_lines(&self) -> Vec<Line<'static>> {
        self.details.raw_lines()
    }

    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.details.display_lines(width)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.details.transcript_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.details.raw_lines()
    }

    fn warning_keys(&self) -> Vec<WarningKey<'_>> {
        vec![WarningKey::Message(&self.key)]
    }

    fn warning_entries(&self) -> Vec<WarningEntry> {
        vec![WarningEntry {
            id: WarningId::Message(self.key.clone()),
            source: "Warning".into(),
            details: self.diagnostic.clone(),
        }]
    }
}

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn warning_count(cells: &[Arc<dyn HistoryCell>]) -> usize {
    cells
        .iter()
        .flat_map(|cell| cell.warning_keys())
        .collect::<BTreeSet<_>>()
        .len()
}
