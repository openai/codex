#[cfg(feature = "read")]
use alloc::borrow::Cow;
use core::fmt::Debug;

use crate::common::Format;
use crate::read::{Reader, ReaderOffset, ReaderOffsetId, Result};

/// Trait for relocating addresses and offsets while reading a section.
pub trait Relocate<T: ReaderOffset = usize> {
    /// Relocate an address which was read from the given section offset.
    fn relocate_address(&self, offset: T, value: u64) -> Result<u64>;

    /// Relocate a value which was read from the given section offset.
    fn relocate_offset(&self, offset: T, value: T) -> Result<T>;
}

/// A `Reader` which applies relocations to addresses and offsets.
///
/// This is useful for reading sections which contain relocations,
/// such as those in a relocatable object file.
/// It is generally not used for reading sections in an executable file.
#[derive(Debug, Clone)]
pub struct RelocateReader<R: Reader<Offset = usize>, T: Relocate<R::Offset>> {
    section: R,
    reader: R,
    relocate: T,
}

impl<R, T> RelocateReader<R, T>
where
    R: Reader<Offset = usize>,
    T: Relocate<R::Offset>,
{
    /// Create a new `RelocateReader` which applies relocations to the given section reader.
    pub fn new(section: R, relocate: T) -> Self {
        let reader = section.clone();
        Self {
            section,
            reader,
            relocate,
        }
    }
}

impl<R, T> Reader for RelocateReader<R, T>
where
    R: Reader<Offset = usize>,
    T: Relocate<R::Offset> + Debug + Clone,
{
    type Endian = R::Endian;
    type Offset = R::Offset;

    fn read_address(&mut self, address_size: u8) -> Result<u64> {
        let offset = self.reader.offset_from(&self.section);
        let value = self.reader.read_address(address_size)?;
        self.relocate.relocate_address(offset, value)
    }

    fn read_offset(&mut self, format: Format) -> Result<R::Offset> {
        let offset = self.reader.offset_from(&self.section);
        let value = self.reader.read_offset(format)?;
        self.relocate.relocate_offset(offset, value)
    }

    fn read_sized_offset(&mut self, size: u8) -> Result<R::Offset> {
        let offset = self.reader.offset_from(&self.section);
        let value = self.reader.read_sized_offset(size)?;
        self.relocate.relocate_offset(offset, value)
    }

    #[inline]
    fn split(&mut self, len: Self::Offset) -> Result<Self> {
        let mut other = self.clone();
        other.reader.truncate(len)?;
        self.reader.skip(len)?;
        Ok(other)
    }

    // All remaining methods simply delegate to `self.reader`.

    #[inline]
    fn endian(&self) -> Self::Endian {
        self.reader.endian()
    }

    #[inline]
    fn len(&self) -> Self::Offset {
        self.reader.len()
    }

    #[inline]
    fn empty(&mut self) {
        self.reader.empty()
    }

    #[inline]
    fn truncate(&mut self, len: Self::Offset) -> Result<()> {
        self.reader.truncate(len)
    }

    #[inline]
    fn offset_from(&self, base: &Self) -> Self::Offset {
        self.reader.offset_from(&base.reader)
    }

    #[inline]
    fn offset_id(&self) -> ReaderOffsetId {
        self.reader.offset_id()
    }

    #[inline]
    fn lookup_offset_id(&self, id: ReaderOffsetId) -> Option<Self::Offset> {
        self.reader.lookup_offset_id(id)
    }

    #[inline]
    fn find(&self, byte: u8) -> Result<Self::Offset> {
        self.reader.find(byte)
    }

    #[inline]
    fn skip(&mut self, len: Self::Offset) -> Result<()> {
        self.reader.skip(len)
    }

    #[cfg(not(feature = "read"))]
    fn cannot_implement() -> super::reader::seal_if_no_alloc::Sealed {
        super::reader::seal_if_no_alloc::Sealed
    }

    #[cfg(feature = "read")]
    #[inline]
    fn to_slice(&self) -> Result<Cow<'_, [u8]>> {
        self.reader.to_slice()
    }

    #[cfg(feature = "read")]
    #[inline]
    fn to_string(&self) -> Result<Cow<'_, str>> {
        self.reader.to_string()
    }

    #[cfg(feature = "read")]
    #[inline]
    fn to_string_lossy(&self) -> Result<Cow<'_, str>> {
        self.reader.to_string_lossy()
    }

    #[inline]
    fn read_slice(&mut self, buf: &mut [u8]) -> Result<()> {
        self.reader.read_slice(buf)
    }
}
