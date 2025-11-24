use core::ops::{Bound, Range, RangeBounds};

pub(crate) fn third<A, B, C>(t: (A, B, C)) -> C {
    t.2
}

pub(crate) fn simplify_range<R>(range: R, len: usize) -> Range<usize>
where
    R: RangeBounds<usize>,
{
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(&i) if i <= len => i,
        Bound::Excluded(&i) if i < len => i + 1,
        bound => panic!("range start {:?} should be <= length {}", bound, len),
    };
    let end = match range.end_bound() {
        Bound::Unbounded => len,
        Bound::Excluded(&i) if i <= len => i,
        Bound::Included(&i) if i < len => i + 1,
        bound => panic!("range end {:?} should be <= length {}", bound, len),
    };
    if start > end {
        panic!(
            "range start {:?} should be <= range end {:?}",
            range.start_bound(),
            range.end_bound()
        );
    }
    start..end
}
