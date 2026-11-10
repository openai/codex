//! Summarize tool invocations on the full-screen conversation surface. The transcript, Find,
//! and scrollback retain their original cells, including command output.

use super::*;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::AgentMessageCell;
use crate::history_cell::DynamicToolCallCell;
use crate::history_cell::FinalMessageSeparator;
use crate::history_cell::ReasoningSummaryCell;
use crate::history_cell::UserHistoryCell;
use crate::history_cell::WebSearchCell;
use codex_protocol::models::MessagePhase;

#[derive(Clone)]
pub(crate) enum TurnRow {
    Prompt { id: String, label: String },
    Tools(Arc<ToolGroup>),
    Hidden,
}

impl TurnRow {
    pub(crate) fn ids(&self) -> &[String] {
        match self {
            Self::Prompt { id, .. } => std::slice::from_ref(id),
            Self::Tools(group) => &group.ids,
            Self::Hidden => &[],
        }
    }

    pub(crate) fn layout(
        &self,
        width: u16,
        expanded: bool,
        auxiliary: Vec<crate::terminal_hyperlinks::HyperlinkLine>,
    ) -> TextLayout {
        let Self::Tools(group) = self else {
            return TextLayout::new(auxiliary, width);
        };
        let mut lines = Vec::new();
        if expanded {
            for call in &group.calls {
                let call = call.replace(char::is_control, " ");
                let line = Line::from(format!("      {call}")).dim();
                lines.push(crate::history_cell::activity_preview::clipped_line(
                    line, width,
                ));
            }
        }
        lines.extend(auxiliary);
        let label = group.label();
        let arrow = if expanded {
            "▾"
        } else if group
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
}

#[derive(Default)]
pub(super) struct TurnDetails {
    // Rows follow canonical cell order, followed by one slot for the live tail.
    rows: Vec<Entry>,
    start: usize,
    source: Option<(usize, Option<EntryKey>)>,
    live: (Vec<String>, Vec<String>),
    pending: Vec<(usize, DynamicToolCallCell)>,
    indexed_snapshot: bool,
}

struct Entry {
    key: EntryKey,
    parent: Option<usize>,
    row: Option<TurnRow>,
}

#[derive(Default)]
pub(crate) struct ToolGroup {
    anchor: Option<usize>,
    ids: Vec<String>,
    calls: Vec<String>,
    explored: usize,
    ran: usize,
}

fn summary_ids(cell: &dyn HistoryCell) -> Vec<String> {
    if let Some(search) = cell.as_any().downcast_ref::<WebSearchCell>() {
        vec![format!("web:{}", search.call_id())]
    } else {
        cell.activity_ids()
    }
}

impl ToolGroup {
    // Walk backwards so completion and phase information are known before folding a row.
    fn push(&mut self, rows: &mut [Entry], index: usize, ids: Vec<String>, calls: Vec<String>) {
        self.anchor = Some(index);
        let key = rows[index].key;
        rows[index].row = Some(TurnRow::Hidden);
        self.ids
            .extend(ids.into_iter().map(|id| format!("summary:{id}")));
        if key != EntryKey::Live {
            self.ids.push(format!("work:{key:?}"));
        }
        self.calls.extend(calls.into_iter().rev());
    }

    fn finish(&mut self, rows: &mut [Entry], cells: &[Arc<dyn HistoryCell>]) {
        if let Some(anchor) = self.anchor {
            let mut group = std::mem::take(self);
            if group.calls.len() == 1
                && cells
                    .get(anchor)
                    .and_then(|cell| cell.as_any().downcast_ref::<WebSearchCell>())
                    .is_some_and(WebSearchCell::is_completed)
            {
                rows[anchor].row = None;
                return;
            }
            if group.ids.is_empty() {
                group.ids.push("work:Live".into());
            }
            group.calls.reverse();
            rows[anchor].row = Some(TurnRow::Tools(Arc::new(group)));
        }
    }

