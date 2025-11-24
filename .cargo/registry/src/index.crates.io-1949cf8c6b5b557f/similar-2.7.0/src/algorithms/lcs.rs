//! LCS diff algorithm.
//!
//! * time: `O((NM)D log (M)D)`
//! * space `O(MN)`
use std::collections::BTreeMap;
use std::ops::{Index, Range};

use crate::algorithms::utils::{common_prefix_len, common_suffix_len, is_empty_range};
use crate::algorithms::DiffHook;
use crate::deadline_support::{deadline_exceeded, Instant};

/// LCS diff algorithm.
///
/// Diff `old`, between indices `old_range` and `new` between indices `new_range`.
///
/// This diff is done with an optional deadline that defines the maximal
/// execution time permitted before it bails and falls back to an very bad
/// approximation.  Deadlines with LCS do not make a lot of sense and should
/// not be used.
pub fn diff<Old, New, D>(
    d: &mut D,
    old: &Old,
    old_range: Range<usize>,
    new: &New,
    new_range: Range<usize>,
) -> Result<(), D::Error>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    D: DiffHook,
    New::Output: PartialEq<Old::Output>,
{
    diff_deadline(d, old, old_range, new, new_range, None)
}

/// LCS diff algorithm.
///
/// Diff `old`, between indices `old_range` and `new` between indices `new_range`.
///
/// This diff is done with an optional deadline that defines the maximal
/// execution time permitted before it bails and falls back to an approximation.
pub fn diff_deadline<Old, New, D>(
    d: &mut D,
    old: &Old,
    old_range: Range<usize>,
    new: &New,
    new_range: Range<usize>,
    deadline: Option<Instant>,
) -> Result<(), D::Error>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    D: DiffHook,
    New::Output: PartialEq<Old::Output>,
{
    if is_empty_range(&new_range) {
        d.delete(old_range.start, old_range.len(), new_range.start)?;
        d.finish()?;
        return Ok(());
    } else if is_empty_range(&old_range) {
        d.insert(old_range.start, new_range.start, new_range.len())?;
        d.finish()?;
        return Ok(());
    }

    let common_prefix_len = common_prefix_len(old, old_range.clone(), new, new_range.clone());
    let common_suffix_len = common_suffix_len(
        old,
        old_range.start + common_prefix_len..old_range.end,
        new,
        new_range.start + common_prefix_len..new_range.end,
    );

    // If the sequences are not different then we're done
    if common_prefix_len == old_range.len() && (old_range.len() == new_range.len()) {
        d.equal(0, 0, old_range.len())?;
        d.finish()?;
        return Ok(());
    }

    let maybe_table = make_table(
        old,
        common_prefix_len..(old_range.len() - common_suffix_len),
        new,
        common_prefix_len..(new_range.len() - common_suffix_len),
        deadline,
    );
    let mut old_idx = 0;
    let mut new_idx = 0;
    let new_len = new_range.len() - common_prefix_len - common_suffix_len;
    let old_len = old_range.len() - common_prefix_len - common_suffix_len;

    if common_prefix_len > 0 {
        d.equal(old_range.start, new_range.start, common_prefix_len)?;
    }

    if let Some(table) = maybe_table {
        while new_idx < new_len && old_idx < old_len {
            let old_orig_idx = old_range.start + common_prefix_len + old_idx;
            let new_orig_idx = new_range.start + common_prefix_len + new_idx;

            if new[new_orig_idx] == old[old_orig_idx] {
                d.equal(old_orig_idx, new_orig_idx, 1)?;
                old_idx += 1;
                new_idx += 1;
            } else if table.get(&(new_idx, old_idx + 1)).unwrap_or(&0)
                >= table.get(&(new_idx + 1, old_idx)).unwrap_or(&0)
            {
                d.delete(old_orig_idx, 1, new_orig_idx)?;
                old_idx += 1;
            } else {
                d.insert(old_orig_idx, new_orig_idx, 1)?;
                new_idx += 1;
            }
        }
    } else {
        let old_orig_idx = old_range.start + common_prefix_len + old_idx;
        let new_orig_idx = new_range.start + common_prefix_len + new_idx;
        d.delete(old_orig_idx, old_len, new_orig_idx)?;
        d.insert(old_orig_idx, new_orig_idx, new_len)?;
    }

    if old_idx < old_len {
        d.delete(
            old_range.start + common_prefix_len + old_idx,
            old_len - old_idx,
            new_range.start + common_prefix_len + new_idx,
        )?;
        old_idx += old_len - old_idx;
    }

    if new_idx < new_len {
        d.insert(
            old_range.start + common_prefix_len + old_idx,
            new_range.start + common_prefix_len + new_idx,
            new_len - new_idx,
        )?;
    }

    if common_suffix_len > 0 {
        d.equal(
            old_range.start + old_len + common_prefix_len,
            new_range.start + new_len + common_prefix_len,
            common_suffix_len,
        )?;
    }

    d.finish()
}

