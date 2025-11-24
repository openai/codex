//! Implements basic compacting.  This is based on the compaction logic from
//! diffy by Brandon Williams.
use std::ops::Index;

use crate::{DiffOp, DiffTag};

use super::utils::{common_prefix_len, common_suffix_len};
use super::DiffHook;

/// Performs semantic cleanup operations on a diff.
///
/// This merges similar ops together but also tries to move hunks up and
/// down the diff with the desire to connect as many hunks as possible.
/// It still needs to be combined with [`Replace`](crate::algorithms::Replace)
/// to get actual replace diff ops out.
#[derive(Debug)]
pub struct Compact<'old, 'new, Old: ?Sized, New: ?Sized, D> {
    d: D,
    ops: Vec<DiffOp>,
    old: &'old Old,
    new: &'new New,
}

impl<'old, 'new, Old, New, D> Compact<'old, 'new, Old, New, D>
where
    D: DiffHook,
    Old: Index<usize> + ?Sized + 'old,
    New: Index<usize> + ?Sized + 'new,
    New::Output: PartialEq<Old::Output>,
{
    /// Creates a new compact hook wrapping another hook.
    pub fn new(d: D, old: &'old Old, new: &'new New) -> Self {
        Compact {
            d,
            ops: Vec::new(),
            old,
            new,
        }
    }

    /// Extracts the inner hook.
    pub fn into_inner(self) -> D {
        self.d
    }
}

impl<Old: ?Sized, New: ?Sized, D: DiffHook> AsRef<D> for Compact<'_, '_, Old, New, D> {
    fn as_ref(&self) -> &D {
        &self.d
    }
}

impl<Old: ?Sized, New: ?Sized, D: DiffHook> AsMut<D> for Compact<'_, '_, Old, New, D> {
    fn as_mut(&mut self) -> &mut D {
        &mut self.d
    }
}

impl<'old, 'new, Old, New, D> DiffHook for Compact<'old, 'new, Old, New, D>
where
    D: DiffHook,
    Old: Index<usize> + ?Sized + 'old,
    New: Index<usize> + ?Sized + 'new,
    New::Output: PartialEq<Old::Output>,
{
    type Error = D::Error;

    #[inline(always)]
    fn equal(&mut self, old_index: usize, new_index: usize, len: usize) -> Result<(), Self::Error> {
        self.ops.push(DiffOp::Equal {
            old_index,
            new_index,
            len,
        });
        Ok(())
    }

    #[inline(always)]
    fn delete(
        &mut self,
        old_index: usize,
        old_len: usize,
        new_index: usize,
    ) -> Result<(), Self::Error> {
        self.ops.push(DiffOp::Delete {
            old_index,
            old_len,
            new_index,
        });
        Ok(())
    }

    #[inline(always)]
    fn insert(
        &mut self,
        old_index: usize,
        new_index: usize,
        new_len: usize,
    ) -> Result<(), Self::Error> {
        self.ops.push(DiffOp::Insert {
            old_index,
            new_index,
            new_len,
        });
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        cleanup_diff_ops(self.old, self.new, &mut self.ops);
        for op in &self.ops {
            op.apply_to_hook(&mut self.d)?;
        }
        self.d.finish()
    }
}

// Walks through all edits and shifts them up and then down, trying to see if
// they run into similar edits which can be merged.
pub fn cleanup_diff_ops<Old, New>(old: &Old, new: &New, ops: &mut Vec<DiffOp>)
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    New::Output: PartialEq<Old::Output>,
{
    // First attempt to compact all Deletions
    let mut pointer = 0;
    while let Some(&op) = ops.get(pointer) {
        if let DiffTag::Delete = op.tag() {
            pointer = shift_diff_ops_up(ops, old, new, pointer);
            pointer = shift_diff_ops_down(ops, old, new, pointer);
        }
        pointer += 1;
    }

    // Then attempt to compact all Insertions
    let mut pointer = 0;
    while let Some(&op) = ops.get(pointer) {
        if let DiffTag::Insert = op.tag() {
            pointer = shift_diff_ops_up(ops, old, new, pointer);
            pointer = shift_diff_ops_down(ops, old, new, pointer);
        }
        pointer += 1;
    }
}

