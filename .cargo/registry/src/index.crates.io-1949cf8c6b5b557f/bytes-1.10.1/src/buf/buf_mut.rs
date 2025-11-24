use crate::buf::{limit, Chain, Limit, UninitSlice};
#[cfg(feature = "std")]
use crate::buf::{writer, Writer};
use crate::{panic_advance, panic_does_not_fit, TryGetError};

use core::{mem, ptr, usize};

use alloc::{boxed::Box, vec::Vec};

/// A trait for values that provide sequential write access to bytes.
///
/// Write bytes to a buffer
///
/// A buffer stores bytes in memory such that write operations are infallible.
/// The underlying storage may or may not be in contiguous memory. A `BufMut`
/// value is a cursor into the buffer. Writing to `BufMut` advances the cursor
/// position.
///
/// The simplest `BufMut` is a `Vec<u8>`.
///
/// ```
/// use bytes::BufMut;
///
/// let mut buf = vec![];
///
/// buf.put(&b"hello world"[..]);
///
/// assert_eq!(buf, b"hello world");
/// ```
pub unsafe trait BufMut {
    /// Returns the number of bytes that can be written from the current
    /// position until the end of the buffer is reached.
    ///
    /// This value is greater than or equal to the length of the slice returned
    /// by `chunk_mut()`.
    ///
    /// Writing to a `BufMut` may involve allocating more memory on the fly.
    /// Implementations may fail before reaching the number of bytes indicated
    /// by this method if they encounter an allocation failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut dst = [0; 10];
    /// let mut buf = &mut dst[..];
    ///
    /// let original_remaining = buf.remaining_mut();
    /// buf.put(&b"hello"[..]);
    ///
    /// assert_eq!(original_remaining - 5, buf.remaining_mut());
    /// ```
    ///
    /// # Implementer notes
    ///
    /// Implementations of `remaining_mut` should ensure that the return value
    /// does not change unless a call is made to `advance_mut` or any other
    /// function that is documented to change the `BufMut`'s current position.
    ///
    /// # Note
    ///
    /// `remaining_mut` may return value smaller than actual available space.
    fn remaining_mut(&self) -> usize;

    /// Advance the internal cursor of the BufMut
    ///
    /// The next call to `chunk_mut` will return a slice starting `cnt` bytes
    /// further into the underlying buffer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the next `cnt` bytes of `chunk` are
    /// initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = Vec::with_capacity(16);
    ///
    /// // Write some data
    /// buf.chunk_mut()[0..2].copy_from_slice(b"he");
    /// unsafe { buf.advance_mut(2) };
    ///
    /// // write more bytes
    /// buf.chunk_mut()[0..3].copy_from_slice(b"llo");
    ///
    /// unsafe { buf.advance_mut(3); }
    ///
    /// assert_eq!(5, buf.len());
    /// assert_eq!(buf, b"hello");
    /// ```
    ///
    /// # Panics
    ///
    /// This function **may** panic if `cnt > self.remaining_mut()`.
    ///
    /// # Implementer notes
    ///
    /// It is recommended for implementations of `advance_mut` to panic if
    /// `cnt > self.remaining_mut()`. If the implementation does not panic,
    /// the call must behave as if `cnt == self.remaining_mut()`.
    ///
    /// A call with `cnt == 0` should never panic and be a no-op.
    unsafe fn advance_mut(&mut self, cnt: usize);

    /// Returns true if there is space in `self` for more bytes.
    ///
    /// This is equivalent to `self.remaining_mut() != 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut dst = [0; 5];
    /// let mut buf = &mut dst[..];
    ///
    /// assert!(buf.has_remaining_mut());
    ///
    /// buf.put(&b"hello"[..]);
    ///
    /// assert!(!buf.has_remaining_mut());
    /// ```
    #[inline]
    fn has_remaining_mut(&self) -> bool {
        self.remaining_mut() > 0
    }

