// The code in this file was adapted from the CharIndices implementation of
// the Rust standard library at revision ab32548539ec38a939c1b58599249f3b54130026
// (https://github.com/rust-lang/rust/blob/ab32548539ec38a939c1b58599249f3b54130026/library/core/src/str/iter.rs).
//
// Excerpt from https://github.com/rust-lang/rust/blob/ab32548539ec38a939c1b58599249f3b54130026/COPYRIGHT ,
// which refers to
// https://github.com/rust-lang/rust/blob/ab32548539ec38a939c1b58599249f3b54130026/LICENSE-APACHE
// and
// https://github.com/rust-lang/rust/blob/ab32548539ec38a939c1b58599249f3b54130026/LICENSE-MIT
// :
//
// For full authorship information, see the version control history or
// https://thanks.rust-lang.org
//
// Except as otherwise noted (below and/or in individual files), Rust is
// licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.

use super::Utf8Chars;
use core::iter::FusedIterator;

/// An iterator over the [`char`]s  and their positions.
#[derive(Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Utf8CharIndices<'a> {
    front_offset: usize,
    iter: Utf8Chars<'a>,
}

impl<'a> Iterator for Utf8CharIndices<'a> {
    type Item = (usize, char);

    #[inline]
    fn next(&mut self) -> Option<(usize, char)> {
        let pre_len = self.as_slice().len();
        match self.iter.next() {
            None => None,
            Some(ch) => {
                let index = self.front_offset;
                let len = self.as_slice().len();
                self.front_offset += pre_len - len;
                Some((index, ch))
            }
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.iter.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline]
    fn last(mut self) -> Option<(usize, char)> {
        // No need to go through the entire string.
        self.next_back()
    }
}

impl<'a> DoubleEndedIterator for Utf8CharIndices<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<(usize, char)> {
        self.iter.next_back().map(|ch| {
            let index = self.front_offset + self.as_slice().len();
            (index, ch)
        })
    }
}

impl FusedIterator for Utf8CharIndices<'_> {}

impl<'a> Utf8CharIndices<'a> {
    #[inline(always)]
    /// Creates the iterator from a byte slice.
    pub fn new(bytes: &'a [u8]) -> Self {
        Utf8CharIndices::<'a> {
            front_offset: 0,
            iter: Utf8Chars::new(bytes),
        }
    }

    /// Views the underlying data as a subslice of the original data.
    ///
    /// This has the same lifetime as the original slice, and so the
    /// iterator can continue to be used while this exists.
    #[must_use]
    #[inline]
    pub fn as_slice(&self) -> &'a [u8] {
        self.iter.as_slice()
    }

    /// Returns the byte position of the next character, or the length
    /// of the underlying string if there are no more characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use utf8_iter::Utf8CharsEx;
    /// let mut chars = "a楽".as_bytes().char_indices();
    ///
    /// assert_eq!(chars.offset(), 0);
    /// assert_eq!(chars.next(), Some((0, 'a')));
    ///
    /// assert_eq!(chars.offset(), 1);
    /// assert_eq!(chars.next(), Some((1, '楽')));
    ///
    /// assert_eq!(chars.offset(), 4);
    /// assert_eq!(chars.next(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn offset(&self) -> usize {
        self.front_offset
    }
}
