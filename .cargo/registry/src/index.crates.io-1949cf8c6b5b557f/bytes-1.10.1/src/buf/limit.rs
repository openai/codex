use crate::buf::UninitSlice;
use crate::BufMut;

use core::cmp;

/// A `BufMut` adapter which limits the amount of bytes that can be written
/// to an underlying buffer.
#[derive(Debug)]
pub struct Limit<T> {
    inner: T,
    limit: usize,
}

pub(super) fn new<T>(inner: T, limit: usize) -> Limit<T> {
    Limit { inner, limit }
}

impl<T> Limit<T> {
    /// Consumes this `Limit`, returning the underlying value.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying `BufMut`.
    ///
    /// It is inadvisable to directly write to the underlying `BufMut`.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying `BufMut`.
    ///
    /// It is inadvisable to directly write to the underlying `BufMut`.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the maximum number of bytes that can be written
    ///
    /// # Note
    ///
    /// If the inner `BufMut` has fewer bytes than indicated by this method then
    /// that is the actual number of available bytes.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Sets the maximum number of bytes that can be written.
    ///
    /// # Note
    ///
    /// If the inner `BufMut` has fewer bytes than `lim` then that is the actual
    /// number of available bytes.
    pub fn set_limit(&mut self, lim: usize) {
        self.limit = lim
    }
}

unsafe impl<T: BufMut> BufMut for Limit<T> {
    fn remaining_mut(&self) -> usize {
        cmp::min(self.inner.remaining_mut(), self.limit)
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        let bytes = self.inner.chunk_mut();
        let end = cmp::min(bytes.len(), self.limit);
        &mut bytes[..end]
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        assert!(cnt <= self.limit);
        self.inner.advance_mut(cnt);
        self.limit -= cnt;
    }
}
