//! Working with byte slices that have an associated endianity.

#[cfg(feature = "read")]
use alloc::borrow::Cow;
#[cfg(feature = "read")]
use alloc::string::String;
use core::fmt;
use core::ops::{Deref, Range, RangeFrom, RangeTo};
use core::str;

use crate::endianity::Endianity;
use crate::read::{Error, Reader, ReaderOffsetId, Result};

/// A `&[u8]` slice with endianity metadata.
///
/// This implements the `Reader` trait, which is used for all reading of DWARF sections.
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EndianSlice<'input, Endian>
where
    Endian: Endianity,
{
    slice: &'input [u8],
    endian: Endian,
}

impl<'input, Endian> EndianSlice<'input, Endian>
where
    Endian: Endianity,
{
    /// Construct a new `EndianSlice` with the given slice and endianity.
    #[inline]
    pub fn new(slice: &'input [u8], endian: Endian) -> EndianSlice<'input, Endian> {
        EndianSlice { slice, endian }
    }

    /// Return a reference to the raw slice.
    #[inline]
    #[doc(hidden)]
    #[deprecated(note = "Method renamed to EndianSlice::slice; use that instead.")]
    pub fn buf(&self) -> &'input [u8] {
        self.slice
    }

    /// Return a reference to the raw slice.
    #[inline]
    pub fn slice(&self) -> &'input [u8] {
        self.slice
    }

    /// Split the slice in two at the given index, resulting in the tuple where
    /// the first item has range [0, idx), and the second has range [idx,
    /// len). Panics if the index is out of bounds.
    #[inline]
    pub fn split_at(
        &self,
        idx: usize,
    ) -> (EndianSlice<'input, Endian>, EndianSlice<'input, Endian>) {
        (self.range_to(..idx), self.range_from(idx..))
    }

    /// Find the first occurrence of a byte in the slice, and return its index.
    #[inline]
    pub fn find(&self, byte: u8) -> Option<usize> {
        self.slice.iter().position(|ch| *ch == byte)
    }

    /// Return the offset of the start of the slice relative to the start
    /// of the given slice.
    #[inline]
    pub fn offset_from(&self, base: EndianSlice<'input, Endian>) -> usize {
        let base_ptr = base.slice.as_ptr() as usize;
        let ptr = self.slice.as_ptr() as usize;
        debug_assert!(base_ptr <= ptr);
        debug_assert!(ptr + self.slice.len() <= base_ptr + base.slice.len());
        ptr - base_ptr
    }

    /// Converts the slice to a string using `str::from_utf8`.
    ///
    /// Returns an error if the slice contains invalid characters.
    #[inline]
    pub fn to_string(&self) -> Result<&'input str> {
        str::from_utf8(self.slice).map_err(|_| Error::BadUtf8)
    }

    /// Converts the slice to a string, including invalid characters,
    /// using `String::from_utf8_lossy`.
    #[cfg(feature = "read")]
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'input, str> {
        String::from_utf8_lossy(self.slice)
    }

    #[inline]
    fn read_slice(&mut self, len: usize) -> Result<&'input [u8]> {
        if self.slice.len() < len {
            Err(Error::UnexpectedEof(self.offset_id()))
        } else {
            let val = &self.slice[..len];
            self.slice = &self.slice[len..];
            Ok(val)
        }
    }
}

/// # Range Methods
///
/// Unfortunately, `std::ops::Index` *must* return a reference, so we can't
/// implement `Index<Range<usize>>` to return a new `EndianSlice` the way we would
/// like to. Instead, we abandon fancy indexing operators and have these plain
/// old methods.
impl<'input, Endian> EndianSlice<'input, Endian>
where
    Endian: Endianity,
{
    /// Take the given `start..end` range of the underlying slice and return a
    /// new `EndianSlice`.
    ///
    /// ```
    /// use gimli::{EndianSlice, LittleEndian};
    ///
    /// let slice = &[0x01, 0x02, 0x03, 0x04];
    /// let endian_slice = EndianSlice::new(slice, LittleEndian);
    /// assert_eq!(endian_slice.range(1..3),
    ///            EndianSlice::new(&slice[1..3], LittleEndian));
    /// ```
    pub fn range(&self, idx: Range<usize>) -> EndianSlice<'input, Endian> {
        EndianSlice {
            slice: &self.slice[idx],
            endian: self.endian,
        }
    }

    /// Take the given `start..` range of the underlying slice and return a new
    /// `EndianSlice`.
    ///
    /// ```
    /// use gimli::{EndianSlice, LittleEndian};
    ///
    /// let slice = &[0x01, 0x02, 0x03, 0x04];
    /// let endian_slice = EndianSlice::new(slice, LittleEndian);
    /// assert_eq!(endian_slice.range_from(2..),
    ///            EndianSlice::new(&slice[2..], LittleEndian));
    /// ```
    pub fn range_from(&self, idx: RangeFrom<usize>) -> EndianSlice<'input, Endian> {
        EndianSlice {
            slice: &self.slice[idx],
            endian: self.endian,
        }
    }

    /// Take the given `..end` range of the underlying slice and return a new
    /// `EndianSlice`.
    ///
    /// ```
    /// use gimli::{EndianSlice, LittleEndian};
    ///
    /// let slice = &[0x01, 0x02, 0x03, 0x04];
    /// let endian_slice = EndianSlice::new(slice, LittleEndian);
    /// assert_eq!(endian_slice.range_to(..3),
    ///            EndianSlice::new(&slice[..3], LittleEndian));
    /// ```
    pub fn range_to(&self, idx: RangeTo<usize>) -> EndianSlice<'input, Endian> {
        EndianSlice {
            slice: &self.slice[idx],
            endian: self.endian,
        }
    }
}

