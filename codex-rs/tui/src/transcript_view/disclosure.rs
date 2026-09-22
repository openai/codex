//! Local activity disclosure keyed by retained tool identities, independent of text selection.
//! Controls appear only while activity is focused; copied text and full-transcript search remain source based.

use crate::style::accent_color;
use std::collections::HashSet;

use crate::bottom_pane::TranscriptFooter;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyEvent;
use ratatui::style::Modifier;
use ratatui::style::Style;

use super::*;

pub(super) struct Disclosure {
    pub(super) expanded: HashSet<String>,
    pub(super) focused: Option<Vec<String>>,
    pub(super) live_ids: Vec<String>,
    pub(super) keymap: RuntimeKeymap,
}

impl Default for Disclosure {
    fn default() -> Self {
        Self {
            expanded: HashSet::new(),
            focused: None,
            live_ids: Vec::new(),
            keymap: RuntimeKeymap::defaults(),
        }
    }
}

impl Disclosure {
    pub(super) fn is_expanded(&self, ids: &[String]) -> bool {
        ids.iter().any(|id| self.expanded.contains(id))
    }
}

impl TranscriptView {
    pub(crate) fn is_activity_focused(&self) -> bool {
        self.disclosure.focused.is_some() && !self.detailed && self.mode == HistoryRenderMode::Rich
    }

    pub(crate) fn clear_activity_focus(&mut self) {
        self.disclosure.focused = None;
    }

    /// Live member identities survive the transition from active widget to committed history.
    pub(crate) fn sync_live_activity(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        ids: Vec<String>,
    ) -> bool {
        // Disclosure follows its activity into history instead of an unrelated next live tail.
        if self.snapshot().is_none()
            && let Position::Reading(anchor) = self.position
            && anchor.key == EntryKey::Live
            && let Some(index) = self.canonical_activity_index(cells, &self.disclosure.live_ids)
            && let Some(cell) = cells.get(index)
        {
            self.position = Position::Reading(Anchor {
                key: EntryKey::cell(cell),
                index,
                offset: 0,
                row_bias: 0,
            });
        }
        if self.disclosure.live_ids != ids {
            self.live_key = None;
        }
        let expanded = self.disclosure.is_expanded(&ids);
        if expanded {
            self.disclosure.expanded.extend(ids.iter().cloned());
        }
        self.disclosure.live_ids = ids;
        expanded
    }

    pub(super) fn activity_ids(&self, cells: &[Arc<dyn HistoryCell>], index: usize) -> Vec<String> {
        cells.get(index).map_or_else(
            || self.disclosure.live_ids.clone(),
            |cell| cell.activity_ids(),
        )
    }

    /// Retired live entries and replaced groups keep the identities of their displayed revision.
    pub(super) fn displayed_activity_ids(
        &self,
        cells: &[Arc<dyn HistoryCell>],
        index: usize,
    ) -> Arc<[String]> {
        let key = cells.get(index).map_or(EntryKey::Live, EntryKey::cell);
        self.snapshot()
            .and_then(|snapshot| snapshot.activities.get(&key))
            .cloned()
            .unwrap_or_else(|| self.activity_ids(cells, index).into())
    }

    pub(super) fn render_disclosure(
        &self,
        ids: &[String],
        layout: &TextLayout,
        row: usize,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let focused = self
            .disclosure
            .focused
            .as_ref()
            .is_some_and(|focused| ids.iter().any(|id| focused.contains(id)));
        if !focused || !layout.disclosure {
            return;
        }
        if layout.disclosure_row() == Some(row) {
            let columns = layout.disclosure_columns();
            buf.set_style(
                Rect::new(
                    area.x + columns.start,
                    area.y,
                    columns.end - columns.start,
                    /*height*/ 1,
                ),
                Style::default()
                    .fg(accent_color())
                    .bold()
                    .remove_modifier(Modifier::DIM),
            );
        } else if row == usize::from(layout.separated) {
            buf.set_style(area, Style::default().underlined());
        }
    }

