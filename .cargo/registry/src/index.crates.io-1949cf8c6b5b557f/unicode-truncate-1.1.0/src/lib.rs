// Copyright 2019 Aetf <aetf at unlimitedcodeworks dot xyz>.
// See the COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![forbid(missing_docs, unsafe_code)]
#![warn(clippy::arithmetic_side_effects)]
#![cfg_attr(not(feature = "std"), no_std)]

//! Unicode-aware algorithm to pad or truncate `str` in terms of displayed width.
//!
//! See the [`UnicodeTruncateStr`](crate::UnicodeTruncateStr) trait for new methods available on
//! `str`.
//!
//! # Examples
//! Safely truncate string to display width even not at character boundaries.
//! ```rust
//! use unicode_truncate::UnicodeTruncateStr;
//! assert_eq!("你好吗".unicode_truncate(5), ("你好", 4));
//! ```
#![cfg_attr(
    feature = "std",
    doc = r##"
Making sure the string is displayed in exactly number of columns by
combining padding and truncating.

```rust
use unicode_truncate::UnicodeTruncateStr;
use unicode_truncate::Alignment;
use unicode_width::UnicodeWidthStr;

let str = "你好吗".unicode_pad(5, Alignment::Left, true);
assert_eq!(str, "你好 ");
assert_eq!(str.width(), 5);
```
"##
)]

use itertools::{merge_join_by, Either};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Defines the alignment for truncation and padding.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum Alignment {
    /// Align to the left
    Left,
    /// Align center
    Center,
    /// Align to the right
    Right,
}

/// Methods for padding or truncating using displayed width of Unicode strings.
pub trait UnicodeTruncateStr {
    /// Truncates a string to be at most `width` in terms of display width by removing the end
    /// characters.
    ///
    /// For wide characters, it may not always be possible to truncate at exact width. In this case,
    /// the longest possible string is returned. To help the caller determine the situation, the
    /// display width of the returned string slice is also returned.
    ///
    /// Zero-width characters decided by [`unicode_width`] are always included when deciding the
    /// truncation point.
    ///
    /// # Arguments
    /// * `max_width` - the maximum display width
    fn unicode_truncate(&self, max_width: usize) -> (&str, usize);

    /// Truncates a string to be at most `width` in terms of display width by removing the start
    /// characters.
    ///
    /// For wide characters, it may not always be possible to truncate at exact width. In this case,
    /// the longest possible string is returned. To help the caller determine the situation, the
    /// display width of the returned string slice is also returned.
    ///
    /// Zero-width characters decided by [`unicode_width`] are always removed when deciding the
    /// truncation point.
    ///
    /// # Arguments
    /// * `max_width` - the maximum display width
    fn unicode_truncate_start(&self, max_width: usize) -> (&str, usize);

    /// Truncates a string to be at most `width` in terms of display width by removing
    /// characters at both start and end.
    ///
    /// For wide characters, it may not always be possible to truncate at exact width. In this case,
    /// the longest possible string is returned. To help the caller determine the situation, the
    /// display width of the returned string slice is also returned.
    ///
    /// Zero-width characters decided by [`unicode_width`] are included if they are at end, or
    /// removed if they are at the beginning when deciding the truncation point.
    ///
    /// # Arguments
    /// * `max_width` - the maximum display width
    fn unicode_truncate_centered(&self, max_width: usize) -> (&str, usize);

    /// Truncates a string to be at most `width` in terms of display width by removing
    /// characters.
    ///
    /// Depending on the alignment characters are removed. When left aligned characters from the end
    /// are removed. When right aligned characters from the start are removed. When centered
    /// characters from both sides are removed.
    ///
    /// For wide characters, it may not always be possible to truncate at exact width. In this case,
    /// the longest possible string is returned. To help the caller determine the situation, the
    /// display width of the returned string slice is also returned.
    ///
    /// Zero-width characters decided by [`unicode_width`] are included if they are at end, or
    /// removed if they are at the beginning when deciding the truncation point.
    ///
    /// # Arguments
    /// * `max_width` - the maximum display width
    /// * `align` - alignment for truncation
    #[inline]
    fn unicode_truncate_aligned(&self, max_width: usize, align: Alignment) -> (&str, usize) {
        match align {
            Alignment::Left => self.unicode_truncate(max_width),
            Alignment::Center => self.unicode_truncate_centered(max_width),
            Alignment::Right => self.unicode_truncate_start(max_width),
        }
    }

