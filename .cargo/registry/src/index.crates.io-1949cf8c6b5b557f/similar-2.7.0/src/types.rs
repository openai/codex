use std::fmt;
use std::ops::{Index, Range};

use crate::algorithms::utils::is_empty_range;
use crate::algorithms::DiffHook;
use crate::iter::ChangesIter;

/// An enum representing a diffing algorithm.
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum Algorithm {
    /// Picks the myers algorithm from [`crate::algorithms::myers`]
    Myers,
    /// Picks the patience algorithm from [`crate::algorithms::patience`]
    Patience,
    /// Picks the LCS algorithm from [`crate::algorithms::lcs`]
    Lcs,
}

impl Default for Algorithm {
    /// Returns the default algorithm ([`Algorithm::Myers`]).
    fn default() -> Algorithm {
        Algorithm::Myers
    }
}

/// The tag of a change.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Ord, PartialOrd)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum ChangeTag {
    /// The change indicates equality (not a change)
    Equal,
    /// The change indicates deleted text.
    Delete,
    /// The change indicates inserted text.
    Insert,
}

impl fmt::Display for ChangeTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match &self {
                ChangeTag::Equal => ' ',
                ChangeTag::Delete => '-',
                ChangeTag::Insert => '+',
            }
        )
    }
}

/// Represents the expanded [`DiffOp`] change.
///
/// This type is returned from [`DiffOp::iter_changes`] and
/// [`TextDiff::iter_changes`](crate::text::TextDiff::iter_changes).
///
/// It exists so that it's more convenient to work with textual differences as
/// the underlying [`DiffOp`] encodes a group of changes.
///
/// This type has additional methods that are only available for types
/// implementing [`DiffableStr`](crate::text::DiffableStr).
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Change<T> {
    pub(crate) tag: ChangeTag,
    pub(crate) old_index: Option<usize>,
    pub(crate) new_index: Option<usize>,
    pub(crate) value: T,
}

/// These methods are available for all change types.
impl<T: Clone> Change<T> {
    /// Returns the change tag.
    pub fn tag(&self) -> ChangeTag {
        self.tag
    }

    /// Returns the old index if available.
    pub fn old_index(&self) -> Option<usize> {
        self.old_index
    }

    /// Returns the new index if available.
    pub fn new_index(&self) -> Option<usize> {
        self.new_index
    }

    /// Returns the underlying changed value.
    ///
    /// Depending on the type of the underlying [`crate::text::DiffableStr`]
    /// this value is more or less useful.  If you always want to have a utf-8
    /// string it's best to use the [`Change::as_str`] and
    /// [`Change::to_string_lossy`] methods.
    pub fn value(&self) -> T {
        self.value.clone()
    }

    /// Returns the underlying changed value as reference.
    pub fn value_ref(&self) -> &T {
        &self.value
    }

    /// Returns the underlying changed value as mutable reference.
    pub fn value_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

/// Utility enum to capture a diff operation.
///
/// This is used by [`Capture`](crate::algorithms::Capture).
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", tag = "op")
)]
pub enum DiffOp {
    /// A segment is equal (see [`DiffHook::equal`])
    Equal {
        /// The starting index in the old sequence.
        old_index: usize,
        /// The starting index in the new sequence.
        new_index: usize,
        /// The length of the segment.
        len: usize,
    },
    /// A segment was deleted (see [`DiffHook::delete`])
    Delete {
        /// The starting index in the old sequence.
        old_index: usize,
        /// The length of the old segment.
        old_len: usize,
        /// The starting index in the new sequence.
        new_index: usize,
    },
    /// A segment was inserted (see [`DiffHook::insert`])
    Insert {
        /// The starting index in the old sequence.
        old_index: usize,
        /// The starting index in the new sequence.
        new_index: usize,
        /// The length of the new segment.
        new_len: usize,
    },
    /// A segment was replaced (see [`DiffHook::replace`])
    Replace {
        /// The starting index in the old sequence.
        old_index: usize,
        /// The length of the old segment.
        old_len: usize,
        /// The starting index in the new sequence.
        new_index: usize,
        /// The length of the new segment.
        new_len: usize,
    },
}

/// The tag of a diff operation.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Ord, PartialOrd)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum DiffTag {
    /// The diff op encodes an equal segment.
    Equal,
    /// The diff op encodes a deleted segment.
    Delete,
    /// The diff op encodes an inserted segment.
    Insert,
    /// The diff op encodes a replaced segment.
    Replace,
}

