//! The various iterators this crate provides.
//!
//! These iterators are not a very stable interface and you really should
//! avoid considering them to be concrete types.  A lot of the iterators in
//! this crate use `impl Iterator` for this reason but restrictions in the
//! language don't allow this to be used in all places on the versions of
//! rust this crate wants to compile for.
use std::marker::PhantomData;
use std::ops::{Index, Range};

use crate::{Change, ChangeTag, DiffOp, DiffTag};

/// Iterator for [`DiffOp::iter_changes`].
pub struct ChangesIter<'lookup, Old: ?Sized, New: ?Sized, T> {
    old: &'lookup Old,
    new: &'lookup New,
    old_range: Range<usize>,
    new_range: Range<usize>,
    old_index: usize,
    new_index: usize,
    old_i: usize,
    new_i: usize,
    tag: DiffTag,
    _marker: PhantomData<T>,
}

impl<'lookup, Old, New, T> ChangesIter<'lookup, Old, New, T>
where
    Old: Index<usize, Output = T> + ?Sized,
    New: Index<usize, Output = T> + ?Sized,
{
    pub(crate) fn new(old: &'lookup Old, new: &'lookup New, op: DiffOp) -> Self {
        let (tag, old_range, new_range) = op.as_tag_tuple();
        let old_index = old_range.start;
        let new_index = new_range.start;
        let old_i = old_range.start;
        let new_i = new_range.start;
        ChangesIter {
            old,
            new,
            old_range,
            new_range,
            old_index,
            new_index,
            old_i,
            new_i,
            tag,
            _marker: PhantomData,
        }
    }
}

impl<Old, New, T> Iterator for ChangesIter<'_, Old, New, T>
where
    Old: Index<usize, Output = T> + ?Sized,
    New: Index<usize, Output = T> + ?Sized,
    T: Clone,
{
    type Item = Change<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.tag {
            DiffTag::Equal => {
                if self.old_i < self.old_range.end {
                    let value = self.old[self.old_i].clone();
                    self.old_i += 1;
                    self.old_index += 1;
                    self.new_index += 1;
                    Some(Change {
                        tag: ChangeTag::Equal,
                        old_index: Some(self.old_index - 1),
                        new_index: Some(self.new_index - 1),
                        value,
                    })
                } else {
                    None
                }
            }
            DiffTag::Delete => {
                if self.old_i < self.old_range.end {
                    let value = self.old[self.old_i].clone();
                    self.old_i += 1;
                    self.old_index += 1;
                    Some(Change {
                        tag: ChangeTag::Delete,
                        old_index: Some(self.old_index - 1),
                        new_index: None,
                        value,
                    })
                } else {
                    None
                }
            }
            DiffTag::Insert => {
                if self.new_i < self.new_range.end {
                    let value = self.new[self.new_i].clone();
                    self.new_i += 1;
                    self.new_index += 1;
                    Some(Change {
                        tag: ChangeTag::Insert,
                        old_index: None,
                        new_index: Some(self.new_index - 1),
                        value,
                    })
                } else {
                    None
                }
            }
            DiffTag::Replace => {
                if self.old_i < self.old_range.end {
                    let value = self.old[self.old_i].clone();
                    self.old_i += 1;
                    self.old_index += 1;
                    Some(Change {
                        tag: ChangeTag::Delete,
                        old_index: Some(self.old_index - 1),
                        new_index: None,
                        value,
                    })
                } else if self.new_i < self.new_range.end {
                    let value = self.new[self.new_i].clone();
                    self.new_i += 1;
                    self.new_index += 1;
                    Some(Change {
                        tag: ChangeTag::Insert,
                        old_index: None,
                        new_index: Some(self.new_index - 1),
                        value,
                    })
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(feature = "text")]
mod text {
    use super::*;

    /// Iterator for [`TextDiff::iter_all_changes`](crate::TextDiff::iter_all_changes).
    pub struct AllChangesIter<'slf, 'data, T: ?Sized> {
        old: &'slf [&'data T],
        new: &'slf [&'data T],
        ops: &'slf [DiffOp],
        current_iter: Option<ChangesIter<'slf, [&'data T], [&'data T], &'data T>>,
    }

    impl<'slf, 'data, T> AllChangesIter<'slf, 'data, T>
    where
        T: 'data + ?Sized + PartialEq,
    {
        pub(crate) fn new(
            old: &'slf [&'data T],
            new: &'slf [&'data T],
            ops: &'slf [DiffOp],
        ) -> Self {
            AllChangesIter {
                old,
                new,
                ops,
                current_iter: None,
            }
        }
    }

    impl<'slf, 'data, T> Iterator for AllChangesIter<'slf, 'data, T>
    where
        T: PartialEq + 'data + ?Sized,
        'data: 'slf,
    {
        type Item = Change<&'data T>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                if let Some(ref mut iter) = self.current_iter {
                    if let Some(rv) = iter.next() {
                        return Some(rv);
                    }
                    self.current_iter.take();
                }
                if let Some((&first, rest)) = self.ops.split_first() {
                    self.current_iter = Some(ChangesIter::new(self.old, self.new, first));
                    self.ops = rest;
                } else {
                    return None;
                }
            }
        }
    }
}

#[cfg(feature = "text")]
pub use self::text::*;