fn make_table<Old, New>(
    old: &Old,
    old_range: Range<usize>,
    new: &New,
    new_range: Range<usize>,
    deadline: Option<Instant>,
) -> Option<BTreeMap<(usize, usize), u32>>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    New::Output: PartialEq<Old::Output>,
{
    let old_len = old_range.len();
    let new_len = new_range.len();
    let mut table = BTreeMap::new();

    for i in (0..new_len).rev() {
        // are we running for too long?  give up on the table
        if deadline_exceeded(deadline) {
            return None;
        }

        for j in (0..old_len).rev() {
            let val = if new[i] == old[j] {
                table.get(&(i + 1, j + 1)).unwrap_or(&0) + 1
            } else {
                *table
                    .get(&(i + 1, j))
                    .unwrap_or(&0)
                    .max(table.get(&(i, j + 1)).unwrap_or(&0))
            };
            if val > 0 {
                table.insert((i, j), val);
            }
        }
    }

    Some(table)
}

#[test]
fn test_table() {
    let table = make_table(&vec![2, 3], 0..2, &vec![0, 1, 2], 0..3, None).unwrap();
    let expected = {
        let mut m = BTreeMap::new();
        m.insert((1, 0), 1);
        m.insert((0, 0), 1);
        m.insert((2, 0), 1);
        m
    };
    assert_eq!(table, expected);
}

#[test]
fn test_diff() {
    let a: &[usize] = &[0, 1, 2, 3, 4];
    let b: &[usize] = &[0, 1, 2, 9, 4];

    let mut d = crate::algorithms::Replace::new(crate::algorithms::Capture::new());
    diff(&mut d, a, 0..a.len(), b, 0..b.len()).unwrap();
    insta::assert_debug_snapshot!(d.into_inner().ops());
}

#[test]
fn test_contiguous() {
    let a: &[usize] = &[0, 1, 2, 3, 4, 4, 4, 5];
    let b: &[usize] = &[0, 1, 2, 8, 9, 4, 4, 7];

    let mut d = crate::algorithms::Replace::new(crate::algorithms::Capture::new());
    diff(&mut d, a, 0..a.len(), b, 0..b.len()).unwrap();
    insta::assert_debug_snapshot!(d.into_inner().ops());
}

#[test]
fn test_pat() {
    let a: &[usize] = &[0, 1, 3, 4, 5];
    let b: &[usize] = &[0, 1, 4, 5, 8, 9];

    let mut d = crate::algorithms::Capture::new();
    diff(&mut d, a, 0..a.len(), b, 0..b.len()).unwrap();
    insta::assert_debug_snapshot!(d.ops());
}

#[test]
fn test_same() {
    let a: &[usize] = &[0, 1, 2, 3, 4, 4, 4, 5];
    let b: &[usize] = &[0, 1, 2, 3, 4, 4, 4, 5];

    let mut d = crate::algorithms::Capture::new();
    diff(&mut d, a, 0..a.len(), b, 0..b.len()).unwrap();
    insta::assert_debug_snapshot!(d.ops());
}

#[test]
fn test_finish_called() {
    struct HasRunFinish(bool);

    impl DiffHook for HasRunFinish {
        type Error = ();
        fn finish(&mut self) -> Result<(), Self::Error> {
            self.0 = true;
            Ok(())
        }
    }

    let mut d = HasRunFinish(false);
    let slice = &[1, 2];
    let slice2 = &[1, 2, 3];
    diff(&mut d, slice, 0..slice.len(), slice2, 0..slice2.len()).unwrap();
    assert!(d.0);

    let mut d = HasRunFinish(false);
    let slice = &[1, 2];
    diff(&mut d, slice, 0..slice.len(), slice, 0..slice.len()).unwrap();
    assert!(d.0);

    let mut d = HasRunFinish(false);
    let slice: &[u8] = &[];
    diff(&mut d, slice, 0..slice.len(), slice, 0..slice.len()).unwrap();
    assert!(d.0);
}

#[test]
fn test_bad_range_regression() {
    use crate::algorithms::Capture;
    use crate::DiffOp;
    let mut d = Capture::new();
    diff(&mut d, &[0], 0..1, &[0, 0], 0..2).unwrap();
    assert_eq!(
        d.into_ops(),
        vec![
            DiffOp::Equal {
                old_index: 0,
                new_index: 0,
                len: 1
            },
            DiffOp::Insert {
                old_index: 1,
                new_index: 1,
                new_len: 1
            }
        ]
    );
}
