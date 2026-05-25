use super::super::KillBufferKind;
use super::super::TextArea;
use super::VimRegisterSelection;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;

impl TextArea {
    pub(in super::super) fn handle_vim_register_name(&mut self, event: KeyEvent) -> bool {
        if self.vim_normal_keymap.cancel_operator.is_pressed(event) {
            self.clear_vim_register_selection();
            self.clear_vim_counts();
            return true;
        }
        let KeyCode::Char(name) = event.code else {
            self.clear_vim_register_selection();
            self.clear_vim_counts();
            return true;
        };
        if !name.is_ascii_alphabetic() {
            self.clear_vim_register_selection();
            self.clear_vim_counts();
            return true;
        }
        self.vim_selected_register = Some(VimRegisterSelection {
            name: name.to_ascii_lowercase(),
            append: name.is_ascii_uppercase(),
        });
        true
    }

    pub(in super::super) fn clear_vim_register_selection(&mut self) {
        self.vim_selected_register = None;
    }

    pub(in super::super) fn write_selected_vim_register(
        &mut self,
        text: &str,
        kind: KillBufferKind,
    ) {
        let Some(selection) = self.vim_selected_register.take() else {
            return;
        };
        if selection.append {
            let entry = self
                .vim_named_registers
                .entry(selection.name)
                .or_insert_with(|| (String::new(), kind));
            entry.0.push_str(text);
            entry.1 = kind;
        } else {
            self.vim_named_registers
                .insert(selection.name, (text.to_string(), kind));
        }
    }

    pub(in super::super) fn vim_paste_after_cursor_counted(&mut self, count: usize) {
        let source = if let Some(selection) = self.vim_selected_register.take() {
            self.vim_named_registers
                .get(&selection.name)
                .cloned()
                .unwrap_or_else(|| (String::new(), KillBufferKind::Characterwise))
        } else {
            (self.kill_buffer.clone(), self.kill_buffer_kind)
        };
        if source.0.is_empty() {
            return;
        }
        for _ in 0..count {
            self.paste_text_after_cursor(&source.0, source.1);
        }
    }
}
