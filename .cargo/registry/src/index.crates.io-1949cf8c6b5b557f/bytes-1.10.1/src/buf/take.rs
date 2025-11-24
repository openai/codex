use crate::Buf;

use core::cmp;

#[cfg(feature = "std")]
use std::io::IoSlice;

/// A `Buf` adapter which limits the bytes read from an underlying buffer.
///
/// This struct is generally created by calling `take()` on `Buf`. See
/// documentation of [`take()`](Buf::take) for more details.
#[derive(Debug)]
pub struct Take<T> {
    inner: T,
    limit: usize,
}

pub fn new<T>(inner: T, limit: usize) -> Take<T> {
    Take { inner, limit }
}

impl<T> Take<T> {
    /// Consumes this `Take`, returning the underlying value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::{Buf, BufMut};
    ///
    /// let mut buf = b"hello world".take(2);
    /// let mut dst = vec![];
    ///
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"he"[..]);
    ///
    /// let mut buf = buf.into_inner();
    ///
    /// dst.clear();
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"llo world"[..]);
    /// ```
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying `Buf`.
    ///
    /// It is inadvisable to directly read from the underlying `Buf`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::Buf;
    ///
    /// let buf = b"hello world".take(2);
    ///
    /// assert_eq!(11, buf.get_ref().remaining());
    /// ```
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying `Buf`.
    ///
    /// It is inadvisable to directly read from the underlying `Buf`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::{Buf, BufMut};
    ///
    /// let mut buf = b"hello world".take(2);
    /// let mut dst = vec![];
    ///
    /// buf.get_mut().advance(2);
    ///
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"ll"[..]);
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the maximum number of bytes that can be read.
    ///
    /// # Note
    ///
    /// If the inner `Buf` has fewer bytes than indicated by this method then
    /// that is the actual number of available bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::Buf;
    ///
    /// let mut buf = b"hello world".take(2);
    ///
    /// assert_eq!(2, buf.limit());
    /// assert_eq!(b'h', buf.get_u8());
    /// assert_eq!(1, buf.limit());
    /// ```
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Sets the maximum number of bytes that can be read.
    ///
    /// # Note
    ///
    /// If the inner `Buf` has fewer bytes than `lim` then that is the actual
    /// number of available bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::{Buf, BufMut};
    ///
    /// let mut buf = b"hello world".take(2);
    /// let mut dst = vec![];
    ///
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"he"[..]);
    ///
    /// dst.clear();
    ///
    /// buf.set_limit(3);
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"llo"[..]);
    /// ```
    pub fn set_limit(&mut self, lim: usize) {
        self.limit = lim
    }
}

impl<T: Buf> Buf for Take<T> {
    fn remaining(&self) -> usize {
        cmp::min(self.inner.remaining(), self.limit)
    }

    fn chunk(&self) -> &[u8] {
        let bytes = self.inner.chunk();
        &bytes[..cmp::min(bytes.len(), self.limit)]
    }

    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.limit);
        self.inner.advance(cnt);
        self.limit -= cnt;
    }

    fn copy_to_bytes(&mut self, len: usize) -> crate::Bytes {
        assert!(len <= self.remaining(), "`len` greater than remaining");

        let r = self.inner.copy_to_bytes(len);
        self.limit -= len;
        r
    }

    #[cfg(feature = "std")]
    fn chunks_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        if self.limit == 0 {
            return 0;
        }

        const LEN: usize = 16;
        let mut slices: [IoSlice<'a>; LEN] = [
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
            IoSlice::new(&[]),
        ];

        let cnt = self
            .inner
            .chunks_vectored(&mut slices[..dst.len().min(LEN)]);
        let mut limit = self.limit;
        for (i, (dst, slice)) in dst[..cnt].iter_mut().zip(slices.iter()).enumerate() {
            if let Some(buf) = slice.get(..limit) {
                // SAFETY: We could do this safely with `IoSlice::advance` if we had a larger MSRV.
                let buf = unsafe { std::mem::transmute::<&[u8], &'a [u8]>(buf) };
                *dst = IoSlice::new(buf);
                return i + 1;
            } else {
                // SAFETY: We could do this safely with `IoSlice::advance` if we had a larger MSRV.
                let buf = unsafe { std::mem::transmute::<&[u8], &'a [u8]>(slice) };
                *dst = IoSlice::new(buf);
                limit -= slice.len();
            }
        }
        cnt
    }
}
