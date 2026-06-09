//! Markdown text-event merging that preserves parser-decoded contents and source offsets.

use std::iter::Peekable;
use std::ops::Range;

use pulldown_cmark::Event;

/// Merges adjacent parsed text events without reconstructing them from the Markdown source.
///
/// Markdown extensions can split visually contiguous text around delimiter characters. Keeping the
/// decoded event contents together lets downstream consumers recognize tokens that span those
/// parser boundaries while the combined source range remains available for offset-aware rendering.
pub(crate) struct DecodedTextMerge<I: Iterator> {
    iter: Peekable<I>,
}

impl<I: Iterator> DecodedTextMerge<I> {
    pub(crate) fn new(iter: I) -> Self {
        Self {
            iter: iter.peekable(),
        }
    }
}

impl<'a, I> Iterator for DecodedTextMerge<I>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    type Item = (Event<'a>, Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        let (event, mut range) = self.iter.next()?;
        let Event::Text(text) = event else {
            return Some((event, range));
        };
        if !matches!(self.iter.peek(), Some((Event::Text(_), _))) {
            return Some((Event::Text(text), range));
        }

        let mut merged = text.into_string();
        while matches!(self.iter.peek(), Some((Event::Text(_), _))) {
            let Some((Event::Text(text), next_range)) = self.iter.next() else {
                break;
            };
            merged.push_str(&text);
            range.end = next_range.end;
        }
        Some((Event::Text(merged.into()), range))
    }
}
