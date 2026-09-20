//! Tab filters and shortcut help yield to configured task and list bindings.

use super::*;
use crossterm::event::KeyEventKind;

impl AgentsOverviewView {
    pub(in crate::app::agents_overview_view) fn command_center_key(
        &mut self,
        key: KeyEvent,
    ) -> bool {
        if self.state().editing_metadata() {
            if self.keymap.action_for(key) == Some(ListAction::Cancel) {
                self.on_ctrl_c();
                return true;
            }
            return false;
        }
        let mut state = self.state();
        if state.help && self.keymap.action_for(key) == Some(ListAction::Cancel) {
            state.help = false;
            return true;
        }
        if state.help {
            if key.code == KeyCode::Char('?')
                && key.kind == KeyEventKind::Press
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            {
                state.help = false;
            }
            return true;
        }
        if self.center_shortcut_keys.is_pressed(key) {
            return false;
        }
        if key.code == KeyCode::Char('?')
            && key.kind == KeyEventKind::Press
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            state.help = !state.help;
            return true;
        }
        if key.code == KeyCode::Tab
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            let step = if key.modifiers.contains(KeyModifiers::SHIFT) {
                TASK_FILTERS.len() - 1
            } else {
                1
            };
            state.status_filter = (state.status_filter + step) % TASK_FILTERS.len();
            state.scroll = 0;
            drop(state);
            self.reconcile_command_center_selection();
            return true;
        }
        false
    }
}
