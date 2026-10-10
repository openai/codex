//! Refresh configuration panels and coordinate their editor handoffs.

use super::BottomPane;
use crate::config_panel::ConfigPanelUpdate;
use crate::key_hint::KeyBinding;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

impl BottomPane {
    pub(crate) fn update_config_panel(&mut self, update: &ConfigPanelUpdate) -> bool {
        let mut updated = false;
        for view in &mut self.view_stack {
            updated |= view.update_config_panel(update);
        }
        self.request_redraw();
        updated
    }

    pub(crate) fn config_editor_opened(&mut self) {
        for view in &mut self.view_stack {
            view.config_editor_opened();
        }
    }

    pub(crate) fn interrupt_config_editor_transition(&mut self) {
        self.view_transition_key_guard = None;
        for view in &mut self.view_stack {
            view.interrupt_config_editor_transition();
        }
    }

    pub(crate) fn observe_config_editor_transition_key_event(&mut self, key: KeyEvent) {
        match key.kind {
            KeyEventKind::Press | KeyEventKind::Repeat => {
                let activation_key = KeyBinding::from_event(key);
                for view in &mut self.view_stack {
                    view.observe_key_event_while_covered(key, activation_key);
                }
            }
            KeyEventKind::Release => {
                for view in &mut self.view_stack {
                    view.handle_key_release(key);
                }
            }
        }
    }
}