    /// Returns a mutable slice starting at the current BufMut position and of
    /// length between 0 and `BufMut::remaining_mut()`. Note that this *can* be shorter than the
    /// whole remainder of the buffer (this allows non-continuous implementation).
    ///
    /// This is a lower level function. Most operations are done with other
    /// functions.
    ///
    /// The returned byte slice may represent uninitialized memory.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = Vec::with_capacity(16);
    ///
    /// unsafe {
    ///     // MaybeUninit::as_mut_ptr
    ///     buf.chunk_mut()[0..].as_mut_ptr().write(b'h');
    ///     buf.chunk_mut()[1..].as_mut_ptr().write(b'e');
    ///
    ///     buf.advance_mut(2);
    ///
    ///     buf.chunk_mut()[0..].as_mut_ptr().write(b'l');
    ///     buf.chunk_mut()[1..].as_mut_ptr().write(b'l');
    ///     buf.chunk_mut()[2..].as_mut_ptr().write(b'o');
    ///
    ///     buf.advance_mut(3);
    /// }
    ///
    /// assert_eq!(5, buf.len());
    /// assert_eq!(buf, b"hello");
    /// ```
    ///
    /// # Implementer notes
    ///
    /// This function should never panic. `chunk_mut()` should return an empty
    /// slice **if and only if** `remaining_mut()` returns 0. In other words,
    /// `chunk_mut()` returning an empty slice implies that `remaining_mut()` will
    /// return 0 and `remaining_mut()` returning 0 implies that `chunk_mut()` will
    /// return an empty slice.
    ///
    /// This function may trigger an out-of-memory abort if it tries to allocate
    /// memory and fails to do so.
    // The `chunk_mut` method was previously called `bytes_mut`. This alias makes the
    // rename more easily discoverable.
    #[cfg_attr(docsrs, doc(alias = "bytes_mut"))]
    fn chunk_mut(&mut self) -> &mut UninitSlice;

    /// Transfer bytes into `self` from `src` and advance the cursor by the
    /// number of bytes written.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    ///
    /// buf.put_u8(b'h');
    /// buf.put(&b"ello"[..]);
    /// buf.put(&b" world"[..]);
    ///
    /// assert_eq!(buf, b"hello world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `self` does not have enough capacity to contain `src`.
    #[inline]
    fn put<T: super::Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        if self.remaining_mut() < src.remaining() {
            panic_advance(&TryGetError {
                requested: src.remaining(),
                available: self.remaining_mut(),
            });
        }

