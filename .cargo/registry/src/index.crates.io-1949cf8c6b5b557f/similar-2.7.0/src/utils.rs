//! Utilities for common diff related operations.
//!
//! This module provides specialized utilities and simplified diff operations
//! for common operations.  It's useful when you want to work with text diffs
//! and you're interested in getting vectors of these changes directly.
//!
//! # Slice Remapping
//!
//! When working with [`TextDiff`] it's common that one takes advantage of the
//! built-in tokenization of the differ.  This for instance lets you do
//! grapheme level diffs.  This is implemented by the differ generating rather
//! small slices of strings and running a diff algorithm over them.
//!
//! The downside of this is that all the [`DiffOp`] objects produced by the
//! diffing algorithm encode operations on these rather small slices.  For
//! a lot of use cases this is not what one wants which can make this very
//! inconvenient.  This module provides a [`TextDiffRemapper`] which lets you
//! map from the ranges that the [`TextDiff`] returns to the original input
//! strings.  For more information see [`TextDiffRemapper`].
//!
//! # Simple Diff Functions
//!
//! This module provides a range of common test diff functions that will
//! produce vectors of `(change_tag, value)` tuples.  They will automatically
//! optimize towards returning the most useful slice that one would expect for
//! the type of diff performed.

use std::hash::Hash;
use std::ops::{Index, Range};

use crate::{
    capture_diff_slices, Algorithm, ChangeTag, DiffOp, DiffableStr, DiffableStrRef, TextDiff,
};

struct SliceRemapper<'x, T: ?Sized> {
    source: &'x T,
    indexes: Vec<Range<usize>>,
}

impl<'x, T: DiffableStr + ?Sized> SliceRemapper<'x, T> {
    fn new(source: &'x T, slices: &[&'x T]) -> SliceRemapper<'x, T> {
        let indexes = slices
            .iter()
            .scan(0, |state, item| {
                let start = *state;
                let end = start + item.len();
                *state = end;
                Some(start..end)
            })
            .collect();
        SliceRemapper { source, indexes }
    }

    fn slice(&self, range: Range<usize>) -> Option<&'x T> {
        let start = self.indexes.get(range.start)?.start;
        let end = self.indexes.get(range.end - 1)?.end;
        Some(self.source.slice(start..end))
    }
}

impl<T: DiffableStr + ?Sized> Index<Range<usize>> for SliceRemapper<'_, T> {
    type Output = T;

    fn index(&self, range: Range<usize>) -> &Self::Output {
        self.slice(range).expect("out of bounds")
    }
}

/// A remapper that can remap diff ops to the original slices.
///
/// The idea here is that when a [`TextDiff`](crate::TextDiff) is created from
/// two strings and the internal tokenization is used, this remapper can take
/// a range in the tokenized sequences and remap it to the original string.
/// This is particularly useful when you want to do things like character or
/// grapheme level diffs but you want to not have to iterate over small sequences
/// but large consequitive ones from the source.
///
/// ```rust
/// use similar::{ChangeTag, TextDiff};
/// use similar::utils::TextDiffRemapper;
///
/// let old = "yo! foo bar baz";
/// let new = "yo! foo bor baz";
/// let diff = TextDiff::from_words(old, new);
/// let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
/// let changes: Vec<_> = diff.ops()
///     .iter()
///     .flat_map(move |x| remapper.iter_slices(x))
///     .collect();
///
/// assert_eq!(changes, vec![
///     (ChangeTag::Equal, "yo! foo "),
///     (ChangeTag::Delete, "bar"),
///     (ChangeTag::Insert, "bor"),
///     (ChangeTag::Equal, " baz")
/// ]);
pub struct TextDiffRemapper<'x, T: ?Sized> {
    old: SliceRemapper<'x, T>,
    new: SliceRemapper<'x, T>,
}

impl<'x, T: DiffableStr + ?Sized> TextDiffRemapper<'x, T> {
    /// Creates a new remapper from strings and slices.
    pub fn new(
        old_slices: &[&'x T],
        new_slices: &[&'x T],
        old: &'x T,
        new: &'x T,
    ) -> TextDiffRemapper<'x, T> {
        TextDiffRemapper {
            old: SliceRemapper::new(old, old_slices),
            new: SliceRemapper::new(new, new_slices),
        }
    }

    /// Creates a new remapper from a text diff and the original strings.
    pub fn from_text_diff<'old, 'new, 'bufs>(
        diff: &TextDiff<'old, 'new, 'bufs, T>,
        old: &'x T,
        new: &'x T,
    ) -> TextDiffRemapper<'x, T>
    where
        'old: 'x,
        'new: 'x,
    {
        TextDiffRemapper {
            old: SliceRemapper::new(old, diff.old_slices()),
            new: SliceRemapper::new(new, diff.new_slices()),
        }
    }

