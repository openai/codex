#[cfg(feature = "read")]
use alloc::borrow::Cow;
use core::convert::TryInto;
use core::fmt::Debug;
use core::hash::Hash;
use core::ops::{Add, AddAssign, Sub};

use crate::common::Format;
use crate::endianity::Endianity;
use crate::leb128;
use crate::read::{Error, Result};

/// An identifier for an offset within a section reader.
///
/// This is used for error reporting. The meaning of this value is specific to
/// each reader implementation. The values should be chosen to be unique amongst
/// all readers. If values are not unique then errors may point to the wrong reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReaderOffsetId(pub u64);

/// A trait for offsets with a DWARF section.
///
/// This allows consumers to choose a size that is appropriate for their address space.
pub trait ReaderOffset:
    Debug + Copy + Eq + Ord + Hash + Add<Output = Self> + AddAssign + Sub<Output = Self>
{
    /// Convert a u8 to an offset.
    fn from_u8(offset: u8) -> Self;

    /// Convert a u16 to an offset.
    fn from_u16(offset: u16) -> Self;

    /// Convert an i16 to an offset.
    fn from_i16(offset: i16) -> Self;

    /// Convert a u32 to an offset.
    fn from_u32(offset: u32) -> Self;

    /// Convert a u64 to an offset.
    ///
    /// Returns `Error::UnsupportedOffset` if the value is too large.
    fn from_u64(offset: u64) -> Result<Self>;

    /// Convert an offset to a u64.
    fn into_u64(self) -> u64;

    /// Wrapping (modular) addition. Computes `self + other`.
    fn wrapping_add(self, other: Self) -> Self;

    /// Checked subtraction. Computes `self - other`.
    fn checked_sub(self, other: Self) -> Option<Self>;
}

impl ReaderOffset for u64 {
    #[inline]
    fn from_u8(offset: u8) -> Self {
        u64::from(offset)
    }

    #[inline]
    fn from_u16(offset: u16) -> Self {
        u64::from(offset)
    }

    #[inline]
    fn from_i16(offset: i16) -> Self {
        offset as u64
    }

    #[inline]
    fn from_u32(offset: u32) -> Self {
        u64::from(offset)
    }

    #[inline]
    fn from_u64(offset: u64) -> Result<Self> {
        Ok(offset)
    }

    #[inline]
    fn into_u64(self) -> u64 {
        self
    }

    #[inline]
    fn wrapping_add(self, other: Self) -> Self {
        self.wrapping_add(other)
    }

    #[inline]
    fn checked_sub(self, other: Self) -> Option<Self> {
        self.checked_sub(other)
    }
}

impl ReaderOffset for u32 {
    #[inline]
    fn from_u8(offset: u8) -> Self {
        u32::from(offset)
    }

    #[inline]
    fn from_u16(offset: u16) -> Self {
        u32::from(offset)
    }

    #[inline]
    fn from_i16(offset: i16) -> Self {
        offset as u32
    }

    #[inline]
    fn from_u32(offset: u32) -> Self {
        offset
    }

    #[inline]
    fn from_u64(offset64: u64) -> Result<Self> {
        let offset = offset64 as u32;
        if u64::from(offset) == offset64 {
            Ok(offset)
        } else {
            Err(Error::UnsupportedOffset)
        }
    }

    #[inline]
    fn into_u64(self) -> u64 {
        u64::from(self)
    }

    #[inline]
    fn wrapping_add(self, other: Self) -> Self {
        self.wrapping_add(other)
    }

    #[inline]
    fn checked_sub(self, other: Self) -> Option<Self> {
        self.checked_sub(other)
    }
}

impl ReaderOffset for usize {
    #[inline]
    fn from_u8(offset: u8) -> Self {
        offset as usize
    }

    #[inline]
    fn from_u16(offset: u16) -> Self {
        offset as usize
    }

    #[inline]
    fn from_i16(offset: i16) -> Self {
        offset as usize
    }

    #[inline]
    fn from_u32(offset: u32) -> Self {
        offset as usize
    }

    #[inline]
    fn from_u64(offset64: u64) -> Result<Self> {
        let offset = offset64 as usize;
        if offset as u64 == offset64 {
            Ok(offset)
        } else {
            Err(Error::UnsupportedOffset)
        }
    }

    #[inline]
    fn into_u64(self) -> u64 {
        self as u64
    }

    #[inline]
    fn wrapping_add(self, other: Self) -> Self {
        self.wrapping_add(other)
    }

