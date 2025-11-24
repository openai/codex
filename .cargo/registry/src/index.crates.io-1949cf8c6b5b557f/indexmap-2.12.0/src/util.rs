use core::ops::{Bound, Range, RangeBounds};

pub(crate) fn third<A, B, C>(t: (A, B, C)) -> C {
    t.2
}

#[track_caller]
pub(crate) fn simplify_range<R>(range: R, len: usize) -> Range<usize>
where
    R: RangeBounds<usize>,
{
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(&i) if i <= len => i,
        Bound::Excluded(&i) if i < len => i + 1,
        Bound::Included(i) | Bound::Excluded(i) => {
            panic!("range start index {i} out of range for slice of length {len}")
        }
    };
    let end = match range.end_bound() {
        Bound::Unbounded => len,
        Bound::Excluded(&i) if i <= len => i,
        Bound::Included(&i) if i < len => i + 1,
        Bound::Included(i) | Bound::Excluded(i) => {
            panic!("range end index {i} out of range for slice of length {len}")
        }
    };
    if start > end {
        panic!(
            "range start index {:?} should be <= range end index {:?}",
            range.start_bound(),
            range.end_bound()
        );
    }
    start..end
}

pub(crate) fn try_simplify_range<R>(range: R, len: usize) -> Option<Range<usize>>
where
    R: RangeBounds<usize>,
{
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(&i) if i <= len => i,
        Bound::Excluded(&i) if i < len => i + 1,
        _ => return None,
    };
    let end = match range.end_bound() {
        Bound::Unbounded => len,
        Bound::Excluded(&i) if i <= len => i,
        Bound::Included(&i) if i < len => i + 1,
        _ => return None,
    };
    if start > end {
        return None;
    }
    Some(start..end)
}

// Generic slice equality -- copied from the standard library but adding a custom comparator,
// allowing for our `Bucket` wrapper on either or both sides.
pub(crate) fn slice_eq<T, U>(left: &[T], right: &[U], eq: impl Fn(&T, &U) -> bool) -> bool {
    if left.len() != right.len() {
        return false;
    }

    // Implemented as explicit indexing rather
    // than zipped iterators for performance reasons.
    // See PR https://github.com/rust-lang/rust/pull/116846
    for i in 0..left.len() {
        // bound checks are optimized away
        if !eq(&left[i], &right[i]) {
            return false;
        }
    }

    true
}