        while src.has_remaining() {
            let s = src.chunk();
            let d = self.chunk_mut();
            let cnt = usize::min(s.len(), d.len());

            d[..cnt].copy_from_slice(&s[..cnt]);

            // SAFETY: We just initialized `cnt` bytes in `self`.
            unsafe { self.advance_mut(cnt) };
            src.advance(cnt);
        }
    }

    /// Transfer bytes into `self` from `src` and advance the cursor by the
    /// number of bytes written.
    ///
    /// `self` must have enough remaining capacity to contain all of `src`.
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut dst = [0; 6];
    ///
    /// {
    ///     let mut buf = &mut dst[..];
    ///     buf.put_slice(b"hello");
    ///
    ///     assert_eq!(1, buf.remaining_mut());
    /// }
    ///
    /// assert_eq!(b"hello\0", &dst);
    /// ```
    #[inline]
    fn put_slice(&mut self, mut src: &[u8]) {
        if self.remaining_mut() < src.len() {
            panic_advance(&TryGetError {
                requested: src.len(),
                available: self.remaining_mut(),
            });
        }

        while !src.is_empty() {
            let dst = self.chunk_mut();
            let cnt = usize::min(src.len(), dst.len());

            dst[..cnt].copy_from_slice(&src[..cnt]);
            src = &src[cnt..];

            // SAFETY: We just initialized `cnt` bytes in `self`.
            unsafe { self.advance_mut(cnt) };
        }
    }

    /// Put `cnt` bytes `val` into `self`.
    ///
    /// Logically equivalent to calling `self.put_u8(val)` `cnt` times, but may work faster.
    ///
    /// `self` must have at least `cnt` remaining capacity.
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut dst = [0; 6];
    ///
    /// {
    ///     let mut buf = &mut dst[..];
    ///     buf.put_bytes(b'a', 4);
    ///
    ///     assert_eq!(2, buf.remaining_mut());
    /// }
    ///
    /// assert_eq!(b"aaaa\0\0", &dst);
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_bytes(&mut self, val: u8, mut cnt: usize) {
        if self.remaining_mut() < cnt {
            panic_advance(&TryGetError {
                requested: cnt,
                available: self.remaining_mut(),
            })
        }

        while cnt > 0 {
            let dst = self.chunk_mut();
            let dst_len = usize::min(dst.len(), cnt);
            // SAFETY: The pointer is valid for `dst_len <= dst.len()` bytes.
            unsafe { core::ptr::write_bytes(dst.as_mut_ptr(), val, dst_len) };
            // SAFETY: We just initialized `dst_len` bytes in `self`.
            unsafe { self.advance_mut(dst_len) };
            cnt -= dst_len;
        }
    }

    /// Writes an unsigned 8 bit integer to `self`.
    ///
    /// The current position is advanced by 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u8(0x01);
    /// assert_eq!(buf, b"\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u8(&mut self, n: u8) {
        let src = [n];
        self.put_slice(&src);
    }

    /// Writes a signed 8 bit integer to `self`.
    ///
    /// The current position is advanced by 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i8(0x01);
    /// assert_eq!(buf, b"\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i8(&mut self, n: i8) {
        let src = [n as u8];
        self.put_slice(&src)
    }

    /// Writes an unsigned 16 bit integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u16(0x0809);
    /// assert_eq!(buf, b"\x08\x09");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u16(&mut self, n: u16) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes an unsigned 16 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u16_le(0x0809);
    /// assert_eq!(buf, b"\x09\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u16_le(&mut self, n: u16) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes an unsigned 16 bit integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u16_ne(0x0809);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x08\x09");
    /// } else {
    ///     assert_eq!(buf, b"\x09\x08");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u16_ne(&mut self, n: u16) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes a signed 16 bit integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i16(0x0809);
    /// assert_eq!(buf, b"\x08\x09");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i16(&mut self, n: i16) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes a signed 16 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i16_le(0x0809);
    /// assert_eq!(buf, b"\x09\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i16_le(&mut self, n: i16) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes a signed 16 bit integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i16_ne(0x0809);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x08\x09");
    /// } else {
    ///     assert_eq!(buf, b"\x09\x08");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i16_ne(&mut self, n: i16) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes an unsigned 32 bit integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u32(0x0809A0A1);
    /// assert_eq!(buf, b"\x08\x09\xA0\xA1");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u32(&mut self, n: u32) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes an unsigned 32 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u32_le(0x0809A0A1);
    /// assert_eq!(buf, b"\xA1\xA0\x09\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u32_le(&mut self, n: u32) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes an unsigned 32 bit integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u32_ne(0x0809A0A1);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x08\x09\xA0\xA1");
    /// } else {
    ///     assert_eq!(buf, b"\xA1\xA0\x09\x08");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u32_ne(&mut self, n: u32) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes a signed 32 bit integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i32(0x0809A0A1);
    /// assert_eq!(buf, b"\x08\x09\xA0\xA1");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i32(&mut self, n: i32) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes a signed 32 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i32_le(0x0809A0A1);
    /// assert_eq!(buf, b"\xA1\xA0\x09\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i32_le(&mut self, n: i32) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes a signed 32 bit integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i32_ne(0x0809A0A1);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x08\x09\xA0\xA1");
    /// } else {
    ///     assert_eq!(buf, b"\xA1\xA0\x09\x08");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i32_ne(&mut self, n: i32) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes an unsigned 64 bit integer to `self` in the big-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u64(0x0102030405060708);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u64(&mut self, n: u64) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes an unsigned 64 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u64_le(0x0102030405060708);
    /// assert_eq!(buf, b"\x08\x07\x06\x05\x04\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u64_le(&mut self, n: u64) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes an unsigned 64 bit integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u64_ne(0x0102030405060708);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08");
    /// } else {
    ///     assert_eq!(buf, b"\x08\x07\x06\x05\x04\x03\x02\x01");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u64_ne(&mut self, n: u64) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes a signed 64 bit integer to `self` in the big-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i64(0x0102030405060708);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i64(&mut self, n: i64) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes a signed 64 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i64_le(0x0102030405060708);
    /// assert_eq!(buf, b"\x08\x07\x06\x05\x04\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i64_le(&mut self, n: i64) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes a signed 64 bit integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i64_ne(0x0102030405060708);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08");
    /// } else {
    ///     assert_eq!(buf, b"\x08\x07\x06\x05\x04\x03\x02\x01");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i64_ne(&mut self, n: i64) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes an unsigned 128 bit integer to `self` in the big-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u128(0x01020304050607080910111213141516);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15\x16");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u128(&mut self, n: u128) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes an unsigned 128 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u128_le(0x01020304050607080910111213141516);
    /// assert_eq!(buf, b"\x16\x15\x14\x13\x12\x11\x10\x09\x08\x07\x06\x05\x04\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u128_le(&mut self, n: u128) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes an unsigned 128 bit integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_u128_ne(0x01020304050607080910111213141516);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15\x16");
    /// } else {
    ///     assert_eq!(buf, b"\x16\x15\x14\x13\x12\x11\x10\x09\x08\x07\x06\x05\x04\x03\x02\x01");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_u128_ne(&mut self, n: u128) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes a signed 128 bit integer to `self` in the big-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i128(0x01020304050607080910111213141516);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15\x16");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i128(&mut self, n: i128) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Writes a signed 128 bit integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i128_le(0x01020304050607080910111213141516);
    /// assert_eq!(buf, b"\x16\x15\x14\x13\x12\x11\x10\x09\x08\x07\x06\x05\x04\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i128_le(&mut self, n: i128) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Writes a signed 128 bit integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by 16.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_i128_ne(0x01020304050607080910111213141516);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15\x16");
    /// } else {
    ///     assert_eq!(buf, b"\x16\x15\x14\x13\x12\x11\x10\x09\x08\x07\x06\x05\x04\x03\x02\x01");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_i128_ne(&mut self, n: i128) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes an unsigned n-byte integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_uint(0x010203, 3);
    /// assert_eq!(buf, b"\x01\x02\x03");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self` or if `nbytes` is greater than 8.
    #[inline]
    fn put_uint(&mut self, n: u64, nbytes: usize) {
        let start = match mem::size_of_val(&n).checked_sub(nbytes) {
            Some(start) => start,
            None => panic_does_not_fit(nbytes, mem::size_of_val(&n)),
        };

        self.put_slice(&n.to_be_bytes()[start..]);
    }

    /// Writes an unsigned n-byte integer to `self` in the little-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_uint_le(0x010203, 3);
    /// assert_eq!(buf, b"\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self` or if `nbytes` is greater than 8.
    #[inline]
    fn put_uint_le(&mut self, n: u64, nbytes: usize) {
        let slice = n.to_le_bytes();
        let slice = match slice.get(..nbytes) {
            Some(slice) => slice,
            None => panic_does_not_fit(nbytes, slice.len()),
        };

        self.put_slice(slice);
    }

    /// Writes an unsigned n-byte integer to `self` in the native-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_uint_ne(0x010203, 3);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x01\x02\x03");
    /// } else {
    ///     assert_eq!(buf, b"\x03\x02\x01");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self` or if `nbytes` is greater than 8.
    #[inline]
    fn put_uint_ne(&mut self, n: u64, nbytes: usize) {
        if cfg!(target_endian = "big") {
            self.put_uint(n, nbytes)
        } else {
            self.put_uint_le(n, nbytes)
        }
    }

    /// Writes low `nbytes` of a signed integer to `self` in big-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_int(0x0504010203, 3);
    /// assert_eq!(buf, b"\x01\x02\x03");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self` or if `nbytes` is greater than 8.
    #[inline]
    fn put_int(&mut self, n: i64, nbytes: usize) {
        let start = match mem::size_of_val(&n).checked_sub(nbytes) {
            Some(start) => start,
            None => panic_does_not_fit(nbytes, mem::size_of_val(&n)),
        };

        self.put_slice(&n.to_be_bytes()[start..]);
    }

    /// Writes low `nbytes` of a signed integer to `self` in little-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_int_le(0x0504010203, 3);
    /// assert_eq!(buf, b"\x03\x02\x01");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self` or if `nbytes` is greater than 8.
    #[inline]
    fn put_int_le(&mut self, n: i64, nbytes: usize) {
        let slice = n.to_le_bytes();
        let slice = match slice.get(..nbytes) {
            Some(slice) => slice,
            None => panic_does_not_fit(nbytes, slice.len()),
        };

        self.put_slice(slice);
    }

    /// Writes low `nbytes` of a signed integer to `self` in native-endian byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_int_ne(0x010203, 3);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x01\x02\x03");
    /// } else {
    ///     assert_eq!(buf, b"\x03\x02\x01");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self` or if `nbytes` is greater than 8.
    #[inline]
    fn put_int_ne(&mut self, n: i64, nbytes: usize) {
        if cfg!(target_endian = "big") {
            self.put_int(n, nbytes)
        } else {
            self.put_int_le(n, nbytes)
        }
    }

    /// Writes an IEEE754 single-precision (4 bytes) floating point number to
    /// `self` in big-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f32(1.2f32);
    /// assert_eq!(buf, b"\x3F\x99\x99\x9A");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_f32(&mut self, n: f32) {
        self.put_u32(n.to_bits());
    }

    /// Writes an IEEE754 single-precision (4 bytes) floating point number to
    /// `self` in little-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f32_le(1.2f32);
    /// assert_eq!(buf, b"\x9A\x99\x99\x3F");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_f32_le(&mut self, n: f32) {
        self.put_u32_le(n.to_bits());
    }

    /// Writes an IEEE754 single-precision (4 bytes) floating point number to
    /// `self` in native-endian byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f32_ne(1.2f32);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x3F\x99\x99\x9A");
    /// } else {
    ///     assert_eq!(buf, b"\x9A\x99\x99\x3F");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_f32_ne(&mut self, n: f32) {
        self.put_u32_ne(n.to_bits());
    }

    /// Writes an IEEE754 double-precision (8 bytes) floating point number to
    /// `self` in big-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f64(1.2f64);
    /// assert_eq!(buf, b"\x3F\xF3\x33\x33\x33\x33\x33\x33");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_f64(&mut self, n: f64) {
        self.put_u64(n.to_bits());
    }

    /// Writes an IEEE754 double-precision (8 bytes) floating point number to
    /// `self` in little-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f64_le(1.2f64);
    /// assert_eq!(buf, b"\x33\x33\x33\x33\x33\x33\xF3\x3F");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_f64_le(&mut self, n: f64) {
        self.put_u64_le(n.to_bits());
    }

    /// Writes an IEEE754 double-precision (8 bytes) floating point number to
    /// `self` in native-endian byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    /// buf.put_f64_ne(1.2f64);
    /// if cfg!(target_endian = "big") {
    ///     assert_eq!(buf, b"\x3F\xF3\x33\x33\x33\x33\x33\x33");
    /// } else {
    ///     assert_eq!(buf, b"\x33\x33\x33\x33\x33\x33\xF3\x3F");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    #[inline]
    fn put_f64_ne(&mut self, n: f64) {
        self.put_u64_ne(n.to_bits());
    }

    /// Creates an adaptor which can write at most `limit` bytes to `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let arr = &mut [0u8; 128][..];
    /// assert_eq!(arr.remaining_mut(), 128);
    ///
    /// let dst = arr.limit(10);
    /// assert_eq!(dst.remaining_mut(), 10);
    /// ```
    #[inline]
    fn limit(self, limit: usize) -> Limit<Self>
    where
        Self: Sized,
    {
        limit::new(self, limit)
    }

    /// Creates an adaptor which implements the `Write` trait for `self`.
    ///
    /// This function returns a new value which implements `Write` by adapting
    /// the `Write` trait functions to the `BufMut` trait functions. Given that
    /// `BufMut` operations are infallible, none of the `Write` functions will
    /// return with `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    /// use std::io::Write;
    ///
    /// let mut buf = vec![].writer();
    ///
    /// let num = buf.write(&b"hello world"[..]).unwrap();
    /// assert_eq!(11, num);
    ///
    /// let buf = buf.into_inner();
    ///
    /// assert_eq!(*buf, b"hello world"[..]);
    /// ```
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[inline]
    fn writer(self) -> Writer<Self>
    where
        Self: Sized,
    {
        writer::new(self)
    }

    /// Creates an adapter which will chain this buffer with another.
    ///
    /// The returned `BufMut` instance will first write to all bytes from
    /// `self`. Afterwards, it will write to `next`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut a = [0u8; 5];
    /// let mut b = [0u8; 6];
    ///
    /// let mut chain = (&mut a[..]).chain_mut(&mut b[..]);
    ///
    /// chain.put_slice(b"hello world");
    ///
    /// assert_eq!(&a[..], b"hello");
    /// assert_eq!(&b[..], b" world");
    /// ```
    #[inline]
    fn chain_mut<U: BufMut>(self, next: U) -> Chain<Self, U>
    where
        Self: Sized,
    {
        Chain::new(self, next)
    }
}