impl<'input, Endian> Deref for EndianSlice<'input, Endian>
where
    Endian: Endianity,
{
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl<'input, Endian: Endianity> fmt::Debug for EndianSlice<'input, Endian> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> core::result::Result<(), fmt::Error> {
        fmt.debug_tuple("EndianSlice")
            .field(&self.endian)
            .field(&DebugBytes(self.slice))
            .finish()
    }
}

struct DebugBytes<'input>(&'input [u8]);

impl<'input> core::fmt::Debug for DebugBytes<'input> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> core::result::Result<(), fmt::Error> {
        let mut list = fmt.debug_list();
        list.entries(self.0.iter().take(8).copied().map(DebugByte));
        if self.0.len() > 8 {
            list.entry(&DebugLen(self.0.len()));
        }
        list.finish()
    }
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

impl<'input, Endian> Reader for EndianSlice<'input, Endian>
where
    Endian: Endianity,
{
    type Endian = Endian;
    type Offset = usize;

    #[inline]
    fn endian(&self) -> Endian {
        self.endian
    }

    #[inline]
    fn len(&self) -> usize {
        self.slice.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    #[inline]
    fn empty(&mut self) {
        self.slice = &[];
    }

    #[inline]
    fn truncate(&mut self, len: usize) -> Result<()> {
        if self.slice.len() < len {
            Err(Error::UnexpectedEof(self.offset_id()))
        } else {
            self.slice = &self.slice[..len];
            Ok(())
        }
    }

    #[inline]
    fn offset_from(&self, base: &Self) -> usize {
        self.offset_from(*base)
    }

    #[inline]
    fn offset_id(&self) -> ReaderOffsetId {
        ReaderOffsetId(self.slice.as_ptr() as u64)
    }

    #[inline]
    fn lookup_offset_id(&self, id: ReaderOffsetId) -> Option<Self::Offset> {
        let id = id.0;
        let self_id = self.slice.as_ptr() as u64;
        let self_len = self.slice.len() as u64;
        if id >= self_id && id <= self_id + self_len {
            Some((id - self_id) as usize)
        } else {
            None
        }
    }

    #[inline]
    fn find(&self, byte: u8) -> Result<usize> {
        self.find(byte)
            .ok_or_else(|| Error::UnexpectedEof(self.offset_id()))
    }

    #[inline]
    fn skip(&mut self, len: usize) -> Result<()> {
        if self.slice.len() < len {
            Err(Error::UnexpectedEof(self.offset_id()))
        } else {
            self.slice = &self.slice[len..];
            Ok(())
        }
    }

    #[inline]
    fn split(&mut self, len: usize) -> Result<Self> {
        let slice = self.read_slice(len)?;
        Ok(EndianSlice::new(slice, self.endian))
    }

    #[cfg(not(feature = "read"))]
    fn cannot_implement() -> super::reader::seal_if_no_alloc::Sealed {
        super::reader::seal_if_no_alloc::Sealed
    }

    #[cfg(feature = "read")]
    #[inline]
    fn to_slice(&self) -> Result<Cow<'_, [u8]>> {
        Ok(self.slice.into())
    }

    #[cfg(feature = "read")]
    #[inline]
    fn to_string(&self) -> Result<Cow<'_, str>> {
        match str::from_utf8(self.slice) {
            Ok(s) => Ok(s.into()),
            _ => Err(Error::BadUtf8),
        }
    }

    #[cfg(feature = "read")]
    #[inline]
    fn to_string_lossy(&self) -> Result<Cow<'_, str>> {
        Ok(String::from_utf8_lossy(self.slice))
    }

    #[inline]
    fn read_slice(&mut self, buf: &mut [u8]) -> Result<()> {
        let slice = self.read_slice(buf.len())?;
        buf.copy_from_slice(slice);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endianity::NativeEndian;

    #[test]
    fn test_endian_slice_split_at() {
        let endian = NativeEndian;
        let slice = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        let eb = EndianSlice::new(slice, endian);
        assert_eq!(
            eb.split_at(3),
            (
                EndianSlice::new(&slice[..3], endian),
                EndianSlice::new(&slice[3..], endian)
            )
        );
    }

    #[test]
    #[should_panic]
    fn test_endian_slice_split_at_out_of_bounds() {
        let slice = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        let eb = EndianSlice::new(slice, NativeEndian);
        eb.split_at(30);
    }
}
