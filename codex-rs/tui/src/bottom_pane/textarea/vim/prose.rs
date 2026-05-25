use super::super::TextArea;
use super::VimTextObjectScope;
use std::ops::Range;

impl TextArea {
    pub(in super::super) fn sentence_text_object_range(
        &self,
        scope: VimTextObjectScope,
    ) -> Option<Range<usize>> {
        let inner = self.prose_range_at_cursor(self.sentence_ranges())?;
        Some(self.expand_prose_range(scope, inner))
    }

    pub(in super::super) fn paragraph_text_object_range(
        &self,
        scope: VimTextObjectScope,
    ) -> Option<Range<usize>> {
        let inner = self.prose_range_at_cursor(self.paragraph_ranges())?;
        Some(self.expand_prose_range(scope, inner))
    }

    fn sentence_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let mut start = self.skip_prose_separator(/*pos*/ 0);
        let mut pos = start;
        while pos < self.text.len() {
            let Some(ch) = self.text[pos..].chars().next() else {
                break;
            };
            let next = pos + ch.len_utf8();
            if !self.is_inside_element(pos) && matches!(ch, '.' | '!' | '?') {
                let end = self.sentence_closing_punctuation_end(next);
                if end == self.text.len()
                    || self.text[end..]
                        .chars()
                        .next()
                        .is_some_and(char::is_whitespace)
                {
                    ranges.push(start..end);
                    start = self.skip_prose_separator(end);
                    pos = start;
                    continue;
                }
            }
            pos = next;
        }
        if start < self.text.len() {
            ranges.push(start..self.text.len());
        }
        ranges
    }

    fn sentence_closing_punctuation_end(&self, mut pos: usize) -> usize {
        while pos < self.text.len() && !self.is_inside_element(pos) {
            let Some(ch) = self.text[pos..].chars().next() else {
                break;
            };
            if !matches!(ch, ')' | ']' | '}' | '"' | '\'' | '`') {
                break;
            }
            pos += ch.len_utf8();
        }
        pos
    }

    fn paragraph_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let mut block_start = None;
        let mut block_end = 0;
        let mut line_start = 0;
        loop {
            let line_end = self.text[line_start..]
                .find('\n')
                .map_or(self.text.len(), |offset| line_start + offset);
            if self.line_has_prose_content(line_start..line_end) {
                block_start.get_or_insert(line_start);
                block_end = line_end;
            } else if let Some(start) = block_start.take() {
                ranges.push(start..block_end);
            }
            if line_end == self.text.len() {
                break;
            }
            line_start = line_end + '\n'.len_utf8();
        }
        if let Some(start) = block_start {
            ranges.push(start..block_end);
        }
        ranges
    }

    fn line_has_prose_content(&self, range: Range<usize>) -> bool {
        self.text[range.clone()]
            .char_indices()
            .any(|(offset, ch)| !ch.is_whitespace() || self.is_inside_element(range.start + offset))
    }

    fn prose_range_at_cursor(&self, ranges: Vec<Range<usize>>) -> Option<Range<usize>> {
        ranges
            .iter()
            .find(|range| range.start <= self.cursor_pos && self.cursor_pos <= range.end)
            .cloned()
            .or_else(|| {
                ranges
                    .iter()
                    .find(|range| self.cursor_pos < range.start)
                    .cloned()
            })
            .or_else(|| ranges.last().cloned())
    }

    fn expand_prose_range(&self, scope: VimTextObjectScope, inner: Range<usize>) -> Range<usize> {
        match scope {
            VimTextObjectScope::Inner => inner,
            VimTextObjectScope::Around => {
                let following = self.following_whitespace_end(inner.end);
                if following > inner.end {
                    inner.start..following
                } else {
                    self.preceding_whitespace_start(inner.start)..inner.end
                }
            }
        }
    }

    fn skip_prose_separator(&self, mut pos: usize) -> usize {
        while pos < self.text.len() && !self.is_inside_element(pos) {
            let Some(ch) = self.text[pos..].chars().next() else {
                break;
            };
            if !ch.is_whitespace() {
                break;
            }
            pos += ch.len_utf8();
        }
        pos
    }
}