macro_rules! deref_forward_bufmut {
    () => {
        #[inline]
        fn remaining_mut(&self) -> usize {
            (**self).remaining_mut()
        }

        #[inline]
        fn chunk_mut(&mut self) -> &mut UninitSlice {
            (**self).chunk_mut()
        }

        #[inline]
        unsafe fn advance_mut(&mut self, cnt: usize) {
            (**self).advance_mut(cnt)
        }

        #[inline]
        fn put_slice(&mut self, src: &[u8]) {
            (**self).put_slice(src)
        }

        #[inline]
        fn put_u8(&mut self, n: u8) {
            (**self).put_u8(n)
        }

        #[inline]
        fn put_i8(&mut self, n: i8) {
            (**self).put_i8(n)
        }

        #[inline]
        fn put_u16(&mut self, n: u16) {
            (**self).put_u16(n)
        }

        #[inline]
        fn put_u16_le(&mut self, n: u16) {
            (**self).put_u16_le(n)
        }

        #[inline]
        fn put_u16_ne(&mut self, n: u16) {
            (**self).put_u16_ne(n)
        }

        #[inline]
        fn put_i16(&mut self, n: i16) {
            (**self).put_i16(n)
        }

        #[inline]
        fn put_i16_le(&mut self, n: i16) {
            (**self).put_i16_le(n)
        }

        #[inline]
        fn put_i16_ne(&mut self, n: i16) {
            (**self).put_i16_ne(n)
        }

        #[inline]
        fn put_u32(&mut self, n: u32) {
            (**self).put_u32(n)
        }

        #[inline]
        fn put_u32_le(&mut self, n: u32) {
            (**self).put_u32_le(n)
        }

        #[inline]
        fn put_u32_ne(&mut self, n: u32) {
            (**self).put_u32_ne(n)
        }

        #[inline]
        fn put_i32(&mut self, n: i32) {
            (**self).put_i32(n)
        }

        #[inline]
        fn put_i32_le(&mut self, n: i32) {
            (**self).put_i32_le(n)
        }

        #[inline]
        fn put_i32_ne(&mut self, n: i32) {
            (**self).put_i32_ne(n)
        }

        #[inline]
        fn put_u64(&mut self, n: u64) {
            (**self).put_u64(n)
        }

        #[inline]
        fn put_u64_le(&mut self, n: u64) {
            (**self).put_u64_le(n)
        }

        #[inline]
        fn put_u64_ne(&mut self, n: u64) {
            (**self).put_u64_ne(n)
        }

        #[inline]
        fn put_i64(&mut self, n: i64) {
            (**self).put_i64(n)
        }

        #[inline]
        fn put_i64_le(&mut self, n: i64) {
            (**self).put_i64_le(n)
        }

        #[inline]
        fn put_i64_ne(&mut self, n: i64) {
            (**self).put_i64_ne(n)
        }
    };
}

