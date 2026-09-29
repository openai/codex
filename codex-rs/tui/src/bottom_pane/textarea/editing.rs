//! Resolve the replacement range once for context-sensitive edits, then consume it on insertion.
//! Targets respect character and atomic-element boundaries and must be used before another edit
//! or cursor/selection change. Text insertion retains Vim Replace and repeat policies; atomic
//! elements always replace the target literally.

use super::*;

/// The selection to replace, or an empty range at the insertion cursor.
/// Vim Replace without a selection computes its overwrite span as it inserts each grapheme.
#[derive(Debug)]
pub(crate) struct EditTarget {
    range: Range<usize>,
}

impl EditTarget {
    pub(crate) fn start(&self) -> usize {
        self.range.start
    }

    pub(crate) fn replaces_text(&self) -> bool {
        !self.range.is_empty()
    }
}

impl TextArea {
    pub(crate) fn edit_target(&self) -> EditTarget {
        let range = self.mouse_selection_range().map_or_else(
            || {
                let cursor = self.clamp_pos_for_insertion(self.cursor_pos);
                cursor..cursor
            },
            |range| self.expand_range_to_element_boundaries(range),
        );
        EditTarget { range }
    }

    pub fn insert_str(&mut self, text: &str) {
        self.insert_str_at_target(self.edit_target(), text);
    }

    pub(crate) fn insert_str_at_target(&mut self, target: EditTarget, text: &str) {
        // Empty text must not delete a selection or create a Vim replacement step.
        if text.is_empty() {
            if !self.is_vim_replace_mode() {
                self.insert_str_at(self.cursor_pos, text);
            }
            return;
        }
        let replaces_selection = target.replaces_text();
        if replaces_selection {
            // Pointer-selected edits cannot be replayed as keyboard-relative Vim commands.
            self.vim_commands = VimCommandState::default();
        }
        self.record_vim_inserted_text(text);
        if self.is_vim_replace_mode() && !replaces_selection {
            self.replace_vim_text(text);
        } else {
            self.replace_range(target.range, text);
        }
    }

    pub fn insert_element(&mut self, text: &str) -> u64 {
        self.insert_element_at_target(self.edit_target(), text)
    }

    pub(crate) fn insert_element_at_target(&mut self, target: EditTarget, text: &str) -> u64 {
        let start = target.start();
        if target.replaces_text() {
            self.vim_commands = VimCommandState::default();
        }
        self.replace_range(target.range, text);
        let end = start + text.len();
        let id = self.add_element(start..end);
        self.set_cursor(end);
        id
    }
}

#[cfg(test)]
#[path = "editing_tests.rs"]
mod tests;
