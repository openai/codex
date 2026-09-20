//! A frozen set of warnings, shown one at a time without changing the retained draft.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell::WarningEntry;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::KeymapContext;
use crate::keymap::KeymapContextSet;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use std::cell::Cell;

#[path = "warnings_view_render.rs"]
mod render;

pub(super) struct WarningsView {
    entries: Vec<WarningEntry>,
    current: usize,
    offset: Cell<usize>,
    page_size: Cell<usize>,
    max_offset: Cell<usize>,
    keymap: RuntimeKeymap,
    tx: AppEventSender,
    pub(super) pending_hint: Option<Vec<(String, String)>>,
    pub(super) flash: Option<(ratatui::text::Line<'static>, std::time::Instant)>,
}

impl WarningsView {
    pub(super) fn new(
        entries: Vec<WarningEntry>,
        keymap: RuntimeKeymap,
        tx: AppEventSender,
    ) -> Self {
        Self {
            entries,
            current: 0,
            offset: Cell::new(/*value*/ 0),
            page_size: Cell::new(/*value*/ 1),
            max_offset: Cell::new(/*value*/ 0),
            keymap,
            tx,
            pending_hint: None,
            flash: None,
        }
    }

    pub(super) fn set_keymap(&mut self, keymap: &RuntimeKeymap) {
        self.keymap = keymap.clone();
    }

    pub(super) fn keymap_contexts(&self) -> KeymapContextSet {
        KeymapContextSet::warnings()
    }

    /// Returns true when the user closes the viewer. No key is forwarded to the draft.
    pub(super) fn handle_key(&mut self, event: KeyEvent) -> bool {
        if event.kind == KeyEventKind::Release {
            return false;
        }
        let action = self
            .keymap
            .list
            .action_for(event)
            .filter(|action| *action != ListAction::Accept);
        if action.is_none() && self.keymap.app.open_warnings.is_pressed(event)
            || self.keymap.list.cancel.is_pressed(event)
        {
            return true;
        }
        if action.is_none() && self.keymap.app.copy.is_pressed(event) {
            if let Some(entry) = self.entries.get(self.current) {
                self.tx.send(AppEvent::CopyWarning(entry.details.clone()));
            }
            return false;
        }
        let offset = self.offset.get();
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
            Some(ListAction::PageUp) => {
                self.offset.set(offset.saturating_sub(self.page_size.get()))
            }
            Some(ListAction::PageDown) => self.offset.set(
                offset
                    .saturating_add(self.page_size.get())
                    .min(self.max_offset.get()),
            ),
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