unsafe impl<T: BufMut + ?Sized> BufMut for &mut T {
    deref_forward_bufmut!();
}

unsafe impl<T: BufMut + ?Sized> BufMut for Box<T> {
    deref_forward_bufmut!();
}

unsafe impl BufMut for &mut [u8] {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        UninitSlice::new(self)
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        if self.len() < cnt {
            panic_advance(&TryGetError {
                requested: cnt,
                available: self.len(),
            });
        }

        // Lifetime dance taken from `impl Write for &mut [u8]`.
        let (_, b) = core::mem::replace(self, &mut []).split_at_mut(cnt);
        *self = b;
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        if self.len() < src.len() {
            panic_advance(&TryGetError {
                requested: src.len(),
                available: self.len(),
            });
        }

        self[..src.len()].copy_from_slice(src);
        // SAFETY: We just initialized `src.len()` bytes.
        unsafe { self.advance_mut(src.len()) };
    }

    #[inline]
    fn put_bytes(&mut self, val: u8, cnt: usize) {
        if self.len() < cnt {
            panic_advance(&TryGetError {
                requested: cnt,
                available: self.len(),
            });
        }

        // SAFETY: We just checked that the pointer is valid for `cnt` bytes.
        unsafe {
            ptr::write_bytes(self.as_mut_ptr(), val, cnt);
            self.advance_mut(cnt);
        }
    }
}