    #[inline]
    fn checked_sub(self, other: Self) -> Option<Self> {
        self.checked_sub(other)
    }
}

/// A trait for addresses within a DWARF section.
///
/// Currently this is a simple extension trait for `u64`, but it may be expanded
/// in the future to support user-defined address types.
pub(crate) trait ReaderAddress: Sized {
    /// Add a length to an address of the given size.
    ///
    /// Returns an error for overflow.
    fn add_sized(self, length: u64, size: u8) -> Result<Self>;

    /// Add a length to an address of the given size.
    ///
    /// Wraps the result to the size of the address to allow for the possibility
    /// that the length is a negative value.
    fn wrapping_add_sized(self, length: u64, size: u8) -> Self;

    /// The all-ones value of an address of the given size.
    fn ones_sized(size: u8) -> Self;
}

impl ReaderAddress for u64 {
    #[inline]
    fn add_sized(self, length: u64, size: u8) -> Result<Self> {
        let address = self.checked_add(length).ok_or(Error::AddressOverflow)?;
        let mask = Self::ones_sized(size);
        if address & !mask != 0 {
            return Err(Error::AddressOverflow);
        }
        Ok(address)
    }

    #[inline]
    fn wrapping_add_sized(self, length: u64, size: u8) -> Self {
        let mask = Self::ones_sized(size);
        self.wrapping_add(length) & mask
    }

    #[inline]
    fn ones_sized(size: u8) -> Self {
        !0 >> (64 - size * 8)
    }
}

#[cfg(not(feature = "read"))]
pub(crate) mod seal_if_no_alloc {
    #[derive(Debug)]
    pub struct Sealed;
}

/// A trait for reading the data from a DWARF section.
///
/// All read operations advance the section offset of the reader
/// unless specified otherwise.
///
/// ## Choosing a `Reader` Implementation
///
/// `gimli` comes with a few different `Reader` implementations and lets you
/// choose the one that is right for your use case. A `Reader` is essentially a
/// view into the raw bytes that make up some DWARF, but this view might borrow
/// the underlying data or use reference counting ownership, and it might be
/// thread safe or not.
///
/// | Implementation    | Ownership         | Thread Safe | Notes |
/// |:------------------|:------------------|:------------|:------|
/// | [`EndianSlice`](./struct.EndianSlice.html)        | Borrowed          | Yes         | Fastest, but requires that all of your code work with borrows. |
/// | [`EndianRcSlice`](./struct.EndianRcSlice.html)    | Reference counted | No          | Shared ownership via reference counting, which alleviates the borrow restrictions of `EndianSlice` but imposes reference counting increments and decrements. Cannot be sent across threads, because the reference count is not atomic. |
/// | [`EndianArcSlice`](./struct.EndianArcSlice.html)  | Reference counted | Yes         | The same as `EndianRcSlice`, but uses atomic reference counting, and therefore reference counting operations are slower but `EndianArcSlice`s may be sent across threads. |
/// | [`EndianReader<T>`](./struct.EndianReader.html)   | Same as `T`       | Same as `T` | Escape hatch for easily defining your own type of `Reader`. |
pub trait Reader: Debug + Clone {
    /// The endianity of bytes that are read.
    type Endian: Endianity;

    /// The type used for offsets and lengths.
    type Offset: ReaderOffset;

    /// Return the endianity of bytes that are read.
    fn endian(&self) -> Self::Endian;

    /// Return the number of bytes remaining.
    fn len(&self) -> Self::Offset;

    /// Set the number of bytes remaining to zero.
    fn empty(&mut self);

    /// Set the number of bytes remaining to the specified length.
    fn truncate(&mut self, len: Self::Offset) -> Result<()>;

    /// Return the offset of this reader's data relative to the start of
    /// the given base reader's data.
    ///
    /// May panic if this reader's data is not contained within the given
    /// base reader's data.
    fn offset_from(&self, base: &Self) -> Self::Offset;

    /// Return an identifier for the current reader offset.
    fn offset_id(&self) -> ReaderOffsetId;

    /// Return the offset corresponding to the given `id` if
    /// it is associated with this reader.
    fn lookup_offset_id(&self, id: ReaderOffsetId) -> Option<Self::Offset>;

    /// Find the index of the first occurrence of the given byte.
    /// The offset of the reader is not changed.
    fn find(&self, byte: u8) -> Result<Self::Offset>;

    /// Discard the specified number of bytes.
    fn skip(&mut self, len: Self::Offset) -> Result<()>;