    /// Pads a string to be `width` in terms of display width. Only available when the `std` feature
    /// of this library is activated, and it is activated by default.
    ///
    /// When `truncate` is true, the string is truncated to `width` if necessary. In case of wide
    /// characters and truncation point not at character boundary, the longest possible string is
    /// used, and padded to exact `width` according to `align`.
    /// See [`unicode_truncate`](crate::UnicodeTruncateStr::unicode_truncate) for the behavior of
    /// truncation.
    ///
    /// # Arguments
    /// * `target_width` - the display width to pad to
    /// * `align` - alignment for truncation and padding
    /// * `truncate` - whether to truncate string if necessary
    #[cfg(feature = "std")]
    fn unicode_pad(
        &self,
        target_width: usize,
        align: Alignment,
        truncate: bool,
    ) -> std::borrow::Cow<'_, str>;
}

impl UnicodeTruncateStr for str {
    #[inline]
    fn unicode_truncate(&self, max_width: usize) -> (&str, usize) {
        let (byte_index, new_width) = self
            .grapheme_indices(true)
            // map to byte index and the width of grapheme at the index
            .map(|(byte_index, grapheme)| (byte_index, grapheme.width()))
            // chain a final element representing the position past the last char
            .chain(core::iter::once((self.len(), 0)))
            // fold to byte index and the width up to the index
            .scan(0, |sum: &mut usize, (byte_index, grapheme_width)| {
                // byte_index is the start while the grapheme_width is at the end. Current width is
                // the sum until now while the next byte_index is including the current
                // grapheme_width.
                let current_width = *sum;
                *sum = sum.checked_add(grapheme_width)?;
                Some((byte_index, current_width))
            })
            // take the longest but still shorter than requested
            .take_while(|&(_, current_width)| current_width <= max_width)
            .last()
            .unwrap_or((0, 0));

        // unwrap is safe as the index comes from grapheme_indices
        let result = self.get(..byte_index).unwrap();
        debug_assert_eq!(result.width(), new_width);
        (result, new_width)
    }

    #[inline]
    fn unicode_truncate_start(&self, max_width: usize) -> (&str, usize) {
        let (byte_index, new_width) = self
            .grapheme_indices(true)
            // instead of start checking from the start do so from the end
            .rev()
            // map to byte index and the width of grapheme start at the index
            .map(|(byte_index, grapheme)| (byte_index, grapheme.width()))
            // fold to byte index and the width from end to the index
            .scan(0, |sum: &mut usize, (byte_index, grapheme_width)| {
                *sum = sum.checked_add(grapheme_width)?;
                Some((byte_index, *sum))
            })
            .take_while(|&(_, current_width)| current_width <= max_width)
            .last()
            .unwrap_or((self.len(), 0));

        // unwrap is safe as the index comes from grapheme_indices
        let result = self.get(byte_index..).unwrap();
        debug_assert_eq!(result.width(), new_width);
        (result, new_width)
    }

