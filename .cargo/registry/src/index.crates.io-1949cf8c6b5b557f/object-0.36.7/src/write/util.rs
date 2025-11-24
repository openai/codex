use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::{io, mem};

use crate::pod::{bytes_of, bytes_of_slice, Pod};

/// Trait for writable buffer.
#[allow(clippy::len_without_is_empty)]
pub trait WritableBuffer {
    /// Returns position/offset for data to be written at.
    ///
    /// Should only be used in debug assertions
    fn len(&self) -> usize;

    /// Reserves specified number of bytes in the buffer.
    ///
    /// This will be called exactly once before writing anything to the buffer,
    /// and the given size is the exact total number of bytes that will be written.
    fn reserve(&mut self, size: usize) -> Result<(), ()>;

    /// Writes zero bytes at the end of the buffer until the buffer
    /// has the specified length.
    fn resize(&mut self, new_len: usize);

    /// Writes the specified slice of bytes at the end of the buffer.
    fn write_bytes(&mut self, val: &[u8]);

    /// Writes the specified `Pod` type at the end of the buffer.
    fn write_pod<T: Pod>(&mut self, val: &T)
    where
        Self: Sized,
    {
        self.write_bytes(bytes_of(val))
    }

    /// Writes the specified `Pod` slice at the end of the buffer.
    fn write_pod_slice<T: Pod>(&mut self, val: &[T])
    where
        Self: Sized,
    {
        self.write_bytes(bytes_of_slice(val))
    }
}

impl<'a> dyn WritableBuffer + 'a {
    /// Writes the specified `Pod` type at the end of the buffer.
    pub fn write<T: Pod>(&mut self, val: &T) {
        self.write_bytes(bytes_of(val))
    }

    /// Writes the specified `Pod` slice at the end of the buffer.
    pub fn write_slice<T: Pod>(&mut self, val: &[T]) {
        self.write_bytes(bytes_of_slice(val))
    }
}

impl WritableBuffer for Vec<u8> {
    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn reserve(&mut self, size: usize) -> Result<(), ()> {
        debug_assert!(self.is_empty());
        self.reserve(size);
        Ok(())
    }

    #[inline]
    fn resize(&mut self, new_len: usize) {
        debug_assert!(new_len >= self.len());
        self.resize(new_len, 0);
    }

    #[inline]
    fn write_bytes(&mut self, val: &[u8]) {
        debug_assert!(self.len() + val.len() <= self.capacity());
        self.extend_from_slice(val)
    }
}

/// A [`WritableBuffer`] that streams data to a [`Write`](std::io::Write) implementation.
///
/// [`Self::result`] must be called to determine if an I/O error occurred during writing.
///
/// It is advisable to use a buffered writer like [`BufWriter`](std::io::BufWriter)
/// instead of an unbuffered writer like [`File`](std::fs::File).
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct StreamingBuffer<W> {
    writer: W,
    len: usize,
    result: Result<(), io::Error>,
}

#[cfg(feature = "std")]
impl<W> StreamingBuffer<W> {
    /// Create a new `StreamingBuffer` backed by the given writer.
    pub fn new(writer: W) -> Self {
        StreamingBuffer {
            writer,
            len: 0,
            result: Ok(()),
        }
    }

    /// Unwraps this [`StreamingBuffer`] giving back the original writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Returns any error that occurred during writing.
    pub fn result(&mut self) -> Result<(), io::Error> {
        mem::replace(&mut self.result, Ok(()))
    }
}

#[cfg(feature = "std")]
impl<W: io::Write> WritableBuffer for StreamingBuffer<W> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }

    #[inline]
    fn reserve(&mut self, _size: usize) -> Result<(), ()> {
        Ok(())
    }

    #[inline]
    fn resize(&mut self, new_len: usize) {
        debug_assert!(self.len <= new_len);
        while self.len < new_len {
            let write_amt = (new_len - self.len - 1) % 1024 + 1;
            self.write_bytes(&[0; 1024][..write_amt]);
        }
    }

    #[inline]
    fn write_bytes(&mut self, val: &[u8]) {
        if self.result.is_ok() {
            self.result = self.writer.write_all(val);
        }
        self.len += val.len();
    }
}

/// A trait for mutable byte slices.
///
/// It provides convenience methods for `Pod` types.
pub(crate) trait BytesMut {
    fn write_at<T: Pod>(self, offset: usize, val: &T) -> Result<(), ()>;
}

impl<'a> BytesMut for &'a mut [u8] {
    #[inline]
    fn write_at<T: Pod>(self, offset: usize, val: &T) -> Result<(), ()> {
        let src = bytes_of(val);
        let dest = self.get_mut(offset..).ok_or(())?;
        let dest = dest.get_mut(..src.len()).ok_or(())?;
        dest.copy_from_slice(src);
        Ok(())
    }
}

/// Write an unsigned number using the LEB128 encoding to a buffer.
///
/// Returns the number of bytes written.
#[allow(dead_code)]
pub(crate) fn write_uleb128(buf: &mut Vec<u8>, mut val: u64) -> usize {
    let mut len = 0;
    loop {
        let mut byte = (val & 0x7f) as u8;
        val >>= 7;
        let done = val == 0;
        if !done {
            byte |= 0x80;
        }

        buf.push(byte);
        len += 1;

        if done {
            return len;
        }
    }
}

/// Write a signed number using the LEB128 encoding to a buffer.
///
/// Returns the number of bytes written.
#[allow(dead_code)]
pub(crate) fn write_sleb128(buf: &mut Vec<u8>, mut val: i64) -> usize {
    let mut len = 0;
    loop {
        let mut byte = val as u8;
        // Keep the sign bit for testing
        val >>= 6;
        let done = val == 0 || val == -1;
        if done {
            byte &= !0x80;
        } else {
            // Remove the sign bit
            val >>= 1;
            byte |= 0x80;
        }

        buf.push(byte);
        len += 1;

        if done {
            return len;
        }
    }
}

pub(crate) fn align(offset: usize, size: usize) -> usize {
    (offset + (size - 1)) & !(size - 1)
}

#[allow(dead_code)]
pub(crate) fn align_u32(offset: u32, size: u32) -> u32 {
    (offset + (size - 1)) & !(size - 1)
}

#[allow(dead_code)]
pub(crate) fn align_u64(offset: u64, size: u64) -> u64 {
    (offset + (size - 1)) & !(size - 1)
}

pub(crate) fn write_align(buffer: &mut dyn WritableBuffer, size: usize) {
    let new_len = align(buffer.len(), size);
    buffer.resize(new_len);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_mut() {
        let data = vec![0x01, 0x23, 0x45, 0x67];

        let mut bytes = data.clone();
        bytes.extend_from_slice(bytes_of(&u16::to_be(0x89ab)));
        assert_eq!(bytes, [0x01, 0x23, 0x45, 0x67, 0x89, 0xab]);

        let mut bytes = data.clone();
        assert_eq!(bytes.write_at(0, &u16::to_be(0x89ab)), Ok(()));
        assert_eq!(bytes, [0x89, 0xab, 0x45, 0x67]);

        let mut bytes = data.clone();
        assert_eq!(bytes.write_at(2, &u16::to_be(0x89ab)), Ok(()));
        assert_eq!(bytes, [0x01, 0x23, 0x89, 0xab]);

        assert_eq!(bytes.write_at(3, &u16::to_be(0x89ab)), Err(()));
        assert_eq!(bytes.write_at(4, &u16::to_be(0x89ab)), Err(()));
        assert_eq!([].write_at(0, &u32::to_be(0x89ab)), Err(()));
    }
}
