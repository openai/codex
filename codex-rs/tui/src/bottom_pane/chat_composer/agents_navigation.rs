//! Availability of the empty-prompt agents shortcut and its footer hint.
//!
//! Sessions connected to a local daemon or remote App Server enable this shortcut.
//! Draft contents, transient input surfaces, and explicit editor remaps take precedence.

use super::*;

impl ChatComposer {
    pub(crate) fn agents_navigation_key_available(&self) -> bool {
        let move_left = if self.draft.textarea.is_vim_normal_mode() {
            &self.vim_normal_keymap.move_left
        } else {
            &self.editor_keymap.move_left
        };
        move_left.is_pressed(KeyCode::Left.into())
    }

    pub(super) fn agents_navigation_available(&self) -> bool {
        self.command_center_enabled
            && self.agents_navigation_key_available()
            && self.has_focus
            && self.draft.input_enabled
            && self.slash_commands_enabled()
            && self.is_empty()
            && !self.is_in_paste_burst()
            && !self.popup_active()
            && !self.draft.textarea.is_vim_operator_pending()
    }

    pub(super) fn handle_agents_navigation(&mut self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Press
            && key_hint::plain(KeyCode::Left).is_press(key)
            && self.agents_navigation_available()
        {
            self.app_event_tx.send(AppEvent::OpenAgentsOverview);
            return true;
        }
        false
    }
}