    /// Slices into the old string.
    pub fn slice_old(&self, range: Range<usize>) -> Option<&'x T> {
        self.old.slice(range)
    }

    /// Slices into the new string.
    pub fn slice_new(&self, range: Range<usize>) -> Option<&'x T> {
        self.new.slice(range)
    }

    /// Given a diffop yields the changes it encodes against the original strings.
    ///
    /// This is the same as the [`DiffOp::iter_slices`] method.
    ///
    /// ## Panics
    ///
    /// This method can panic if the input strings passed to the constructor
    /// are incompatible with the input strings passed to the diffing algorithm.
    pub fn iter_slices(&self, op: &DiffOp) -> impl Iterator<Item = (ChangeTag, &'x T)> {
        // note: this is equivalent to the code in `DiffOp::iter_slices`.  It is
        // a copy/paste because the slicing currently cannot be well abstracted
        // because of lifetime issues caused by the `Index` trait.
        match *op {
            DiffOp::Equal { old_index, len, .. } => {
                Some((ChangeTag::Equal, self.old.slice(old_index..old_index + len)))
                    .into_iter()
                    .chain(None)
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => Some((
                ChangeTag::Insert,
                self.new.slice(new_index..new_index + new_len),
            ))
            .into_iter()
            .chain(None),
            DiffOp::Delete {
                old_index, old_len, ..
            } => Some((
                ChangeTag::Delete,
                self.old.slice(old_index..old_index + old_len),
            ))
            .into_iter()
            .chain(None),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => Some((
                ChangeTag::Delete,
                self.old.slice(old_index..old_index + old_len),
            ))
            .into_iter()
            .chain(Some((
                ChangeTag::Insert,
                self.new.slice(new_index..new_index + new_len),
            ))),
        }
        .map(|(tag, opt_val)| (tag, opt_val.expect("slice out of bounds")))
    }
}

/// Shortcut for diffing two slices.
///
/// This function produces the diff of two slices and returns a vector
/// with the changes.
///
/// ```rust
/// use similar::{Algorithm, ChangeTag};
/// use similar::utils::diff_slices;
///
/// let old = "foo\nbar\nbaz".lines().collect::<Vec<_>>();
/// let new = "foo\nbar\nBAZ".lines().collect::<Vec<_>>();
/// assert_eq!(diff_slices(Algorithm::Myers, &old, &new), vec![
///     (ChangeTag::Equal, &["foo", "bar"][..]),
///     (ChangeTag::Delete, &["baz"][..]),
///     (ChangeTag::Insert, &["BAZ"][..]),
/// ]);
/// ```
pub fn diff_slices<'x, T: PartialEq + Hash + Ord>(
    alg: Algorithm,
    old: &'x [T],
    new: &'x [T],
) -> Vec<(ChangeTag, &'x [T])> {
    capture_diff_slices(alg, old, new)
        .iter()
        .flat_map(|op| op.iter_slices(old, new))
        .collect()
}

/// Shortcut for making a character level diff.
///
/// This function produces the diff of two strings and returns a vector
/// with the changes.  It returns connected slices into the original string
/// rather than character level slices.
///
/// ```rust
/// use similar::{Algorithm, ChangeTag};
/// use similar::utils::diff_chars;
///
/// assert_eq!(diff_chars(Algorithm::Myers, "foobarbaz", "fooBARbaz"), vec![
///     (ChangeTag::Equal, "foo"),
///     (ChangeTag::Delete, "bar"),
///     (ChangeTag::Insert, "BAR"),
///     (ChangeTag::Equal, "baz"),
/// ]);
/// ```
pub fn diff_chars<'x, T: DiffableStrRef + ?Sized>(
    alg: Algorithm,
    old: &'x T,
    new: &'x T,
) -> Vec<(ChangeTag, &'x T::Output)> {
    let old = old.as_diffable_str();
    let new = new.as_diffable_str();
    let diff = TextDiff::configure().algorithm(alg).diff_chars(old, new);
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
    diff.ops()
        .iter()
        .flat_map(move |x| remapper.iter_slices(x))
        .collect()
}

/// Shortcut for making a word level diff.
///
/// This function produces the diff of two strings and returns a vector
/// with the changes.  It returns connected slices into the original string
/// rather than word level slices.
///
/// ```rust
/// use similar::{Algorithm, ChangeTag};
/// use similar::utils::diff_words;
///
/// assert_eq!(diff_words(Algorithm::Myers, "foo bar baz", "foo bor baz"), vec![
///     (ChangeTag::Equal, "foo "),
///     (ChangeTag::Delete, "bar"),
///     (ChangeTag::Insert, "bor"),
///     (ChangeTag::Equal, " baz"),
/// ]);
/// ```
pub fn diff_words<'x, T: DiffableStrRef + ?Sized>(
    alg: Algorithm,
    old: &'x T,
    new: &'x T,
) -> Vec<(ChangeTag, &'x T::Output)> {
    let old = old.as_diffable_str();
    let new = new.as_diffable_str();
    let diff = TextDiff::configure().algorithm(alg).diff_words(old, new);
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
    diff.ops()
        .iter()
        .flat_map(move |x| remapper.iter_slices(x))
        .collect()
}

