use alloc::string::String;
use core::convert::TryInto;
use core::fmt;
use core::marker::PhantomData;

use crate::pod::{from_bytes, slice_from_bytes, Pod};
use crate::read::ReadRef;

/// A newtype for byte slices.
///
/// It has these important features:
/// - no methods that can panic, such as `Index`
/// - convenience methods for `Pod` types
/// - a useful `Debug` implementation
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Bytes<'data>(pub &'data [u8]);

impl<'data> fmt::Debug for Bytes<'data> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_list_bytes(self.0, fmt)
    }
}

impl<'data> Bytes<'data> {
    /// Return the length of the byte slice.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return true if the byte slice is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Skip over the given number of bytes at the start of the byte slice.
    ///
    /// Modifies the byte slice to start after the bytes.
    ///
    /// Returns an error if there are too few bytes.
    #[inline]
    pub fn skip(&mut self, offset: usize) -> Result<(), ()> {
        match self.0.get(offset..) {
            Some(tail) => {
                self.0 = tail;
                Ok(())
            }
            None => {
                self.0 = &[];
                Err(())
            }
        }
    }

    /// Return a reference to the given number of bytes at the start of the byte slice.
    ///
    /// Modifies the byte slice to start after the bytes.
    ///
    /// Returns an error if there are too few bytes.
    #[inline]
    pub fn read_bytes(&mut self, count: usize) -> Result<Bytes<'data>, ()> {
        match (self.0.get(..count), self.0.get(count..)) {
            (Some(head), Some(tail)) => {
                self.0 = tail;
                Ok(Bytes(head))
            }
            _ => {
                self.0 = &[];
                Err(())
            }
        }
    }

    /// Return a reference to the given number of bytes at the given offset of the byte slice.
    ///
    /// Returns an error if the offset is invalid or there are too few bytes.
    #[inline]
    pub fn read_bytes_at(mut self, offset: usize, count: usize) -> Result<Bytes<'data>, ()> {
        self.skip(offset)?;
        self.read_bytes(count)
    }

    /// Return a reference to a `Pod` struct at the start of the byte slice.
    ///
    /// Modifies the byte slice to start after the bytes.
    ///
    /// Returns an error if there are too few bytes or the slice is incorrectly aligned.
    #[inline]
    pub fn read<T: Pod>(&mut self) -> Result<&'data T, ()> {
        match from_bytes(self.0) {
            Ok((value, tail)) => {
                self.0 = tail;
                Ok(value)
            }
            Err(()) => {
                self.0 = &[];
                Err(())
            }
        }
    }

    /// Return a reference to a `Pod` struct at the given offset of the byte slice.
    ///
    /// Returns an error if there are too few bytes or the offset is incorrectly aligned.
    #[inline]
    pub fn read_at<T: Pod>(mut self, offset: usize) -> Result<&'data T, ()> {
        self.skip(offset)?;
        self.read()
    }

    /// Return a reference to a slice of `Pod` structs at the start of the byte slice.
    ///
    /// Modifies the byte slice to start after the bytes.
    ///
    /// Returns an error if there are too few bytes or the offset is incorrectly aligned.
    #[inline]
    pub fn read_slice<T: Pod>(&mut self, count: usize) -> Result<&'data [T], ()> {
        match slice_from_bytes(self.0, count) {
            Ok((value, tail)) => {
                self.0 = tail;
                Ok(value)
            }
            Err(()) => {
                self.0 = &[];
                Err(())
            }
        }
    }

    /// Return a reference to a slice of `Pod` structs at the given offset of the byte slice.
    ///
    /// Returns an error if there are too few bytes or the offset is incorrectly aligned.
    #[inline]
    pub fn read_slice_at<T: Pod>(mut self, offset: usize, count: usize) -> Result<&'data [T], ()> {
        self.skip(offset)?;
        self.read_slice(count)
    }

    /// Read a null terminated string.
    ///
    /// Does not assume any encoding.
    /// Reads past the null byte, but doesn't return it.
    #[inline]
    pub fn read_string(&mut self) -> Result<&'data [u8], ()> {
        match memchr::memchr(b'\0', self.0) {
            Some(null) => {
                // These will never fail.
                let bytes = self.read_bytes(null)?;
                self.skip(1)?;
                Ok(bytes.0)
            }
            None => {
                self.0 = &[];
                Err(())
            }
        }
    }

    /// Read a null terminated string at an offset.
    ///
    /// Does not assume any encoding. Does not return the null byte.
    #[inline]
    pub fn read_string_at(mut self, offset: usize) -> Result<&'data [u8], ()> {
        self.skip(offset)?;
        self.read_string()
    }

    /// Read an unsigned LEB128 number.
    pub fn read_uleb128(&mut self) -> Result<u64, ()> {
        let mut result = 0;
        let mut shift = 0;

        loop {
            let byte = *self.read::<u8>()?;
            if shift == 63 && byte != 0x00 && byte != 0x01 {
                return Err(());
            }
            result |= u64::from(byte & 0x7f) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
    }

    /// Read a signed LEB128 number.
    pub fn read_sleb128(&mut self) -> Result<i64, ()> {
        let mut result = 0;
        let mut shift = 0;

        loop {
            let byte = *self.read::<u8>()?;
            if shift == 63 && byte != 0x00 && byte != 0x7f {
                return Err(());
            }
            result |= i64::from(byte & 0x7f) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                if shift < 64 && (byte & 0x40) != 0 {
                    // Sign extend the result.
                    result |= !0 << shift;
                }
                return Ok(result);
            }
        }
    }
}