    /// Split a reader in two.
    ///
    /// A new reader is returned that can be used to read the next
    /// `len` bytes, and `self` is advanced so that it reads the remainder.
    fn split(&mut self, len: Self::Offset) -> Result<Self>;

    /// This trait cannot be implemented if "read" feature is not enabled.
    ///
    /// `Reader` trait has a few methods that depend on `alloc` crate.
    /// Disallowing `Reader` trait implementation prevents a crate that only depends on
    /// "read-core" from being broken if another crate depending on `gimli` enables
    /// "read" feature.
    #[cfg(not(feature = "read"))]
    fn cannot_implement() -> seal_if_no_alloc::Sealed;

    /// Return all remaining data as a clone-on-write slice.
    ///
    /// The slice will be borrowed where possible, but some readers may
    /// always return an owned vector.
    ///
    /// Does not advance the reader.
    #[cfg(feature = "read")]
    fn to_slice(&self) -> Result<Cow<'_, [u8]>>;

    /// Convert all remaining data to a clone-on-write string.
    ///
    /// The string will be borrowed where possible, but some readers may
    /// always return an owned string.
    ///
    /// Does not advance the reader.
    ///
    /// Returns an error if the data contains invalid characters.
    #[cfg(feature = "read")]
    fn to_string(&self) -> Result<Cow<'_, str>>;

    /// Convert all remaining data to a clone-on-write string, including invalid characters.
    ///
    /// The string will be borrowed where possible, but some readers may
    /// always return an owned string.
    ///
    /// Does not advance the reader.
    #[cfg(feature = "read")]
    fn to_string_lossy(&self) -> Result<Cow<'_, str>>;

    /// Read exactly `buf.len()` bytes into `buf`.
    fn read_slice(&mut self, buf: &mut [u8]) -> Result<()>;

    /// Read a u8 array.
    #[inline]
    fn read_u8_array<A>(&mut self) -> Result<A>
    where
        A: Sized + Default + AsMut<[u8]>,
    {
        let mut val = Default::default();
        self.read_slice(<A as AsMut<[u8]>>::as_mut(&mut val))?;
        Ok(val)
    }

