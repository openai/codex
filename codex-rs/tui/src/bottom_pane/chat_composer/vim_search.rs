//! Composer UI bridge for textarea-owned Vim queries, independent of prompt-history recall.

use super::ChatComposer;
use super::inset_footer_hint_area;
use ratatui::layout::Rect;

impl ChatComposer {
    pub(crate) fn cancel_vim_search(&mut self) -> bool {
        let canceled = self.draft.textarea.cancel_vim_search();
        if canceled {
            self.finish_vim_edit();
        }
        canceled
    }

    pub(super) fn vim_search_cursor_pos(&self, mut footer: Rect) -> Option<(u16, u16)> {
        let query = self.draft.textarea.vim_query()?;
        footer.y = footer.bottom().saturating_sub(1);
        footer.height = footer.height.min(1);
        query.cursor_pos(inset_footer_hint_area(footer))
    }
}

#[cfg(test)]
#[path = "vim_search_tests.rs"]
mod tests;