impl DiffOp {
    /// Returns the tag of the operation.
    pub fn tag(self) -> DiffTag {
        self.as_tag_tuple().0
    }

    /// Returns the old range.
    pub fn old_range(&self) -> Range<usize> {
        self.as_tag_tuple().1
    }

    /// Returns the new range.
    pub fn new_range(&self) -> Range<usize> {
        self.as_tag_tuple().2
    }

    /// Transform the op into a tuple of diff tag and ranges.
    ///
    /// This is useful when operating on slices.  The returned format is
    /// `(tag, i1..i2, j1..j2)`:
    ///
    /// * `Replace`: `a[i1..i2]` should be replaced by `b[j1..j2]`
    /// * `Delete`: `a[i1..i2]` should be deleted (`j1 == j2` in this case).
    /// * `Insert`: `b[j1..j2]` should be inserted at `a[i1..i2]` (`i1 == i2` in this case).
    /// * `Equal`: `a[i1..i2]` is equal to `b[j1..j2]`.
    pub fn as_tag_tuple(&self) -> (DiffTag, Range<usize>, Range<usize>) {
        match *self {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => (
                DiffTag::Equal,
                old_index..old_index + len,
                new_index..new_index + len,
            ),
            DiffOp::Delete {
                old_index,
                new_index,
                old_len,
            } => (
                DiffTag::Delete,
                old_index..old_index + old_len,
                new_index..new_index,
            ),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => (
                DiffTag::Insert,
                old_index..old_index,
                new_index..new_index + new_len,
            ),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => (
                DiffTag::Replace,
                old_index..old_index + old_len,
                new_index..new_index + new_len,
            ),
        }
    }

    /// Apply this operation to a diff hook.
    pub fn apply_to_hook<D: DiffHook>(&self, d: &mut D) -> Result<(), D::Error> {
        match *self {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => d.equal(old_index, new_index, len),
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => d.delete(old_index, old_len, new_index),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => d.insert(old_index, new_index, new_len),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => d.replace(old_index, old_len, new_index, new_len),
        }
    }