    /// Return true if the number of bytes remaining is zero.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == Self::Offset::from_u8(0)
    }

    /// Read a u8.
    #[inline]
    fn read_u8(&mut self) -> Result<u8> {
        let a: [u8; 1] = self.read_u8_array()?;
        Ok(a[0])
    }

    /// Read an i8.
    #[inline]
    fn read_i8(&mut self) -> Result<i8> {
        let a: [u8; 1] = self.read_u8_array()?;
        Ok(a[0] as i8)
    }

    /// Read a u16.
    #[inline]
    fn read_u16(&mut self) -> Result<u16> {
        let a: [u8; 2] = self.read_u8_array()?;
        Ok(self.endian().read_u16(&a))
    }

    /// Read an i16.
    #[inline]
    fn read_i16(&mut self) -> Result<i16> {
        let a: [u8; 2] = self.read_u8_array()?;
        Ok(self.endian().read_i16(&a))
    }

    /// Read a u32.
    #[inline]
    fn read_u32(&mut self) -> Result<u32> {
        let a: [u8; 4] = self.read_u8_array()?;
        Ok(self.endian().read_u32(&a))
    }

    /// Read an i32.
    #[inline]
    fn read_i32(&mut self) -> Result<i32> {
        let a: [u8; 4] = self.read_u8_array()?;
        Ok(self.endian().read_i32(&a))
    }

    /// Read a u64.
    #[inline]
    fn read_u64(&mut self) -> Result<u64> {
        let a: [u8; 8] = self.read_u8_array()?;
        Ok(self.endian().read_u64(&a))
    }

    /// Read an i64.
    #[inline]
    fn read_i64(&mut self) -> Result<i64> {
        let a: [u8; 8] = self.read_u8_array()?;
        Ok(self.endian().read_i64(&a))
    }

    /// Read a f32.
    #[inline]
    fn read_f32(&mut self) -> Result<f32> {
        let a: [u8; 4] = self.read_u8_array()?;
        Ok(self.endian().read_f32(&a))
    }

    /// Read a f64.
    #[inline]
    fn read_f64(&mut self) -> Result<f64> {
        let a: [u8; 8] = self.read_u8_array()?;
        Ok(self.endian().read_f64(&a))
    }

    /// Read an unsigned n-bytes integer u64.
    ///
    /// # Panics
    ///
    /// Panics when nbytes < 1 or nbytes > 8
    #[inline]
    fn read_uint(&mut self, n: usize) -> Result<u64> {
        let mut buf = [0; 8];
        self.read_slice(&mut buf[..n])?;
        Ok(self.endian().read_uint(&buf[..n]))
    }

    /// Read a null-terminated slice, and return it (excluding the null).
    fn read_null_terminated_slice(&mut self) -> Result<Self> {
        let idx = self.find(0)?;
        let val = self.split(idx)?;
        self.skip(Self::Offset::from_u8(1))?;
        Ok(val)
    }

    /// Skip a LEB128 encoded integer.
    fn skip_leb128(&mut self) -> Result<()> {
        leb128::read::skip(self)
    }

    /// Read an unsigned LEB128 encoded integer.
    fn read_uleb128(&mut self) -> Result<u64> {
        leb128::read::unsigned(self)
    }

    /// Read an unsigned LEB128 encoded u32.
    fn read_uleb128_u32(&mut self) -> Result<u32> {
        leb128::read::unsigned(self)?
            .try_into()
            .map_err(|_| Error::BadUnsignedLeb128)
    }

    /// Read an unsigned LEB128 encoded u16.
    fn read_uleb128_u16(&mut self) -> Result<u16> {
        leb128::read::u16(self)
    }

    /// Read a signed LEB128 encoded integer.
    fn read_sleb128(&mut self) -> Result<i64> {
        leb128::read::signed(self)
    }

    /// Read an initial length field.
    ///
    /// This field is encoded as either a 32-bit length or
    /// a 64-bit length, and the returned `Format` indicates which.
    fn read_initial_length(&mut self) -> Result<(Self::Offset, Format)> {
        const MAX_DWARF_32_UNIT_LENGTH: u32 = 0xffff_fff0;
        const DWARF_64_INITIAL_UNIT_LENGTH: u32 = 0xffff_ffff;

        let val = self.read_u32()?;
        if val < MAX_DWARF_32_UNIT_LENGTH {
            Ok((Self::Offset::from_u32(val), Format::Dwarf32))
        } else if val == DWARF_64_INITIAL_UNIT_LENGTH {
            let val = self.read_u64().and_then(Self::Offset::from_u64)?;
            Ok((val, Format::Dwarf64))
        } else {
            Err(Error::UnknownReservedLength)
        }
    }

    /// Read a byte and validate it as an address size.
    fn read_address_size(&mut self) -> Result<u8> {
        let size = self.read_u8()?;
        match size {
            1 | 2 | 4 | 8 => Ok(size),
            _ => Err(Error::UnsupportedAddressSize(size)),
        }
    }

    /// Read an address-sized integer, and return it as a `u64`.
    fn read_address(&mut self, address_size: u8) -> Result<u64> {
        match address_size {
            1 => self.read_u8().map(u64::from),
            2 => self.read_u16().map(u64::from),
            4 => self.read_u32().map(u64::from),
            8 => self.read_u64(),
            otherwise => Err(Error::UnsupportedAddressSize(otherwise)),
        }
    }

    /// Parse a word-sized integer according to the DWARF format.
    ///
    /// These are always used to encode section offsets or lengths,
    /// and so have a type of `Self::Offset`.
    fn read_word(&mut self, format: Format) -> Result<Self::Offset> {
        match format {
            Format::Dwarf32 => self.read_u32().map(Self::Offset::from_u32),
            Format::Dwarf64 => self.read_u64().and_then(Self::Offset::from_u64),
        }
    }

    /// Parse a word-sized section length according to the DWARF format.
    #[inline]
    fn read_length(&mut self, format: Format) -> Result<Self::Offset> {
        self.read_word(format)
    }

    /// Parse a word-sized section offset according to the DWARF format.
    #[inline]
    fn read_offset(&mut self, format: Format) -> Result<Self::Offset> {
        self.read_word(format)
    }

    /// Parse a section offset of the given size.
    ///
    /// This is used for `DW_FORM_ref_addr` values in DWARF version 2.
    fn read_sized_offset(&mut self, size: u8) -> Result<Self::Offset> {
        match size {
            1 => self.read_u8().map(u64::from),
            2 => self.read_u16().map(u64::from),
            4 => self.read_u32().map(u64::from),
            8 => self.read_u64(),
            otherwise => Err(Error::UnsupportedOffsetSize(otherwise)),
        }
        .and_then(Self::Offset::from_u64)
    }
}