    #[inline]
    fn unicode_truncate_centered(&self, max_width: usize) -> (&str, usize) {
        if max_width == 0 {
            return ("", 0);
        }

        let original_width = self.width();
        if original_width <= max_width {
            return (self, original_width);
        }

        // We need to remove at least this much
        // unwrap is safe as original_width > max_width
        let min_removal_width = original_width.checked_sub(max_width).unwrap();

        // Around the half to improve performance. In order to ensure the center grapheme stays
        // remove its max possible length. This assumes a grapheme width is always <= 10 (4 people
        // family emoji has width 8). This might end up not perfect on graphemes wider than this but
        // performance is more important here.
        let less_than_half = min_removal_width.saturating_sub(10) / 2;

        let from_start = self
            .grapheme_indices(true)
            .map(|(byte_index, grapheme)| (byte_index, grapheme.width()))
            // fold to byte index and the width from start to the index (not including the current
            // grapheme width)
            .scan(
                (0usize, 0usize),
                |(sum, prev_width), (byte_index, grapheme_width)| {
                    *sum = sum.checked_add(*prev_width)?;
                    *prev_width = grapheme_width;
                    Some((byte_index, *sum))
                },
            )
            // fast forward to around the half
            .skip_while(|&(_, removed)| removed < less_than_half);

        let from_end = self
            .grapheme_indices(true)
            .map(|(byte_index, grapheme)| (byte_index, grapheme.width()))
            .rev()
            // fold to byte index and the width from end to the index (including the current
            // grapheme width)
            .scan(0usize, |sum, (byte_index, grapheme_width)| {
                *sum = sum.checked_add(grapheme_width)?;
                Some((byte_index, *sum))
            })
            // fast forward to around the half
            .skip_while(|&(_, removed)| removed < less_than_half);

        let (start_index, end_index, removed_width) = merge_join_by(
            from_start,
            from_end,
            // taking from either left or right iter depending on which side has less removed width
            |&(_, start_removed), &(_, end_removed)| start_removed < end_removed,
        )
        // remember the last left or right and combine them to one sequence of operations
        .scan(
            (0usize, 0usize, 0usize, 0usize),
            |(start_removed, end_removed, start_index, end_index), position| {
                match position {
                    Either::Left((idx, removed)) => {
                        *start_index = idx;
                        *start_removed = removed;
                    }
                    Either::Right((idx, removed)) => {
                        *end_index = idx;
                        *end_removed = removed;
                    }
                }
                // unwrap is safe as total length was also <= usize::MAX
                let total_removed = start_removed.checked_add(*end_removed).unwrap();
                Some((*start_index, *end_index, total_removed))
            },
        )
        .find(|&(_, _, removed)| removed >= min_removal_width)
        // should not happen as the removed width is not larger than the original width
        // but a sane default is to remove everything (i.e. min_removal_width too large)
        .unwrap_or((0, 0, original_width));

        // unwrap is safe as the index comes from grapheme_indices
        let result = self.get(start_index..end_index).unwrap();
        // unwrap is safe as removed is always smaller than total width
        let result_width = original_width.checked_sub(removed_width).unwrap();
        debug_assert_eq!(result.width(), result_width);
        (result, result_width)
    }

    #[cfg(feature = "std")]
    #[inline]
    fn unicode_pad(
        &self,
        target_width: usize,
        align: Alignment,
        truncate: bool,
    ) -> std::borrow::Cow<'_, str> {
        use std::borrow::Cow;

        if !truncate && self.width() >= target_width {
            return Cow::Borrowed(self);
        }

        let (truncated, columns) = self.unicode_truncate(target_width);
        if columns == target_width {
            return Cow::Borrowed(truncated);
        }

        // the string is less than width, or truncated to less than width
        let diff = target_width.saturating_sub(columns);
        let (left_pad, right_pad) = match align {
            Alignment::Left => (0, diff),
            Alignment::Right => (diff, 0),
            Alignment::Center => (diff / 2, diff.saturating_sub(diff / 2)),
        };
        debug_assert_eq!(diff, left_pad.saturating_add(right_pad));