    /// Iterates over all changes encoded in the diff op against old and new
    /// sequences.
    ///
    /// `old` and `new` are two indexable objects like the types you pass to
    /// the diffing algorithm functions.
    ///
    /// ```rust
    /// use similar::{ChangeTag, Algorithm};
    /// use similar::capture_diff_slices;
    /// let old = vec!["foo", "bar", "baz"];
    /// let new = vec!["foo", "bar", "blah"];
    /// let ops = capture_diff_slices(Algorithm::Myers, &old, &new);
    /// let changes: Vec<_> = ops
    ///     .iter()
    ///     .flat_map(|x| x.iter_changes(&old, &new))
    ///     .map(|x| (x.tag(), x.value()))
    ///     .collect();
    /// assert_eq!(changes, vec![
    ///     (ChangeTag::Equal, "foo"),
    ///     (ChangeTag::Equal, "bar"),
    ///     (ChangeTag::Delete, "baz"),
    ///     (ChangeTag::Insert, "blah"),
    /// ]);
    /// ```
    pub fn iter_changes<'lookup, Old, New, T>(
        &self,
        old: &'lookup Old,
        new: &'lookup New,
    ) -> ChangesIter<'lookup, Old, New, T>
    where
        Old: Index<usize, Output = T> + ?Sized,
        New: Index<usize, Output = T> + ?Sized,
    {
        ChangesIter::new(old, new, *self)
    }

    /// Given a diffop yields the changes it encodes against the given slices.
    ///
    /// This is similar to [`DiffOp::iter_changes`] but instead of yielding the
    /// individual changes it yields consequitive changed slices.
    ///
    /// This will only ever yield a single tuple or two tuples in case a
    /// [`DiffOp::Replace`] operation is passed.
    ///
    /// ```rust
    /// use similar::{ChangeTag, Algorithm};
    /// use similar::capture_diff_slices;
    /// let old = vec!["foo", "bar", "baz"];
    /// let new = vec!["foo", "bar", "blah"];
    /// let ops = capture_diff_slices(Algorithm::Myers, &old, &new);
    /// let changes: Vec<_> = ops.iter().flat_map(|x| x.iter_slices(&old, &new)).collect();
    /// assert_eq!(changes, vec![
    ///     (ChangeTag::Equal, &["foo", "bar"][..]),
    ///     (ChangeTag::Delete, &["baz"][..]),
    ///     (ChangeTag::Insert, &["blah"][..]),
    /// ]);
    /// ```
    ///
    /// Due to lifetime restrictions it's currently impossible for the
    /// returned slices to outlive the lookup.
    pub fn iter_slices<'lookup, Old, New, T>(
        &self,
        old: &'lookup Old,
        new: &'lookup New,
    ) -> impl Iterator<Item = (ChangeTag, &'lookup T)>
    where
        T: 'lookup + ?Sized,
        Old: Index<Range<usize>, Output = T> + ?Sized,
        New: Index<Range<usize>, Output = T> + ?Sized,
    {
        match *self {
            DiffOp::Equal { old_index, len, .. } => {
                Some((ChangeTag::Equal, &old[old_index..old_index + len]))
                    .into_iter()
                    .chain(None)
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => Some((ChangeTag::Insert, &new[new_index..new_index + new_len]))
                .into_iter()
                .chain(None),
            DiffOp::Delete {
                old_index, old_len, ..
            } => Some((ChangeTag::Delete, &old[old_index..old_index + old_len]))
                .into_iter()
                .chain(None),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => Some((ChangeTag::Delete, &old[old_index..old_index + old_len]))
                .into_iter()
                .chain(Some((
                    ChangeTag::Insert,
                    &new[new_index..new_index + new_len],
                ))),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        let (_, old, new) = self.as_tag_tuple();
        is_empty_range(&old) && is_empty_range(&new)
    }

    pub(crate) fn shift_left(&mut self, adjust: usize) {
        self.adjust((adjust, true), (0, false));
    }

    pub(crate) fn shift_right(&mut self, adjust: usize) {
        self.adjust((adjust, false), (0, false));
    }

    pub(crate) fn grow_left(&mut self, adjust: usize) {
        self.adjust((adjust, true), (adjust, false));
    }

    pub(crate) fn grow_right(&mut self, adjust: usize) {
        self.adjust((0, false), (adjust, false));
    }

    pub(crate) fn shrink_left(&mut self, adjust: usize) {
        self.adjust((0, false), (adjust, true));
    }

    pub(crate) fn shrink_right(&mut self, adjust: usize) {
        self.adjust((adjust, false), (adjust, true));
    }

    fn adjust(&mut self, adjust_offset: (usize, bool), adjust_len: (usize, bool)) {
        #[inline(always)]
        fn modify(val: &mut usize, adj: (usize, bool)) {
            if adj.1 {
                *val -= adj.0;
            } else {
                *val += adj.0;
            }
        }

        match self {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                modify(old_index, adjust_offset);
                modify(new_index, adjust_offset);
                modify(len, adjust_len);
            }
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => {
                modify(old_index, adjust_offset);
                modify(old_len, adjust_len);
                modify(new_index, adjust_offset);
            }
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => {
                modify(old_index, adjust_offset);
                modify(new_index, adjust_offset);
                modify(new_len, adjust_len);
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                modify(old_index, adjust_offset);
                modify(old_len, adjust_len);
                modify(new_index, adjust_offset);
                modify(new_len, adjust_len);
            }
        }
    }
}

#[cfg(feature = "text")]
mod text_additions {
    use super::*;
    use crate::text::DiffableStr;
    use std::borrow::Cow;

    /// The text interface can produce changes over [`DiffableStr`] implementing
    /// values.  As those are generic interfaces for different types of strings
    /// utility methods to make working with standard rust strings more enjoyable.
    impl<'s, T: DiffableStr + ?Sized> Change<&'s T> {
        /// Returns the value as string if it is utf-8.
        pub fn as_str(&self) -> Option<&'s str> {
            T::as_str(self.value)
        }

        /// Returns the value (lossy) decoded as utf-8 string.
        pub fn to_string_lossy(&self) -> Cow<'s, str> {
            T::to_string_lossy(self.value)
        }

        /// Returns `true` if this change does not end in a newline and must be
        /// followed up by one if line based diffs are used.
        ///
        /// The [`std::fmt::Display`] implementation of [`Change`] will automatically
        /// insert a newline after the value if this is true.
        pub fn missing_newline(&self) -> bool {
            !T::ends_with_newline(self.value)
        }
    }

    impl<T: DiffableStr + ?Sized> fmt::Display for Change<&T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "{}{}",
                self.to_string_lossy(),
                if self.missing_newline() { "\n" } else { "" }
            )
        }
    }
}
