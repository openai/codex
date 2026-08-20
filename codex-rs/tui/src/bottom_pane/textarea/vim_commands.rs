//! Pending Vim editing commands.

use super::TextArea;
use super::VimPending;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

impl TextArea {
    pub(super) fn handle_vim_extra_command(&mut self, event: KeyEvent) -> bool {
        if self.vim_normal_keymap.replace_char.is_pressed(event)
            && self.cursor_pos < self.end_of_current_line()
        {
            self.vim_pending = VimPending::Replace;
            return true;
        }
        false
    }

    pub(super) fn handle_vim_pending_command(&mut self, pending: VimPending, event: KeyEvent) {
        match pending {
            VimPending::Replace => {
                if let Some(ch) = vim_command_char(event) {
                    let start = self.cursor_pos;
                    let end = self.next_atomic_boundary(start);
                    self.replace_range(start..end, &ch.to_string());
                    self.set_cursor(start + usize::from(ch == '\n'));
                }
            }
            VimPending::None | VimPending::Operator(_) | VimPending::TextObject { .. } => {}
        }
    }
}

fn vim_command_char(event: KeyEvent) -> Option<char> {
    if event.code == KeyCode::Enter {
        return Some('\n');
    }
    let KeyCode::Char(ch) = event.code else {
        return None;
    };
    match event.modifiers {
        KeyModifiers::NONE => Some(ch),
        KeyModifiers::SHIFT => Some(if ch.is_ascii_lowercase() {
            ch.to_ascii_uppercase()
        } else {
            ch
        }),
        modifiers if crate::key_hint::is_altgr(modifiers) => Some(ch),
        _ => None,
    }
}

#[cfg(test)]
#[path = "vim_commands_tests.rs"]
mod tests;
