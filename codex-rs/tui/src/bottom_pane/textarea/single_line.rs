//! Single-line editing strips line breaks and keeps a grapheme-aligned cursor viewport.

use super::*;

impl TextArea {
    pub(crate) fn new_single_line() -> Self {
        Self {
            single_line: true,
            ..Self::new()
        }
    }

    pub(super) fn filter_line_breaks<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.single_line && text.contains(['\r', '\n']) {
            Cow::Owned(text.replace(['\r', '\n'], ""))
        } else {
            Cow::Borrowed(text)
        }
    }

    pub(super) fn single_line_viewport(&self, width: u16) -> (usize, u16) {
        let cursor_width = self.text[self.cursor_pos..]
            .graphemes(/*is_extended*/ true)
            .next()
            .map_or(/*default*/ 1, display_width)
            .max(/*other*/ 1)
            .min(usize::from(width));
        let mut start = self.cursor_pos;
        let mut column = 0;
        for (offset, grapheme) in self.text[..self.cursor_pos]
            .grapheme_indices(/*is_extended*/ true)
            .rev()
        {
            let grapheme_width = display_width(grapheme);
            if column + grapheme_width > usize::from(width).saturating_sub(cursor_width) {
                break;
            }
            start = offset;
            column += grapheme_width;
        }
        (start, column as u16)
    }
}

#[cfg(test)]
#[path = "single_line_tests.rs"]
mod tests;
