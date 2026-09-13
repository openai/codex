//! Availability of the empty-prompt agents shortcut and its footer hint.
//!
//! Only local-daemon sessions enable this shortcut. Draft contents, transient input surfaces,
//! and explicit editor remaps take precedence over navigation.

use super::*;

impl ChatComposer {
    pub(crate) fn set_agents_navigation_enabled(&mut self, enabled: bool) {
        self.agents_navigation_enabled = enabled;
    }

    pub(super) fn agents_navigation_available(&self) -> bool {
        let move_left = if self.draft.textarea.is_vim_normal_mode() {
            &self.vim_normal_keymap.move_left
        } else {
            &self.editor_keymap.move_left
        };
        self.agents_navigation_enabled
            && move_left.is_pressed(KeyCode::Left.into())
            && self.has_focus
            && self.draft.input_enabled
            && self.slash_commands_enabled()
            && self.is_empty()
            && !self.is_in_paste_burst()
            && !self.popup_active()
            && !self.draft.textarea.is_vim_operator_pending()
    }
}
