use crate::range::{DiffRange, SliceLike};

// Walks through all edits and shifts them up and then down, trying to see if they run into similar
// edits which can be merged
#[allow(clippy::needless_lifetimes)]
pub fn compact<'a, 'b, T: ?Sized + SliceLike>(diffs: &mut Vec<DiffRange<'a, 'b, T>>) {
    // First attempt to compact all Deletions
    let mut pointer = 0;
    while let Some(&diff) = diffs.get(pointer) {
        if let DiffRange::Delete(_) = diff {
            pointer = shift_diff_up(diffs, pointer);
            pointer = shift_diff_down(diffs, pointer);
        }
        pointer += 1;
    }

    // TODO maybe able to merge these and do them in the same pass?
    // Then attempt to compact all Insertions
    let mut pointer = 0;
    while let Some(&diff) = diffs.get(pointer) {
        if let DiffRange::Insert(_) = diff {
            pointer = shift_diff_up(diffs, pointer);
            pointer = shift_diff_down(diffs, pointer);
        }
        pointer += 1;
    }
}

// Attempts to shift the Insertion or Deletion at location `pointer` as far upwards as possible.
#[allow(clippy::needless_lifetimes)]
fn shift_diff_up<'a, 'b, T: ?Sized + SliceLike>(
    diffs: &mut Vec<DiffRange<'a, 'b, T>>,
    mut pointer: usize,
) -> usize {
    while let Some(&prev_diff) = pointer.checked_sub(1).and_then(|idx| diffs.get(idx)) {
        match (diffs[pointer], prev_diff) {
            //
            // Shift Inserts Upwards
            //
            (DiffRange::Insert(this_diff), DiffRange::Equal(prev_diff1, _)) => {
                // check common suffix for the amount we can shift
                let suffix_len = this_diff.common_suffix_len(prev_diff1);
                if suffix_len != 0 {
                    if let Some(DiffRange::Equal(..)) = diffs.get(pointer + 1) {
                        diffs[pointer + 1].grow_up(suffix_len);
                    } else {
                        diffs.insert(
                            pointer + 1,
                            DiffRange::Equal(
                                prev_diff1.slice(prev_diff1.len() - suffix_len..),
                                this_diff.slice(this_diff.len() - suffix_len..),
                            ),
                        );
                    }
                    diffs[pointer].shift_up(suffix_len);
                    diffs[pointer - 1].shrink_back(suffix_len);

                    if diffs[pointer - 1].is_empty() {
                        diffs.remove(pointer - 1);
                        pointer -= 1;
                    }
                } else if diffs[pointer - 1].is_empty() {
                    diffs.remove(pointer - 1);
                    pointer -= 1;
                } else {
                    // We can't shift upwards anymore
                    break;
                }
            }

            //
            // Shift Deletions Upwards
            //
            (DiffRange::Delete(this_diff), DiffRange::Equal(_, prev_diff2)) => {
                // check common suffix for the amount we can shift
                let suffix_len = this_diff.common_suffix_len(prev_diff2);
                if suffix_len != 0 {
                    if let Some(DiffRange::Equal(..)) = diffs.get(pointer + 1) {
                        diffs[pointer + 1].grow_up(suffix_len);
                    } else {
                        diffs.insert(
                            pointer + 1,
                            DiffRange::Equal(
                                this_diff.slice(this_diff.len() - suffix_len..),
                                prev_diff2.slice(prev_diff2.len() - suffix_len..),
                            ),
                        );
                    }
                    diffs[pointer].shift_up(suffix_len);
                    diffs[pointer - 1].shrink_back(suffix_len);

                    if diffs[pointer - 1].is_empty() {
                        diffs.remove(pointer - 1);
                        pointer -= 1;
                    }
                } else if diffs[pointer - 1].is_empty() {
                    diffs.remove(pointer - 1);
                    pointer -= 1;
                } else {
                    // We can't shift upwards anymore
                    break;
                }
            }

            //
            // Swap the Delete and Insert
            //
            (DiffRange::Insert(_), DiffRange::Delete(_))
            | (DiffRange::Delete(_), DiffRange::Insert(_)) => {
                diffs.swap(pointer - 1, pointer);
                pointer -= 1;
            }

            //
            // Merge the two ranges
            //
            (this_diff @ DiffRange::Insert(_), DiffRange::Insert(_))
            | (this_diff @ DiffRange::Delete(_), DiffRange::Delete(_)) => {
                diffs[pointer - 1].grow_down(this_diff.len());
                diffs.remove(pointer);
                pointer -= 1;
            }

            _ => panic!("range to shift must be either Insert or Delete"),
        }
    }

    pointer
}

