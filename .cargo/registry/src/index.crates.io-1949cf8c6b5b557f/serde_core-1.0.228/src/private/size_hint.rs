#[cfg(any(feature = "std", feature = "alloc"))]
use crate::lib::*;

pub fn from_bounds<I>(iter: &I) -> Option<usize>
where
    I: Iterator,
{
    helper(iter.size_hint())
}

#[cfg(any(feature = "std", feature = "alloc"))]
pub fn cautious<Element>(hint: Option<usize>) -> usize {
    const MAX_PREALLOC_BYTES: usize = 1024 * 1024;

    if mem::size_of::<Element>() == 0 {
        0
    } else {
        cmp::min(
            hint.unwrap_or(0),
            MAX_PREALLOC_BYTES / mem::size_of::<Element>(),
        )
    }
}

fn helper(bounds: (usize, Option<usize>)) -> Option<usize> {
    match bounds {
        (lower, Some(upper)) if lower == upper => Some(upper),
        _ => None,
    }
}