    pub(super) fn toggle_disclosure_at(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        column: u16,
        row: u16,
    ) -> bool {
        if self.detailed || self.mode != HistoryRenderMode::Rich {
            return false;
        }
        let Some(visible) = row
            .checked_sub(self.area.y)
            .and_then(|row| self.visible.get(usize::from(row)))
        else {
            return false;
        };
        if visible.layout.disclosure_row() != Some(visible.row)
            || column
                .checked_sub(self.area.x)
                .is_none_or(|column| !visible.layout.disclosure_columns().contains(&column))
        {
            return false;
        }
        let Some(index) = self.canonical_activity_index(cells, &visible.activity_ids) else {
            return false;
        };
        self.toggle_activity(cells, index);
        true
    }

    pub(super) fn handle_disclosure_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        if self.detailed || self.mode != HistoryRenderMode::Rich || self.search.is_active() {
            return None;
        }
        if self.disclosure.keymap.app.focus_activity.is_pressed(key) {
            if self.is_activity_focused() {
                self.disclosure.focused = None;
            } else {
                self.end_selection(cells);
                let visible = self
                    .visible
                    .iter()
                    .rev()
                    .find_map(|row| self.canonical_activity_index(cells, &row.activity_ids));
                for index in visible.into_iter().chain((0..=cells.len()).rev()) {
                    if !self.activity_ids(cells, index).is_empty()
                        && self.focus_activity(cells, index)
                    {
                        break;
                    }
                }
            }
            return Some(ViewAction::Changed);
        }
        let focused = self
            .disclosure
            .focused
            .as_ref()
            .and_then(|ids| self.canonical_activity_index(cells, ids));
        let Some(focused) = focused else {
            self.disclosure.focused = None;
            return None;
        };
        let Some(action) = self.disclosure.keymap.list.action_for(key) else {
            // Ordinary text resumes composing; activity navigation never becomes a text editor.
            self.disclosure.focused = None;
            return None;
        };
        match action {
            ListAction::Cancel => self.disclosure.focused = None,
            ListAction::Accept => self.toggle_activity(cells, focused),
            ListAction::MoveLeft | ListAction::MoveRight => {
                let ids = self.activity_ids(cells, focused);
                if self.disclosure.is_expanded(&ids) != (action == ListAction::MoveRight) {
                    self.toggle_activity(cells, focused);
                }
            }
            ListAction::MoveUp
            | ListAction::MoveDown
            | ListAction::PageUp
            | ListAction::PageDown
            | ListAction::JumpTop
            | ListAction::JumpBottom => {
                let (mut range, backwards, steps) = match action {
                    ListAction::MoveUp => (0..focused, true, 1),
                    ListAction::PageUp => (0..focused, true, 5),
                    ListAction::JumpTop => (0..cells.len() + 1, false, 1),
                    ListAction::MoveDown => (focused + 1..cells.len() + 1, false, 1),
                    ListAction::PageDown => (focused + 1..cells.len() + 1, false, 5),
                    ListAction::JumpBottom => (0..cells.len() + 1, true, 1),
                    ListAction::MoveLeft
                    | ListAction::MoveRight
                    | ListAction::Accept
                    | ListAction::Cancel => unreachable!(),
                };
                let mut next = focused;
                for _ in 0..steps {
                    let candidate = if backwards {
                        range
                            .by_ref()
                            .rev()
                            .find(|index| !self.activity_ids(cells, *index).is_empty())
                    } else {
                        range
                            .by_ref()
                            .find(|index| !self.activity_ids(cells, *index).is_empty())
                    };
                    let Some(index) = candidate else { break };
                    next = index;
                }
                loop {
                    if self.focus_activity(cells, next) {
                        break;
                    }
                    let candidate = if backwards {
                        range.next_back()
                    } else {
                        range.next()
                    };
                    let Some(index) = candidate else { break };
                    next = index;
                }
            }
        }
        Some(ViewAction::Changed)
    }

    pub(super) fn canonical_activity_index(
        &self,
        cells: &[Arc<dyn HistoryCell>],
        ids: &[String],
    ) -> Option<usize> {
        if ids.is_empty() {
            return None;
        }
        // A visible immutable cell resolves directly; replaced groups fall back to member IDs.
        if let Some(row) = self.visible.iter().find(|row| {
            row.index <= cells.len()
                && row.activity_ids.iter().any(|id| ids.contains(id))
                && cells.get(row.index).map_or(EntryKey::Live, EntryKey::cell) == row.key
                && self
                    .activity_ids(cells, row.index)
                    .iter()
                    .any(|id| ids.contains(id))
        }) {
            return Some(row.index);
        }
        (0..=cells.len()).find(|index| {
            self.activity_ids(cells, *index)
                .iter()
                .any(|id| ids.contains(id))
        })
    }

    fn focus_activity(&mut self, cells: &[Arc<dyn HistoryCell>], index: usize) -> bool {
        if self
            .current_layout(cells, index)
            .is_none_or(|layout| !layout.disclosure)
        {
            return false;
        }
        let ids = self.activity_ids(cells, index);
        let header_visible = self.visible.iter().any(|row| {
            row.layout.disclosure
                && row.activity_ids.iter().any(|id| ids.contains(id))
                && row.row == usize::from(row.layout.separated)
        });
        self.disclosure.focused = Some(ids);
        if !header_visible {
            self.jump_to_entry(cells, index);
        }
        true
    }

    fn toggle_activity(&mut self, cells: &[Arc<dyn HistoryCell>], index: usize) {
        let ids = self.activity_ids(cells, index);
        if ids.is_empty() {
            return;
        }
        let previous_top = self.visible.first().map(|visible| {
            let offset = visible.layout.position_at(visible.row, /*column*/ 0);
            (
                visible.key,
                Arc::clone(&visible.activity_ids),
                offset,
                visible.layout.row_for_offset(offset) as isize - visible.row as isize,
            )
        });
        self.end_selection(cells);
        self.release_live_reading();
        // Source offsets only survive an unchanged cell. Regrouped/retired live entries resolve
        // through their member IDs and anchor at their header, never a stale positional index.
        let anchor = previous_top
            .and_then(|(key, top_ids, offset, row_bias)| {
                let unchanged = cells.iter().position(|cell| EntryKey::cell(cell) == key);
                let top = unchanged.or_else(|| self.canonical_activity_index(cells, &top_ids))?;
                let preserve_offset = unchanged.is_some() && top != index;
                Some(Anchor {
                    key: cells.get(top).map_or(EntryKey::Live, EntryKey::cell),
                    index: top,
                    offset: if preserve_offset { offset } else { 0 },
                    row_bias: if preserve_offset { row_bias } else { 0 },
                })
            })
            .unwrap_or(Anchor {
                key: cells.get(index).map_or(EntryKey::Live, EntryKey::cell),
                index,
                offset: 0,
                row_bias: 0,
            });
        if self.disclosure.is_expanded(&ids) {
            self.disclosure.expanded.retain(|id| !ids.contains(id));
        } else {
            self.disclosure.expanded.extend(ids.iter().cloned());
        }
        self.disclosure.focused = Some(ids);
        self.position = Position::Reading(anchor);
        self.cache.clear();
        self.live_key = None;
        self.live_separated = None;
        self.restart_search();
    }

    pub(super) fn disclosure_footer(&self, width: u16) -> Option<TranscriptFooter> {
        if !self.is_activity_focused() {
            return None;
        }
        let keymap = &self.disclosure.keymap;
        let hints: Vec<_> = [
            (ListAction::MoveUp, "previous"),
            (ListAction::MoveDown, "next"),
            (ListAction::Accept, "details"),
            (ListAction::Cancel, "back"),
        ]
        .into_iter()
        .filter_map(|(action, label)| {
            keymap.list.primary_hint(action).map(|key| {
                let mut spans = key.spans();
                spans.push(format!(" {label}").dim());
                Line::from(spans)
            })
        })
        .collect();
        // Drop navigation before the details/back controls, retaining whole remapped chords.
        let text = crate::footer_hint::first_fitting_line(
            (0..hints.len()).map(|start| {
                let mut spans = Vec::new();
                for (index, hint) in hints[start..].iter().enumerate() {
                    if index > 0 {
                        spans.push(" · ".dim());
                    }
                    spans.extend(hint.spans.clone());
                }
                Line::from(spans)
            }),
            width,
        );
        Some(TranscriptFooter {
            text: text.into(),
            cursor_column: None,
            is_interactive: true,
        })
    }
}