// Attempts to shift the Insertion or Deletion at location `pointer` as far downwards as possible.
#[allow(clippy::needless_lifetimes)]
fn shift_diff_down<'a, 'b, T: ?Sized + SliceLike>(
    diffs: &mut Vec<DiffRange<'a, 'b, T>>,
    mut pointer: usize,
) -> usize {
    while let Some(&next_diff) = pointer.checked_add(1).and_then(|idx| diffs.get(idx)) {
        match (diffs[pointer], next_diff) {
            //
            // Shift Insert Downward
            //
            (DiffRange::Insert(this_diff), DiffRange::Equal(next_diff1, _)) => {
                // check common prefix for the amoutn we can shift
                let prefix_len = this_diff.common_prefix_len(next_diff1);
                if prefix_len != 0 {
                    if let Some(DiffRange::Equal(..)) =
                        pointer.checked_sub(1).and_then(|idx| diffs.get(idx))
                    {
                        diffs[pointer - 1].grow_down(prefix_len);
                    } else {
                        diffs.insert(
                            pointer,
                            DiffRange::Equal(
                                next_diff1.slice(..prefix_len),
                                this_diff.slice(..prefix_len),
                            ),
                        );
                        pointer += 1;
                    }

                    diffs[pointer].shift_down(prefix_len);
                    diffs[pointer + 1].shrink_front(prefix_len);

                    if diffs[pointer + 1].is_empty() {
                        diffs.remove(pointer + 1);
                    }
                } else if diffs[pointer + 1].is_empty() {
                    diffs.remove(pointer + 1);
                } else {
                    // We can't shift downwards anymore
                    break;
                }
            }

            //
            // Shift Deletion Downward
            //
            (DiffRange::Delete(this_diff), DiffRange::Equal(_, next_diff2)) => {
                // check common prefix for the amoutn we can shift
                let prefix_len = this_diff.common_prefix_len(next_diff2);
                if prefix_len != 0 {
                    if let Some(DiffRange::Equal(..)) =
                        pointer.checked_sub(1).and_then(|idx| diffs.get(idx))
                    {
                        diffs[pointer - 1].grow_down(prefix_len);
                    } else {
                        diffs.insert(
                            pointer,
                            DiffRange::Equal(
                                this_diff.slice(..prefix_len),
                                next_diff2.slice(..prefix_len),
                            ),
                        );
                        pointer += 1;
                    }

                    diffs[pointer].shift_down(prefix_len);
                    diffs[pointer + 1].shrink_front(prefix_len);

                    if diffs[pointer + 1].is_empty() {
                        diffs.remove(pointer + 1);
                    }
                } else if diffs[pointer + 1].is_empty() {
                    diffs.remove(pointer + 1);
                } else {
                    // We can't shift downwards anymore
                    break;
                }
            }

            //
            // Swap the Delete and Insert
            //
            (DiffRange::Insert(_), DiffRange::Delete(_))
            | (DiffRange::Delete(_), DiffRange::Insert(_)) => {
                diffs.swap(pointer, pointer + 1);
                pointer += 1;
            }

            //
            // Merge the two ranges
            //
            (DiffRange::Insert(_), next_diff @ DiffRange::Insert(_))
            | (DiffRange::Delete(_), next_diff @ DiffRange::Delete(_)) => {
                diffs[pointer].grow_down(next_diff.len());
                diffs.remove(pointer + 1);
            }

            _ => panic!("range to shift must be either Insert or Delete"),
        }
    }

    pointer
}