// Only for Debug impl of `Bytes`.
fn debug_list_bytes(bytes: &[u8], fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut list = fmt.debug_list();
    list.entries(bytes.iter().take(8).copied().map(DebugByte));
    if bytes.len() > 8 {
        list.entry(&DebugLen(bytes.len()));
    }
    list.finish()
}

struct DebugByte(u8);

impl fmt::Debug for DebugByte {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "0x{:02x}", self.0)
    }
}

struct DebugLen(usize);

impl fmt::Debug for DebugLen {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "...; {}", self.0)
    }
}

/// A newtype for byte strings.
///
/// For byte slices that are strings of an unknown encoding.
///
/// Provides a `Debug` implementation that interprets the bytes as UTF-8.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByteString<'data>(pub &'data [u8]);

impl<'data> fmt::Debug for ByteString<'data> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "\"{}\"", String::from_utf8_lossy(self.0))
    }
}

#[allow(dead_code)]
#[inline]
pub(crate) fn align(offset: usize, size: usize) -> usize {
    (offset + (size - 1)) & !(size - 1)
}

#[allow(dead_code)]
pub(crate) fn data_range(
    data: &[u8],
    data_address: u64,
    range_address: u64,
    size: u64,
) -> Option<&[u8]> {
    let offset = range_address.checked_sub(data_address)?;
    data.get(offset.try_into().ok()?..)?
        .get(..size.try_into().ok()?)
}

/// A table of zero-terminated strings.
///
/// This is used by most file formats for strings such as section names and symbol names.
#[derive(Debug, Clone, Copy)]
pub struct StringTable<'data, R = &'data [u8]>
where
    R: ReadRef<'data>,
{
    data: Option<R>,
    start: u64,
    end: u64,
    marker: PhantomData<&'data ()>,
}

impl<'data, R: ReadRef<'data>> StringTable<'data, R> {
    /// Interpret the given data as a string table.
    pub fn new(data: R, start: u64, end: u64) -> Self {
        StringTable {
            data: Some(data),
            start,
            end,
            marker: PhantomData,
        }
    }

    /// Return the string at the given offset.
    pub fn get(&self, offset: u32) -> Result<&'data [u8], ()> {
        match self.data {
            Some(data) => {
                let r_start = self.start.checked_add(offset.into()).ok_or(())?;
                data.read_bytes_at_until(r_start..self.end, 0)
            }
            None => Err(()),
        }
    }
}

