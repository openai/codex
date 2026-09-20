//! Retain warning details for the transcript while exposing stable identities to the footer.
//! Message identities deduplicate replay; MCP identities count affected servers, not summary rows.

use super::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WarningId {
    Message(String),
    McpServer(String),
}

/// A retained diagnostic. Identity is independent of wrapping and duplicate delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WarningEntry {
    pub(crate) id: WarningId,
    pub(crate) source: String,
    pub(crate) details: String,
}

pub(crate) fn warning_entries(cells: &[Arc<dyn HistoryCell>]) -> Vec<WarningEntry> {
    let mut entries: Vec<WarningEntry> = Vec::new();
    let mut indices = BTreeMap::new();
    for entry in cells.iter().flat_map(|cell| cell.warning_entries()) {
        if let Some(&index) = indices.get(&entry.id) {
            let existing: &mut WarningEntry = &mut entries[index];
            if !existing.details.contains(&entry.details) {
                existing.details.push_str("\n\n");
                existing.details.push_str(&entry.details);
            }
        } else {
            indices.insert(entry.id.clone(), entries.len());
            entries.push(entry);
        }
    }
    entries
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WarningKey<'a> {
    Message(&'a str),
    McpServer(&'a str),
}

#[derive(Debug)]
pub(crate) struct WarningHistoryCell {
    pub(crate) server_version_notice: bool,
    pub(super) key: String,
    pub(super) diagnostic: String,
    pub(super) details: PrefixedWrappedHistoryCell,
}

impl HistoryCell for WarningHistoryCell {
    fn live_raw_lines(&self) -> Vec<Line<'static>> {
        Vec::new()
    }

    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        Vec::new()
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

pub(crate) fn warning_count(cells: &[Arc<dyn HistoryCell>]) -> usize {
    cells
        .iter()
        .flat_map(|cell| cell.warning_keys())
        .collect::<BTreeSet<_>>()
        .len()
}

#[cfg(test)]
#[path = "warnings_tests.rs"]
mod tests;