    fn label(&self) -> String {
        let calls = &self.calls;
        let (explored, ran) = (self.explored, self.ran);
        if calls.len() > 1 && explored + ran == calls.len() {
            let noun = if ran == 1 { "command" } else { "commands" };
            return match (explored, ran) {
                (0, _) => format!("Ran {ran} {noun}"),
                (_, 0) => "Explored files".into(),
                _ => format!("Explored files · ran {ran} {noun}"),
            };
        }
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

impl TurnDetails {
    pub(super) fn child_keys(&self, parent: usize) -> impl Iterator<Item = EntryKey> + '_ {
        self.rows
            .iter()
            .filter(move |entry| entry.parent == Some(parent))
            .map(|entry| entry.key)
    }

    fn index_turn(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        latest: bool,
        snapshot: Option<&ViewSnapshot>,
    ) {
        let rows = &mut self.rows;
        let start = rows.len();
        let has_prompt = cells
            .first()
            .is_some_and(|cell| cell.as_any().is::<UserHistoryCell>());
        let keys = cells
            .iter()
            .map(EntryKey::cell)
            .chain(latest.then_some(EntryKey::Live));
        rows.extend(keys.map(|key| Entry {
            key,
            parent: None,
            row: None,
        }));
        let rows = &mut rows[start..];
        let mut group = ToolGroup::default();
        if latest && !self.live.0.is_empty() {
            group.push(rows, cells.len(), self.live.1.clone(), self.live.0.clone());
        }
        let mut completion = None;
        let mut separator = None;
        let mut fold = false;
        let mut hidden_count = 0;
        let mut searches = std::collections::HashSet::new();
        for (index, cell) in cells.iter().enumerate().rev() {
            // Keep older calls outside the group whose disclosure is pinned on screen.
            if group.anchor.is_some_and(|anchor| {
                snapshot
                    .and_then(|snap| snap.pinned.get(&rows[anchor].key))
                    .is_some_and(|layout| layout.disclosure)
            }) {
                group.finish(rows, cells);
            }
            let any = cell.as_any();
            // A live web search can leave its provisional cell before the completed one.
            if let Some(search) = any.downcast_ref::<WebSearchCell>()
                && !searches.insert(search.call_id())
            {
                rows[index].row = Some(TurnRow::Hidden);
                continue;
            }
            let markdown = any.downcast_ref::<AgentMarkdownCell>();
            let phase = markdown.and_then(|message| message.phase.as_ref());
            let calls = cell.tool_call_labels();
            if let Some(done) = any.downcast_ref::<FinalMessageSeparator>() {
                // /review can complete another turn under the same prompt.
                // A live timestamp can arrive immediately after an untimed replay marker.
                if separator.is_none_or(|(next, _)| next != index + 1)
                    || done.display_label().is_some()
                {
                    separator = Some((index, done));
                }
                fold = false;
            } else if phase == Some(&MessagePhase::FinalAnswer)
                || (!fold
                    && separator.is_some()
                    && ((markdown.is_some() && phase.is_none())
                        || any.is::<crate::history_cell::ProposedPlanCell>()))
            {
                // An answer item can finish before its turn succeeds.
                fold |= separator.is_some();
            } else if fold
                && phase != Some(&MessagePhase::PartialAnswer)
                && has_prompt
                && (!calls.is_empty()
                    || markdown.is_some()
                    || any.is::<AgentMessageCell>()
                    || any
                        .downcast_ref::<ReasoningSummaryCell>()
                        .is_some_and(|reasoning| !reasoning.is_transcript_only()))
            {
                rows[index].parent = Some(start);
                hidden_count += calls.len().max(1);
                if let Some((at, done)) = separator.take() {
                    rows[at].row = Some(TurnRow::Hidden);
                    completion.get_or_insert(done);
                }
            }
            if let Some(tool) = any.downcast_ref::<DynamicToolCallCell>()
                && tool.is_active()
            {
                self.pending.push((start + index, tool.clone()));
            }
            if calls.is_empty() {
                group.finish(rows, cells);
            } else {
                if let Some(exec) = any.downcast_ref::<crate::exec_cell::ExecCell>()
                    && exec.iter_calls().all(|call| {
                        call.output
                            .as_ref()
                            .is_some_and(|output| output.exit_code == 0)
                    })
                {
                    if exec.is_exploring_cell() {
                        group.explored += calls.len();
                    } else {
                        group.ran += calls.len();
                    }
                }
                group.push(rows, index, summary_ids(cell.as_ref()), calls);
            }
        }
        group.finish(rows, cells);
        if hidden_count > 0 {
            let label = completion
                .and_then(FinalMessageSeparator::display_label)
                .unwrap_or_else(|| match hidden_count {
                    1 => "1 previous message".into(),
                    count => format!("{count} previous messages"),
                });
            rows[0].row = Some(TurnRow::Prompt {
                id: format!("turn:{:?}", rows[0].key),
                label,
            });
        }
    }
}

impl TranscriptView {
    pub(crate) fn sync_live_tool_summary(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        enabled: bool,
        active: Option<&dyn HistoryCell>,
    ) -> (Option<TurnRow>, bool) {
        self.sync_turn_details(cells, enabled, active);
        let summary = self.turn_row(cells, cells.len());
        let ids = summary.as_ref().map_or_else(
            || active.map_or_else(Vec::new, HistoryCell::activity_ids),
            |row| row.ids().to_vec(),
        );
        let expanded = self.sync_live_activity(cells, ids) && self.mode == HistoryRenderMode::Rich;
        (summary, expanded)
    }

