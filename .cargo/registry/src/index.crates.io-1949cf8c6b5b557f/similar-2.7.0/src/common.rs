use std::hash::Hash;
use std::ops::{Index, Range};

use crate::algorithms::{diff_deadline, Capture, Compact, Replace};
use crate::deadline_support::Instant;
use crate::{Algorithm, DiffOp};

/// Creates a diff between old and new with the given algorithm capturing the ops.
///
/// This is like [`diff`](crate::algorithms::diff) but instead of using an
/// arbitrary hook this will always use [`Compact`] + [`Replace`] + [`Capture`]
/// and return the captured [`DiffOp`]s.
pub fn capture_diff<Old, New>(
    alg: Algorithm,
    old: &Old,
    old_range: Range<usize>,
    new: &New,
    new_range: Range<usize>,
) -> Vec<DiffOp>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    Old::Output: Hash + Eq + Ord,
    New::Output: PartialEq<Old::Output> + Hash + Eq + Ord,
{
    capture_diff_deadline(alg, old, old_range, new, new_range, None)
}

/// Creates a diff between old and new with the given algorithm capturing the ops.
///
/// Works like [`capture_diff`] but with an optional deadline.
pub fn capture_diff_deadline<Old, New>(
    alg: Algorithm,
    old: &Old,
    old_range: Range<usize>,
    new: &New,
    new_range: Range<usize>,
    deadline: Option<Instant>,
) -> Vec<DiffOp>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    Old::Output: Hash + Eq + Ord,
    New::Output: PartialEq<Old::Output> + Hash + Eq + Ord,
{
    let mut d = Compact::new(Replace::new(Capture::new()), old, new);
    diff_deadline(alg, &mut d, old, old_range, new, new_range, deadline).unwrap();
    d.into_inner().into_inner().into_ops()
}

/// Creates a diff between old and new with the given algorithm capturing the ops.
pub fn capture_diff_slices<T>(alg: Algorithm, old: &[T], new: &[T]) -> Vec<DiffOp>
where
    T: Eq + Hash + Ord,
{
    capture_diff_slices_deadline(alg, old, new, None)
}

/// Creates a diff between old and new with the given algorithm capturing the ops.
///
/// Works like [`capture_diff_slices`] but with an optional deadline.
pub fn capture_diff_slices_deadline<T>(
    alg: Algorithm,
    old: &[T],
    new: &[T],
    deadline: Option<Instant>,
) -> Vec<DiffOp>
where
    T: Eq + Hash + Ord,
{
    capture_diff_deadline(alg, old, 0..old.len(), new, 0..new.len(), deadline)
}

/// Return a measure of similarity in the range `0..=1`.
///
/// A ratio of `1.0` means the two sequences are a complete match, a
/// ratio of `0.0` would indicate completely distinct sequences.  The input
/// is the sequence of diff operations and the length of the old and new
/// sequence.
pub fn get_diff_ratio(ops: &[DiffOp], old_len: usize, new_len: usize) -> f32 {
    let matches = ops
        .iter()
        .map(|op| {
            if let DiffOp::Equal { len, .. } = *op {
                len
            } else {
                0
            }
        })
        .sum::<usize>();
    let len = old_len + new_len;
    if len == 0 {
        1.0
    } else {
        2.0 * matches as f32 / len as f32
    }
}

/// Isolate change clusters by eliminating ranges with no changes.
///
/// This will leave holes behind in long periods of equal ranges so that
/// you can build things like unified diffs.
pub fn group_diff_ops(mut ops: Vec<DiffOp>, n: usize) -> Vec<Vec<DiffOp>> {
    if ops.is_empty() {
        return vec![];
    }

    let mut pending_group = Vec::new();
    let mut rv = Vec::new();

    if let Some(DiffOp::Equal {
        old_index,
        new_index,
        len,
    }) = ops.first_mut()
    {
        let offset = (*len).saturating_sub(n);
        *old_index += offset;
        *new_index += offset;
        *len -= offset;
    }

    if let Some(DiffOp::Equal { len, .. }) = ops.last_mut() {
        *len -= (*len).saturating_sub(n);
    }

    for op in ops.into_iter() {
        if let DiffOp::Equal {
            old_index,
            new_index,
            len,
        } = op
        {
            // End the current group and start a new one whenever
            // there is a large range with no changes.
            if len > n * 2 {
                pending_group.push(DiffOp::Equal {
                    old_index,
                    new_index,
                    len: n,
                });
                rv.push(pending_group);
                let offset = len.saturating_sub(n);
                pending_group = vec![DiffOp::Equal {
                    old_index: old_index + offset,
                    new_index: new_index + offset,
                    len: len - offset,
                }];
                continue;
            }
        }
        pending_group.push(op);
    }

    match &pending_group[..] {
        &[] | &[DiffOp::Equal { .. }] => {}
        _ => rv.push(pending_group),
    }

    rv
}

#[test]
fn test_non_string_iter_change() {
    use crate::ChangeTag;

    let old = vec![1, 2, 3];
    let new = vec![1, 2, 4];
    let ops = capture_diff_slices(Algorithm::Myers, &old, &new);
    let changes: Vec<_> = ops
        .iter()
        .flat_map(|x| x.iter_changes(&old, &new))
        .map(|x| (x.tag(), x.value()))
        .collect();

    assert_eq!(
        changes,
        vec![
            (ChangeTag::Equal, 1),
            (ChangeTag::Equal, 2),
            (ChangeTag::Delete, 3),
            (ChangeTag::Insert, 4),
        ]
    );
}