fn shift_diff_ops_up<Old, New>(
    ops: &mut Vec<DiffOp>,
    old: &Old,
    new: &New,
    mut pointer: usize,
) -> usize
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    New::Output: PartialEq<Old::Output>,
{
    while let Some(&prev_op) = pointer.checked_sub(1).and_then(|idx| ops.get(idx)) {
        let this_op = ops[pointer];
        match (this_op.tag(), prev_op.tag()) {
            // Shift Inserts Upwards
            (DiffTag::Insert, DiffTag::Equal) => {
                let suffix_len =
                    common_suffix_len(old, prev_op.old_range(), new, this_op.new_range());
                if suffix_len > 0 {
                    if let Some(DiffTag::Equal) = ops.get(pointer + 1).map(|x| x.tag()) {
                        ops[pointer + 1].grow_left(suffix_len);
                    } else {
                        ops.insert(
                            pointer + 1,
                            DiffOp::Equal {
                                old_index: prev_op.old_range().end - suffix_len,
                                new_index: this_op.new_range().end - suffix_len,
                                len: suffix_len,
                            },
                        );
                    }
                    ops[pointer].shift_left(suffix_len);
                    ops[pointer - 1].shrink_left(suffix_len);

                    if ops[pointer - 1].is_empty() {
                        ops.remove(pointer - 1);
                        pointer -= 1;
                    }
                } else if ops[pointer - 1].is_empty() {
                    ops.remove(pointer - 1);
                    pointer -= 1;
                } else {
                    // We can't shift upwards anymore
                    break;
                }
            }
            // Shift Deletions Upwards
            (DiffTag::Delete, DiffTag::Equal) => {
                // check common suffix for the amount we can shift
                let suffix_len =
                    common_suffix_len(old, prev_op.old_range(), new, this_op.new_range());
                if suffix_len != 0 {
                    if let Some(DiffTag::Equal) = ops.get(pointer + 1).map(|x| x.tag()) {
                        ops[pointer + 1].grow_left(suffix_len);
                    } else {
                        let old_range = prev_op.old_range();
                        ops.insert(
                            pointer + 1,
                            DiffOp::Equal {
                                old_index: old_range.end - suffix_len,
                                new_index: this_op.new_range().end - suffix_len,
                                len: old_range.len() - suffix_len,
                            },
                        );
                    }
                    ops[pointer].shift_left(suffix_len);
                    ops[pointer - 1].shrink_left(suffix_len);

                    if ops[pointer - 1].is_empty() {
                        ops.remove(pointer - 1);
                        pointer -= 1;
                    }
                } else if ops[pointer - 1].is_empty() {
                    ops.remove(pointer - 1);
                    pointer -= 1;
                } else {
                    // We can't shift upwards anymore
                    break;
                }
            }
            // Swap the Delete and Insert
            (DiffTag::Insert, DiffTag::Delete) | (DiffTag::Delete, DiffTag::Insert) => {
                ops.swap(pointer - 1, pointer);
                pointer -= 1;
            }
            // Merge the two ranges
            (DiffTag::Insert, DiffTag::Insert) => {
                ops[pointer - 1].grow_right(this_op.new_range().len());
                ops.remove(pointer);
                pointer -= 1;
            }
            (DiffTag::Delete, DiffTag::Delete) => {
                ops[pointer - 1].grow_right(this_op.old_range().len());
                ops.remove(pointer);
                pointer -= 1;
            }
            _ => unreachable!("unexpected tag"),
        }
    }
    pointer
}

fn shift_diff_ops_down<Old, New>(
    ops: &mut Vec<DiffOp>,
    old: &Old,
    new: &New,
    mut pointer: usize,
) -> usize
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    New::Output: PartialEq<Old::Output>,
{
    while let Some(&next_op) = pointer.checked_add(1).and_then(|idx| ops.get(idx)) {
        let this_op = ops[pointer];
        match (this_op.tag(), next_op.tag()) {
            // Shift Inserts Downwards
            (DiffTag::Insert, DiffTag::Equal) => {
                let prefix_len =
                    common_prefix_len(old, next_op.old_range(), new, this_op.new_range());
                if prefix_len > 0 {
                    if let Some(DiffTag::Equal) = pointer
                        .checked_sub(1)
                        .and_then(|x| ops.get(x))
                        .map(|x| x.tag())
                    {
                        ops[pointer - 1].grow_right(prefix_len);
                    } else {
                        ops.insert(
                            pointer,
                            DiffOp::Equal {
                                old_index: next_op.old_range().start,
                                new_index: this_op.new_range().start,
                                len: prefix_len,
                            },
                        );
                        pointer += 1;
                    }
                    ops[pointer].shift_right(prefix_len);
                    ops[pointer + 1].shrink_right(prefix_len);

                    if ops[pointer + 1].is_empty() {
                        ops.remove(pointer + 1);
                    }
                } else if ops[pointer + 1].is_empty() {
                    ops.remove(pointer + 1);
                } else {
                    // We can't shift upwards anymore
                    break;
                }
            }
            // Shift Deletions Downwards
            (DiffTag::Delete, DiffTag::Equal) => {
                // check common suffix for the amount we can shift
                let prefix_len =
                    common_prefix_len(old, next_op.old_range(), new, this_op.new_range());
                if prefix_len > 0 {
                    if let Some(DiffTag::Equal) = pointer
                        .checked_sub(1)
                        .and_then(|x| ops.get(x))
                        .map(|x| x.tag())
                    {
                        ops[pointer - 1].grow_right(prefix_len);
                    } else {
                        ops.insert(
                            pointer,
                            DiffOp::Equal {
                                old_index: next_op.old_range().start,
                                new_index: this_op.new_range().start,
                                len: prefix_len,
                            },
                        );
                        pointer += 1;
                    }
                    ops[pointer].shift_right(prefix_len);
                    ops[pointer + 1].shrink_right(prefix_len);

                    if ops[pointer + 1].is_empty() {
                        ops.remove(pointer + 1);
                    }
                } else if ops[pointer + 1].is_empty() {
                    ops.remove(pointer + 1);
                } else {
                    // We can't shift downwards anymore
                    break;
                }
            }
            // Swap the Delete and Insert
            (DiffTag::Insert, DiffTag::Delete) | (DiffTag::Delete, DiffTag::Insert) => {
                ops.swap(pointer, pointer + 1);
                pointer += 1;
            }
            // Merge the two ranges
            (DiffTag::Insert, DiffTag::Insert) => {
                ops[pointer].grow_right(next_op.new_range().len());
                ops.remove(pointer + 1);
            }
            (DiffTag::Delete, DiffTag::Delete) => {
                ops[pointer].grow_right(next_op.old_range().len());
                ops.remove(pointer + 1);
            }
            _ => unreachable!("unexpected tag"),
        }
    }
    pointer
}
