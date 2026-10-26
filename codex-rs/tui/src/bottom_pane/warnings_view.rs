//! A frozen set of warnings, shown one at a time without changing the retained draft.
//! On intentional close, dismiss only warnings actually drawn, unless the user kept them.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell::WarningEntry;
use crate::key_hint;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::KeymapContext;
use crate::keymap::KeymapContextSet;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::MouseEvent;
use ratatui::layout::Rect;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::sync::Arc;

#[path = "warnings_view_render.rs"]
mod render;

pub(crate) struct WarningsView {
    entries: Vec<WarningEntry>,
    transcript: Arc<()>,
    current: usize,
    visited: Vec<Cell<bool>>,
    kept: BTreeSet<usize>,
    offset: Cell<usize>,
    max_offset: Cell<usize>,
    pub(crate) body_area: Cell<Rect>,
    pub(crate) body_layout: Cell<Option<(crate::transcript_view::TextLayout, usize)>>,
    keymap: RuntimeKeymap,
    tx: AppEventSender,
    pub(super) pending_hint: Option<Vec<(String, String)>>,
    pub(super) flash: Option<(ratatui::text::Line<'static>, std::time::Instant)>,
    /// Display only: copying or dismissing a warning must use its original diagnostic.
    pub(super) daemon_restart_hint_at: Option<usize>,
}

impl WarningsView {
    pub(super) fn new(
        entries: Vec<WarningEntry>,
        transcript: Arc<()>,
        keymap: RuntimeKeymap,
        tx: AppEventSender,
    ) -> Self {
        Self {
            visited: vec![Cell::new(/*value*/ false); entries.len()],
            entries,
            transcript,
            current: 0,
            kept: BTreeSet::new(),
            offset: Cell::new(/*value*/ 0),
            max_offset: Cell::new(/*value*/ 0),
            body_area: Cell::default(),
            body_layout: Cell::default(),
            keymap,
            tx,
            pending_hint: None,
            flash: None,
            daemon_restart_hint_at: None,
        }
    }

    pub(super) fn set_keymap(&mut self, keymap: &RuntimeKeymap) {
        self.keymap = keymap.clone();
    }

    pub(super) fn keymap_contexts(&self) -> KeymapContextSet {
        KeymapContextSet::warnings()
    }

    pub(super) fn close(self) {
        let mut dismissed = Vec::new();
        let mut kept = Vec::new();
        for (index, (entry, visited)) in self.entries.into_iter().zip(self.visited).enumerate() {
            if self.kept.contains(&index) {
                kept.push(entry);
            } else if visited.get() {
                dismissed.push(entry);
            }
        }
        if !dismissed.is_empty() || !kept.is_empty() {
            self.tx.send(AppEvent::UpdateWarnings {
                transcript: self.transcript,
                dismissed,
                kept,
            });
        }
    }

    pub(crate) fn scroll_mouse(&self, event: MouseEvent) -> bool {
        let Some(rows) = super::scroll_state::wheel_rows_in(event, self.body_area.get()) else {
            return false;
        };
        if self.max_offset.get() == 0 {
            return false;
        }
        self.offset.set(
            self.offset
                .get()
                .saturating_add_signed(isize::from(rows))
                .min(self.max_offset.get()),
        );
        true
    }

    /// Returns true when the user closes the viewer. No key is forwarded to the draft.
    pub(super) fn handle_key(&mut self, event: KeyEvent) -> bool {
        if event.kind == KeyEventKind::Release {
            return false;
        }
        let action = if key_hint::plain(KeyCode::Char('k')).is_press(event) {
            if event.kind != KeyEventKind::Press {
                return false;
            }
            self.kept.insert(self.current);
            if self.current + 1 >= self.entries.len() {
                return true;
            }
            Some(ListAction::MoveRight)
        } else {
            self.keymap
                .list
                .action_for(event)
                .filter(|action| *action != ListAction::Accept)
        };
        if action.is_none() && self.keymap.app.open_warnings.is_pressed(event) {
            return true;
        }
        if action.is_none() && self.keymap.app.copy.is_pressed(event) {
            if let Some(entry) = self.entries.get(self.current) {
                self.tx.send(AppEvent::CopyWarning(entry.details.clone()));
            }
            return false;
        }
        let offset = self.offset.get();
        let page_size = usize::from(self.body_area.get().height.max(/*other*/ 1));
        match action {
            Some(ListAction::MoveLeft | ListAction::MoveRight) => {
                let previous = self.current;
                self.current = if action == Some(ListAction::MoveLeft) {
                    self.current.saturating_sub(/*rhs*/ 1)
                } else {
                    (self.current + 1).min(self.entries.len().saturating_sub(/*rhs*/ 1))
                };
                if self.current != previous {
                    self.offset.set(/*val*/ 0);
                }
            }
            Some(ListAction::MoveUp) => self.offset.set(offset.saturating_sub(/*rhs*/ 1)),
            Some(ListAction::MoveDown) => self.offset.set((offset + 1).min(self.max_offset.get())),
            Some(ListAction::PageUp) => self.offset.set(offset.saturating_sub(page_size)),
            Some(ListAction::PageDown) => self
                .offset
                .set(offset.saturating_add(page_size).min(self.max_offset.get())),
            Some(ListAction::JumpTop) => self.offset.set(/*val*/ 0),
            Some(ListAction::JumpBottom) => self.offset.set(self.max_offset.get()),
            Some(ListAction::Cancel) => return true,
            Some(ListAction::Accept) | None => {}
        }
        false
    }
}

#[cfg(test)]
#[path = "warnings_view_tests.rs"]
mod tests;
