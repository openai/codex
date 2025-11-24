use std::ops::{
    RangeFull,
    RangeFrom,
    RangeTo,
    Range,
};

// Taken from https://github.com/bluss/odds/blob/master/src/range.rs.

/// **IndexRange** is implemented by Rust's built-in range types, produced
/// by range syntax like `..`, `a..`, `..b` or `c..d`.
pub trait IndexRange<T=usize> {
    #[inline]
    /// Start index (inclusive)
    fn start(&self) -> Option<T> { None }
    #[inline]
    /// End index (exclusive)
    fn end(&self) -> Option<T> { None }
}


impl<T> IndexRange<T> for RangeFull {}

impl<T: Copy> IndexRange<T> for RangeFrom<T> {
    #[inline]
    fn start(&self) -> Option<T> { Some(self.start) }
}

impl<T: Copy> IndexRange<T> for RangeTo<T> {
    #[inline]
    fn end(&self) -> Option<T> { Some(self.end) }
}

impl<T: Copy> IndexRange<T> for Range<T> {
    #[inline]
    fn start(&self) -> Option<T> { Some(self.start) }
    #[inline]
    fn end(&self) -> Option<T> { Some(self.end) }
}