impl<'data, R: ReadRef<'data>> Default for StringTable<'data, R> {
    fn default() -> Self {
        StringTable {
            data: None,
            start: 0,
            end: 0,
            marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pod::bytes_of;

    #[test]
    fn bytes() {
        let x = u32::to_be(0x0123_4567);
        let data = Bytes(bytes_of(&x));

        let mut bytes = data;
        assert_eq!(bytes.skip(0), Ok(()));
        assert_eq!(bytes, data);

        let mut bytes = data;
        assert_eq!(bytes.skip(4), Ok(()));
        assert_eq!(bytes, Bytes(&[]));

        let mut bytes = data;
        assert_eq!(bytes.skip(5), Err(()));
        assert_eq!(bytes, Bytes(&[]));

        let mut bytes = data;
        assert_eq!(bytes.read_bytes(0), Ok(Bytes(&[])));
        assert_eq!(bytes, data);

        let mut bytes = data;
        assert_eq!(bytes.read_bytes(4), Ok(data));
        assert_eq!(bytes, Bytes(&[]));

        let mut bytes = data;
        assert_eq!(bytes.read_bytes(5), Err(()));
        assert_eq!(bytes, Bytes(&[]));

        assert_eq!(data.read_bytes_at(0, 0), Ok(Bytes(&[])));
        assert_eq!(data.read_bytes_at(4, 0), Ok(Bytes(&[])));
        assert_eq!(data.read_bytes_at(0, 4), Ok(data));
        assert_eq!(data.read_bytes_at(1, 4), Err(()));

        let mut bytes = data;
        assert_eq!(bytes.read::<u16>(), Ok(&u16::to_be(0x0123)));
        assert_eq!(bytes, Bytes(&[0x45, 0x67]));
        assert_eq!(data.read_at::<u16>(2), Ok(&u16::to_be(0x4567)));
        assert_eq!(data.read_at::<u16>(3), Err(()));
        assert_eq!(data.read_at::<u16>(4), Err(()));

        let mut bytes = data;
        assert_eq!(bytes.read::<u32>(), Ok(&x));
        assert_eq!(bytes, Bytes(&[]));

        let mut bytes = data;
        assert_eq!(bytes.read::<u64>(), Err(()));
        assert_eq!(bytes, Bytes(&[]));

        let mut bytes = data;
        assert_eq!(bytes.read_slice::<u8>(0), Ok(&[][..]));
        assert_eq!(bytes, data);

        let mut bytes = data;
        assert_eq!(bytes.read_slice::<u8>(4), Ok(data.0));
        assert_eq!(bytes, Bytes(&[]));

        let mut bytes = data;
        assert_eq!(bytes.read_slice::<u8>(5), Err(()));
        assert_eq!(bytes, Bytes(&[]));

        assert_eq!(data.read_slice_at::<u8>(0, 0), Ok(&[][..]));
        assert_eq!(data.read_slice_at::<u8>(4, 0), Ok(&[][..]));
        assert_eq!(data.read_slice_at::<u8>(0, 4), Ok(data.0));
        assert_eq!(data.read_slice_at::<u8>(1, 4), Err(()));

        let data = Bytes(&[0x01, 0x02, 0x00, 0x04]);

        let mut bytes = data;
        assert_eq!(bytes.read_string(), Ok(&data.0[..2]));
        assert_eq!(bytes.0, &data.0[3..]);

        let mut bytes = data;
        bytes.skip(3).unwrap();
        assert_eq!(bytes.read_string(), Err(()));
        assert_eq!(bytes.0, &[]);

        assert_eq!(data.read_string_at(0), Ok(&data.0[..2]));
        assert_eq!(data.read_string_at(1), Ok(&data.0[1..2]));
        assert_eq!(data.read_string_at(2), Ok(&[][..]));
        assert_eq!(data.read_string_at(3), Err(()));
    }

    #[test]
    fn bytes_debug() {
        assert_eq!(format!("{:?}", Bytes(&[])), "[]");
        assert_eq!(format!("{:?}", Bytes(&[0x01])), "[0x01]");
        assert_eq!(
            format!(
                "{:?}",
                Bytes(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
            ),
            "[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]"
        );
        assert_eq!(
            format!(
                "{:?}",
                Bytes(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09])
            ),
            "[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, ...; 9]"
        );
    }
}