        let new_len = truncated
            .len()
            .checked_add(diff)
            .expect("Padded result should fit in a new String");
        let mut result = String::with_capacity(new_len);
        for _ in 0..left_pad {
            result.push(' ');
        }
        result += truncated;
        for _ in 0..right_pad {
            result.push(' ');
        }
        Cow::Owned(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod truncate_end {
        use super::*;

        #[test]
        fn empty() {
            assert_eq!("".unicode_truncate(4), ("", 0));
        }

        #[test]
        fn zero_width() {
            assert_eq!("ab".unicode_truncate(0), ("", 0));
            assert_eq!("你好".unicode_truncate(0), ("", 0));
        }

        #[test]
        fn less_than_limit() {
            assert_eq!("abc".unicode_truncate(4), ("abc", 3));
            assert_eq!("你".unicode_truncate(4), ("你", 2));
        }

        #[test]
        fn at_boundary() {
            assert_eq!("boundary".unicode_truncate(5), ("bound", 5));
            assert_eq!("你好吗".unicode_truncate(4), ("你好", 4));
        }

        #[test]
        fn not_boundary() {
            assert_eq!("你好吗".unicode_truncate(3), ("你", 2));
            assert_eq!("你好吗".unicode_truncate(1), ("", 0));
        }

        #[test]
        fn zero_width_char_in_middle() {
            // zero width character in the middle is intact
            assert_eq!("y\u{0306}es".unicode_truncate(2), ("y\u{0306}e", 2));
        }

        #[test]
        fn keep_zero_width_char_at_boundary() {
            // zero width character at end is preserved
            assert_eq!(
                "y\u{0306}ey\u{0306}s".unicode_truncate(3),
                ("y\u{0306}ey\u{0306}", 3)
            );
        }

        #[test]
        fn family_stays_together() {
            let input = "123👨‍👩‍👧‍👦456";
            assert_eq!(input.unicode_truncate(4), ("123", 3));
            assert_eq!(input.unicode_truncate(8), ("123", 3));
            assert_eq!(input.unicode_truncate(12), ("123👨‍👩‍👧‍👦4", 12));
            assert_eq!(input.unicode_truncate(20), (input, 14));
        }
    }

    mod truncate_start {
        use super::*;

        #[test]
        fn empty() {
            assert_eq!("".unicode_truncate_start(4), ("", 0));
        }

        #[test]
        fn zero_width() {
            assert_eq!("ab".unicode_truncate_start(0), ("", 0));
            assert_eq!("你好".unicode_truncate_start(0), ("", 0));
        }

        #[test]
        fn less_than_limit() {
            assert_eq!("abc".unicode_truncate_start(4), ("abc", 3));
            assert_eq!("你".unicode_truncate_start(4), ("你", 2));
        }

        #[test]
        fn at_boundary() {
            assert_eq!("boundary".unicode_truncate_start(5), ("ndary", 5));
            assert_eq!("你好吗".unicode_truncate_start(4), ("好吗", 4));
        }

        #[test]
        fn not_boundary() {
            assert_eq!("你好吗".unicode_truncate_start(3), ("吗", 2));
            assert_eq!("你好吗".unicode_truncate_start(1), ("", 0));
        }

        #[test]
        fn zero_width_char_in_middle() {
            // zero width character in middle is preserved
            assert_eq!(
                "y\u{0306}ey\u{0306}s".unicode_truncate_start(2),
                ("y\u{0306}s", 2)
            );
        }

        #[test]
        fn remove_zero_width_char_at_boundary() {
            // zero width character in the middle at the cutting boundary is removed
            assert_eq!("y\u{0306}es".unicode_truncate_start(2), ("es", 2));
        }

        #[test]
        fn family_stays_together() {
            let input = "123👨‍👩‍👧‍👦456";
            assert_eq!(input.unicode_truncate_start(4), ("456", 3));
            assert_eq!(input.unicode_truncate_start(8), ("456", 3));
            assert_eq!(input.unicode_truncate_start(12), ("3👨‍👩‍👧‍👦456", 12));
            assert_eq!(input.unicode_truncate_start(20), (input, 14));
        }
    }

    mod truncate_centered {
        use super::*;

        #[test]
        fn empty() {
            assert_eq!("".unicode_truncate_centered(4), ("", 0));
        }

        #[test]
        fn zero_width() {
            assert_eq!("ab".unicode_truncate_centered(0), ("", 0));
            assert_eq!("你好".unicode_truncate_centered(0), ("", 0));
        }

        #[test]
        fn less_than_limit() {
            assert_eq!("abc".unicode_truncate_centered(4), ("abc", 3));
            assert_eq!("你".unicode_truncate_centered(4), ("你", 2));
        }

        /// The source code has special handling for small `min_removal_width` (half-point)
        #[test]
        fn truncate_exactly_one() {
            assert_eq!("abcd".unicode_truncate_centered(3), ("abc", 3));
        }

        #[test]
        fn at_boundary() {
            assert_eq!(
                "boundaryboundary".unicode_truncate_centered(5),
                ("arybo", 5)
            );
            assert_eq!(
                "你好吗你好吗你好吗".unicode_truncate_centered(4),
                ("你好", 4)
            );
        }

        #[test]
        fn not_boundary() {
            assert_eq!("你好吗你好吗".unicode_truncate_centered(3), ("吗", 2));
            assert_eq!("你好吗你好吗".unicode_truncate_centered(1), ("", 0));
        }

        #[test]
        fn zero_width_char_in_middle() {
            // zero width character in middle is preserved
            assert_eq!(
                "yy\u{0306}es".unicode_truncate_centered(2),
                ("y\u{0306}e", 2)
            );
        }

        #[test]
        fn zero_width_char_at_boundary() {
            // zero width character at the cutting boundary in the start is removed
            // but those in the end is kept.
            assert_eq!(
                "y\u{0306}ea\u{0306}b\u{0306}y\u{0306}ea\u{0306}b\u{0306}"
                    .unicode_truncate_centered(2),
                ("b\u{0306}y\u{0306}", 2)
            );
            assert_eq!(
                "ay\u{0306}ea\u{0306}b\u{0306}y\u{0306}ea\u{0306}b\u{0306}"
                    .unicode_truncate_centered(2),
                ("a\u{0306}b\u{0306}", 2)
            );
            assert_eq!(
                "y\u{0306}ea\u{0306}b\u{0306}y\u{0306}ea\u{0306}b\u{0306}a"
                    .unicode_truncate_centered(2),
                ("b\u{0306}y\u{0306}", 2)
            );
        }

        #[test]
        fn control_char() {
            use unicode_width::UnicodeWidthChar;
            assert_eq!("\u{0019}".width(), 1);
            assert_eq!('\u{0019}'.width(), None);
            assert_eq!("\u{0019}".unicode_truncate(2), ("\u{0019}", 1));
        }

        #[test]
        fn family_stays_together() {
            let input = "123👨‍👩‍👧‍👦456";
            assert_eq!(input.unicode_truncate_centered(4), ("", 0));
            assert_eq!(input.unicode_truncate_centered(8), ("👨‍👩‍👧‍👦", 8));
            assert_eq!(input.unicode_truncate_centered(12), ("23👨‍👩‍👧‍👦45", 12));
            assert_eq!(input.unicode_truncate_centered(20), (input, 14));
        }
    }

    #[test]
    fn truncate_aligned() {
        assert_eq!("abc".unicode_truncate_aligned(1, Alignment::Left), ("a", 1));
        assert_eq!(
            "abc".unicode_truncate_aligned(1, Alignment::Center),
            ("b", 1)
        );
        assert_eq!(
            "abc".unicode_truncate_aligned(1, Alignment::Right),
            ("c", 1)
        );
    }

    #[cfg(feature = "std")]
    mod pad {
        use super::*;

        #[test]
        fn zero_width() {
            assert_eq!("你好".unicode_pad(0, Alignment::Left, true), "");
            assert_eq!("你好".unicode_pad(0, Alignment::Left, false), "你好");
        }

        #[test]
        fn less_than_limit() {
            assert_eq!("你".unicode_pad(4, Alignment::Left, true), "你  ");
            assert_eq!("你".unicode_pad(4, Alignment::Left, false), "你  ");
        }

        #[test]
        fn width_at_boundary() {
            assert_eq!("你好吗".unicode_pad(4, Alignment::Left, true), "你好");
            assert_eq!("你好吗".unicode_pad(4, Alignment::Left, false), "你好吗");
        }

        #[test]
        fn width_not_boundary() {
            // above limit wide chars not at boundary
            assert_eq!("你好吗".unicode_pad(3, Alignment::Left, true), "你 ");
            assert_eq!("你好吗".unicode_pad(1, Alignment::Left, true), " ");
            assert_eq!("你好吗".unicode_pad(3, Alignment::Left, false), "你好吗");

            assert_eq!("你好吗".unicode_pad(3, Alignment::Center, true), "你 ");

            assert_eq!("你好吗".unicode_pad(3, Alignment::Right, true), " 你");
        }
    }
}