unsafe impl BufMut for &mut [core::mem::MaybeUninit<u8>] {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        UninitSlice::uninit(self)
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        if self.len() < cnt {
            panic_advance(&TryGetError {
                requested: cnt,
                available: self.len(),
            });
        }

        // Lifetime dance taken from `impl Write for &mut [u8]`.
        let (_, b) = core::mem::replace(self, &mut []).split_at_mut(cnt);
        *self = b;
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        if self.len() < src.len() {
            panic_advance(&TryGetError {
                requested: src.len(),
                available: self.len(),
            });
        }

        // SAFETY: We just checked that the pointer is valid for `src.len()` bytes.
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), self.as_mut_ptr().cast(), src.len());
            self.advance_mut(src.len());
        }
    }

    #[inline]
    fn put_bytes(&mut self, val: u8, cnt: usize) {
        if self.len() < cnt {
            panic_advance(&TryGetError {
                requested: cnt,
                available: self.len(),
            });
        }

        // SAFETY: We just checked that the pointer is valid for `cnt` bytes.
        unsafe {
            ptr::write_bytes(self.as_mut_ptr() as *mut u8, val, cnt);
            self.advance_mut(cnt);
        }
    }
}

unsafe impl BufMut for Vec<u8> {
    #[inline]
    fn remaining_mut(&self) -> usize {
        // A vector can never have more than isize::MAX bytes
        core::isize::MAX as usize - self.len()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        let len = self.len();
        let remaining = self.capacity() - len;

        if remaining < cnt {
            panic_advance(&TryGetError {
                requested: cnt,
                available: remaining,
            });
        }

        // Addition will not overflow since the sum is at most the capacity.
        self.set_len(len + cnt);
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        if self.capacity() == self.len() {
            self.reserve(64); // Grow the vec
        }

        let cap = self.capacity();
        let len = self.len();

        let ptr = self.as_mut_ptr();
        // SAFETY: Since `ptr` is valid for `cap` bytes, `ptr.add(len)` must be
        // valid for `cap - len` bytes. The subtraction will not underflow since
        // `len <= cap`.
        unsafe { UninitSlice::from_raw_parts_mut(ptr.add(len), cap - len) }
    }

    // Specialize these methods so they can skip checking `remaining_mut`
    // and `advance_mut`.
    #[inline]
    fn put<T: super::Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        // In case the src isn't contiguous, reserve upfront.
        self.reserve(src.remaining());

        while src.has_remaining() {
            let s = src.chunk();
            let l = s.len();
            self.extend_from_slice(s);
            src.advance(l);
        }
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }

    #[inline]
    fn put_bytes(&mut self, val: u8, cnt: usize) {
        // If the addition overflows, then the `resize` will fail.
        let new_len = self.len().saturating_add(cnt);
        self.resize(new_len, val);
    }
}

// The existence of this function makes the compiler catch if the BufMut
// trait is "object-safe" or not.
fn _assert_trait_object(_b: &dyn BufMut) {}