    pub(crate) fn invalidate_turn_details_from(&mut self, index: usize) {
        if let Some(details) = &mut self.turn_details {
            details.source = None;
            if index < details.start {
                details.start = 0;
            }
        }
    }

    pub(crate) fn sync_turn_details(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        enabled: bool,
        active: Option<&dyn HistoryCell>,
    ) {
        if self.turn_details.is_some() != enabled {
            self.selection = None;
            self.select_mode = None;
            self.copy_mode = None;
            self.disclosure.focused = None;
            self.live_key = None;
            self.cache.clear();
            self.release_live_reading();
            self.turn_details = enabled.then(TurnDetails::default);
        }
        let snapshot = self
            .selection
            .as_ref()
            .map(|s| &s.snapshot)
            .or(self.held_reading.as_ref());
        let Some(details) = &mut self.turn_details else {
            return;
        };
        let held = snapshot.map(|snapshot| snapshot.cells.as_ref());
        // Appended calls keep their canonical ids while the snapshot paints its pinned rows.
        let replaced_history = held.filter(|held| {
            cells.get(..held.len()).is_none_or(|prefix| {
                prefix.last().map(EntryKey::cell) != held.last().map(EntryKey::cell)
            })
        });
        if details.indexed_snapshot && held.is_none() {
            details.start = 0;
            details.source = None;
            details.indexed_snapshot = false;
        }
        if replaced_history.is_some() && details.source.is_some() {
            return;
        }
        let cells = replaced_history.unwrap_or(cells);
        let source = (cells.len(), cells.last().map(EntryKey::cell));
        let live = active
            .map(|cell| (cell.tool_call_labels(), summary_ids(cell)))
            .unwrap_or_default();
        let finished_tool = details.pending.iter().any(|(_, tool)| !tool.is_active());
        if details.source == Some(source) && details.live == live && !finished_tool {
            return;
        }
        if replaced_history.is_none() {
            details.live = live;
        }
        let mut start = details.start;
        if finished_tool
            || cells.get(start).map(EntryKey::cell)
                != details.rows.get(start).map(|entry| entry.key)
        {
            start = 0;
        }
        details.rows.truncate(start);
        details.pending.retain(|(index, _)| *index < start);
        let retained_end = held.map(<[_]>::len);
        let prompts = (start + 1..cells.len())
            .filter(|&i| Some(i) == retained_end || cells[i].as_any().is::<UserHistoryCell>());
        for end in prompts.chain([cells.len()]) {
            details.start = start;
            details.index_turn(&cells[start..end], end == cells.len(), snapshot);
            start = end;
        }
        details.indexed_snapshot = held.is_some();
        details.source = Some(source);
        self.cache.clear_summaries();
    }

    pub(crate) fn turn_row(&self, cells: &[Arc<dyn HistoryCell>], index: usize) -> Option<TurnRow> {
        if self.detailed || self.mode != HistoryRenderMode::Rich {
            return None;
        }
        let details = self.turn_details.as_ref()?;
        let key = cells.get(index).map_or(EntryKey::Live, EntryKey::cell);
        let entry = details.rows.get(index).filter(|entry| entry.key == key)?;
        if let Some(parent) = entry.parent
            && let Some(TurnRow::Prompt { id, .. }) = &details.rows[parent].row
            && !self.disclosure.expanded.contains(id)
        {
            return Some(TurnRow::Hidden);
        }
        entry.row.clone()
    }
}
