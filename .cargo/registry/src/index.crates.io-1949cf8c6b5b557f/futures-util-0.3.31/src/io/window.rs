use std::ops::{Bound, Range, RangeBounds};

/// An owned window around an underlying buffer.
///
/// Normally slices work great for considering sub-portions of a buffer, but
/// unfortunately a slice is a *borrowed* type in Rust which has an associated
/// lifetime. When working with future and async I/O these lifetimes are not
/// always appropriate, and are sometimes difficult to store in tasks. This
/// type strives to fill this gap by providing an "owned slice" around an
/// underlying buffer of bytes.
///
/// A `Window<T>` wraps an underlying buffer, `T`, and has configurable
/// start/end indexes to alter the behavior of the `AsRef<[u8]>` implementation
/// that this type carries.
///
/// This type can be particularly useful when working with the `write_all`
/// combinator in this crate. Data can be sliced via `Window`, consumed by
/// `write_all`, and then earned back once the write operation finishes through
/// the `into_inner` method on this type.
#[derive(Debug)]
pub struct Window<T> {
    inner: T,
    range: Range<usize>,
}

impl<T: AsRef<[u8]>> Window<T> {
    /// Creates a new window around the buffer `t` defaulting to the entire
    /// slice.
    ///
    /// Further methods can be called on the returned `Window<T>` to alter the
    /// window into the data provided.
    pub fn new(t: T) -> Self {
        Self { range: 0..t.as_ref().len(), inner: t }
    }

    /// Gets a shared reference to the underlying buffer inside of this
    /// `Window`.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying buffer inside of this
    /// `Window`.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consumes this `Window`, returning the underlying buffer.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Returns the starting index of this window into the underlying buffer
    /// `T`.
    pub fn start(&self) -> usize {
        self.range.start
    }

    /// Returns the end index of this window into the underlying buffer
    /// `T`.
    pub fn end(&self) -> usize {
        self.range.end
    }

    /// Changes the range of this window to the range specified.
    ///
    /// # Panics
    ///
    /// This method will panic if `range` is out of bounds for the underlying
    /// slice or if [`start_bound()`] of `range` comes after the [`end_bound()`].
    ///
    /// [`start_bound()`]: std::ops::RangeBounds::start_bound
    /// [`end_bound()`]: std::ops::RangeBounds::end_bound
    pub fn set<R: RangeBounds<usize>>(&mut self, range: R) {
        let start = match range.start_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => *n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(n) => *n + 1,
            Bound::Excluded(n) => *n,
            Bound::Unbounded => self.inner.as_ref().len(),
        };

        assert!(end <= self.inner.as_ref().len());
        assert!(start <= end);

        self.range.start = start;
        self.range.end = end;
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Window<T> {
    fn as_ref(&self) -> &[u8] {
        &self.inner.as_ref()[self.range.start..self.range.end]
    }
}

impl<T: AsMut<[u8]>> AsMut<[u8]> for Window<T> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.inner.as_mut()[self.range.start..self.range.end]
    }
}