/// Shortcut for making a unicode word level diff.
///
/// This function produces the diff of two strings and returns a vector
/// with the changes.  It returns connected slices into the original string
/// rather than word level slices.
///
/// ```rust
/// use similar::{Algorithm, ChangeTag};
/// use similar::utils::diff_unicode_words;
///
/// let old = "The quick (\"brown\") fox can't jump 32.3 feet, right?";
/// let new = "The quick (\"brown\") fox can't jump 9.84 meters, right?";
/// assert_eq!(diff_unicode_words(Algorithm::Myers, old, new), vec![
///     (ChangeTag::Equal, "The quick (\"brown\") fox can\'t jump "),
///     (ChangeTag::Delete, "32.3"),
///     (ChangeTag::Insert, "9.84"),
///     (ChangeTag::Equal, " "),
///     (ChangeTag::Delete, "feet"),
///     (ChangeTag::Insert, "meters"),
///     (ChangeTag::Equal, ", right?")
/// ]);
/// ```
///
/// This requires the `unicode` feature.
#[cfg(feature = "unicode")]
pub fn diff_unicode_words<'x, T: DiffableStrRef + ?Sized>(
    alg: Algorithm,
    old: &'x T,
    new: &'x T,
) -> Vec<(ChangeTag, &'x T::Output)> {
    let old = old.as_diffable_str();
    let new = new.as_diffable_str();
    let diff = TextDiff::configure()
        .algorithm(alg)
        .diff_unicode_words(old, new);
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
    diff.ops()
        .iter()
        .flat_map(move |x| remapper.iter_slices(x))
        .collect()
}

/// Shortcut for making a grapheme level diff.
///
/// This function produces the diff of two strings and returns a vector
/// with the changes.  It returns connected slices into the original string
/// rather than grapheme level slices.
///
/// ```rust
/// use similar::{Algorithm, ChangeTag};
/// use similar::utils::diff_graphemes;
///
/// let old = "The flag of Austria is 🇦🇹";
/// let new = "The flag of Albania is 🇦🇱";
/// assert_eq!(diff_graphemes(Algorithm::Myers, old, new), vec![
///     (ChangeTag::Equal, "The flag of A"),
///     (ChangeTag::Delete, "ustr"),
///     (ChangeTag::Insert, "lban"),
///     (ChangeTag::Equal, "ia is "),
///     (ChangeTag::Delete, "🇦🇹"),
///     (ChangeTag::Insert, "🇦🇱"),
/// ]);
/// ```
///
/// This requires the `unicode` feature.
#[cfg(feature = "unicode")]
pub fn diff_graphemes<'x, T: DiffableStrRef + ?Sized>(
    alg: Algorithm,
    old: &'x T,
    new: &'x T,
) -> Vec<(ChangeTag, &'x T::Output)> {
    let old = old.as_diffable_str();
    let new = new.as_diffable_str();
    let diff = TextDiff::configure()
        .algorithm(alg)
        .diff_graphemes(old, new);
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
    diff.ops()
        .iter()
        .flat_map(move |x| remapper.iter_slices(x))
        .collect()
}

/// Shortcut for making a line diff.
///
/// This function produces the diff of two slices and returns a vector
/// with the changes.  Unlike [`diff_chars`] or [`diff_slices`] it returns a
/// change tag for each line.
///
/// ```rust
/// use similar::{Algorithm, ChangeTag};
/// use similar::utils::diff_lines;
///
/// assert_eq!(diff_lines(Algorithm::Myers, "foo\nbar\nbaz\nblah", "foo\nbar\nbaz\nblurgh"), vec![
///     (ChangeTag::Equal, "foo\n"),
///     (ChangeTag::Equal, "bar\n"),
///     (ChangeTag::Equal, "baz\n"),
///     (ChangeTag::Delete, "blah"),
///     (ChangeTag::Insert, "blurgh"),
/// ]);
/// ```
pub fn diff_lines<'x, T: DiffableStrRef + ?Sized>(
    alg: Algorithm,
    old: &'x T,
    new: &'x T,
) -> Vec<(ChangeTag, &'x T::Output)> {
    TextDiff::configure()
        .algorithm(alg)
        .diff_lines(old, new)
        .iter_all_changes()
        .map(|change| (change.tag(), change.value()))
        .collect()
}

#[test]
fn test_remapper() {
    let a = "foo bar baz";
    let words = a.tokenize_words();
    dbg!(&words);
    let remap = SliceRemapper::new(a, &words);
    assert_eq!(remap.slice(0..3), Some("foo bar"));
    assert_eq!(remap.slice(1..3), Some(" bar"));
    assert_eq!(remap.slice(0..1), Some("foo"));
    assert_eq!(remap.slice(0..5), Some("foo bar baz"));
    assert_eq!(remap.slice(0..6), None);
}
