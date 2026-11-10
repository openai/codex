//! Output-free tool summaries for the full-screen conversation.
use super::*;

pub(crate) struct ToolSummary {
    pub(super) ids: Vec<String>,
    calls: Vec<String>,
}

impl ToolSummary {
    pub(crate) fn layout(
        &self,
        width: u16,
        expanded: bool,
        auxiliary: Vec<HyperlinkLine>,
    ) -> TextLayout {
        let mut lines = Vec::new();
        if expanded {
            for call in &self.calls {
                let call = call.replace(char::is_control, " ");
                let line = Line::from(format!("      {call}")).dim();
                lines.push(crate::history_cell::activity_preview::clipped_line(
                    line, width,
                ));
            }
        }
        lines.extend(auxiliary);
        let label = self.label();
        let arrow = if expanded {
            "▾"
        } else if self
            .calls
            .last()
            .is_some_and(|call| call.starts_with("Search"))
        {
            "⌕"
        } else {
            "▸"
        };
        TextLayout::new(lines, width)
            .with_disclosure_control_at(format!("{arrow} {label}"), /*source_offset*/ 0)
    }

    fn label(&self) -> String {
        let calls = &self.calls;
        let mut lines = calls.last().into_iter().flat_map(|last| last.lines());
        let mut label = lines
            .next()
            .unwrap_or_default()
            .replace(char::is_control, " ");
        if lines.next().is_some() {
            label.push_str(" …");
        }
        if calls.len() > 1 {
            return format!("{} tool calls · {label}", calls.len());
        }
        label
    }
}

impl TranscriptView {
    pub(super) fn tool_summary(
        &self,
        cell: &dyn HistoryCell,
        key: EntryKey,
    ) -> Option<ToolSummary> {
        if !self.collapse_turn_details || self.detailed || self.mode != HistoryRenderMode::Rich {
            return None;
        }
        let calls = cell.tool_call_labels();
        if calls.is_empty() {
            return None;
        }
        let mut ids: Vec<_> = cell
            .activity_ids()
            .into_iter()
            .map(|id| format!("summary:{id}"))
            .collect();
        if key != EntryKey::Live || ids.is_empty() {
            ids.push(format!("work:{key:?}"));
        }
        Some(ToolSummary { ids, calls })
    }

    pub(crate) fn sync_live_tool_summary(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        enabled: bool,
        active: Option<&dyn HistoryCell>,
    ) -> (Option<ToolSummary>, bool) {
        if self.collapse_turn_details != enabled {
            self.collapse_turn_details = enabled;
            self.selection = None;
            self.select_mode = None;
            self.copy_mode = None;
            self.disclosure.focused = None;
            self.live_key = None;
            self.cache.clear();
            self.release_live_reading();
        }
        let summary = active.and_then(|cell| self.tool_summary(cell, EntryKey::Live));
        let ids = summary.as_ref().map_or_else(
            || active.map_or_else(Vec::new, HistoryCell::activity_ids),
            |summary| summary.ids.clone(),
        );
        let expanded = self.sync_live_activity(cells, ids) && self.mode == HistoryRenderMode::Rich;
        (summary, expanded)
    }
}
