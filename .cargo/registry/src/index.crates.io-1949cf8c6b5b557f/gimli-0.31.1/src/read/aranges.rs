use crate::common::{DebugArangesOffset, DebugInfoOffset, Encoding, SectionId};
use crate::endianity::Endianity;
use crate::read::{
    EndianSlice, Error, Range, Reader, ReaderAddress, ReaderOffset, Result, Section,
};

/// The `DebugAranges` struct represents the DWARF address range information
/// found in the `.debug_aranges` section.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebugAranges<R> {
    section: R,
}

impl<'input, Endian> DebugAranges<EndianSlice<'input, Endian>>
where
    Endian: Endianity,
{
    /// Construct a new `DebugAranges` instance from the data in the `.debug_aranges`
    /// section.
    ///
    /// It is the caller's responsibility to read the `.debug_aranges` section and
    /// present it as a `&[u8]` slice. That means using some ELF loader on
    /// Linux, a Mach-O loader on macOS, etc.
    ///
    /// ```
    /// use gimli::{DebugAranges, LittleEndian};
    ///
    /// # let buf = [];
    /// # let read_debug_aranges_section = || &buf;
    /// let debug_aranges =
    ///     DebugAranges::new(read_debug_aranges_section(), LittleEndian);
    /// ```
    pub fn new(section: &'input [u8], endian: Endian) -> Self {
        DebugAranges {
            section: EndianSlice::new(section, endian),
        }
    }
}

impl<R: Reader> DebugAranges<R> {
    /// Iterate the sets of entries in the `.debug_aranges` section.
    ///
    /// Each set of entries belongs to a single unit.
    pub fn headers(&self) -> ArangeHeaderIter<R> {
        ArangeHeaderIter {
            input: self.section.clone(),
            offset: DebugArangesOffset(R::Offset::from_u8(0)),
        }
    }

    /// Get the header at the given offset.
    pub fn header(&self, offset: DebugArangesOffset<R::Offset>) -> Result<ArangeHeader<R>> {
        let mut input = self.section.clone();
        input.skip(offset.0)?;
        ArangeHeader::parse(&mut input, offset)
    }
}

impl<T> DebugAranges<T> {
    /// Create a `DebugAranges` section that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// Used by `DwarfSections::borrow`.
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F) -> DebugAranges<R>
    where
        F: FnMut(&'a T) -> R,
    {
        borrow(&self.section).into()
    }
}

impl<R> Section<R> for DebugAranges<R> {
    fn id() -> SectionId {
        SectionId::DebugAranges
    }

    fn reader(&self) -> &R {
        &self.section
    }
}

impl<R> From<R> for DebugAranges<R> {
    fn from(section: R) -> Self {
        DebugAranges { section }
    }
}

/// An iterator over the headers of a `.debug_aranges` section.
#[derive(Clone, Debug)]
pub struct ArangeHeaderIter<R: Reader> {
    input: R,
    offset: DebugArangesOffset<R::Offset>,
}

impl<R: Reader> ArangeHeaderIter<R> {
    /// Advance the iterator to the next header.
    pub fn next(&mut self) -> Result<Option<ArangeHeader<R>>> {
        if self.input.is_empty() {
            return Ok(None);
        }

        let len = self.input.len();
        match ArangeHeader::parse(&mut self.input, self.offset) {
            Ok(header) => {
                self.offset.0 += len - self.input.len();
                Ok(Some(header))
            }
            Err(e) => {
                self.input.empty();
                Err(e)
            }
        }
    }
}

#[cfg(feature = "fallible-iterator")]
impl<R: Reader> fallible_iterator::FallibleIterator for ArangeHeaderIter<R> {
    type Item = ArangeHeader<R>;
    type Error = Error;

    fn next(&mut self) -> ::core::result::Result<Option<Self::Item>, Self::Error> {
        ArangeHeaderIter::next(self)
    }
}

/// A header for a set of entries in the `.debug_arange` section.
///
/// These entries all belong to a single unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArangeHeader<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    offset: DebugArangesOffset<Offset>,
    encoding: Encoding,
    length: Offset,
    debug_info_offset: DebugInfoOffset<Offset>,
    entries: R,
}

impl<R, Offset> ArangeHeader<R, Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    fn parse(input: &mut R, offset: DebugArangesOffset<Offset>) -> Result<Self> {
        let (length, format) = input.read_initial_length()?;
        let mut rest = input.split(length)?;

        // Check the version. The DWARF 5 spec says that this is always 2, but version 3
        // has been observed in the wild, potentially due to a bug; see
        // https://github.com/gimli-rs/gimli/issues/559 for more information.
        // lldb allows versions 2 through 5, possibly by mistake.
        let version = rest.read_u16()?;
        if version != 2 && version != 3 {
            return Err(Error::UnknownVersion(u64::from(version)));
        }

        let debug_info_offset = rest.read_offset(format).map(DebugInfoOffset)?;
        let address_size = rest.read_address_size()?;
        let segment_size = rest.read_u8()?;
        if segment_size != 0 {
            return Err(Error::UnsupportedSegmentSize);
        }

        // unit_length + version + offset + address_size + segment_size
        let header_length = format.initial_length_size() + 2 + format.word_size() + 1 + 1;

        // The first tuple following the header in each set begins at an offset that is
        // a multiple of the size of a single tuple (that is, twice the size of an address).
        let tuple_length = address_size
            .checked_mul(2)
            .ok_or(Error::UnsupportedAddressSize(address_size))?;
        if tuple_length == 0 {
            return Err(Error::UnsupportedAddressSize(address_size));
        }
        let padding = if header_length % tuple_length == 0 {
            0
        } else {
            tuple_length - header_length % tuple_length
        };
        rest.skip(R::Offset::from_u8(padding))?;

        let encoding = Encoding {
            format,
            version,
            address_size,
        };
        Ok(ArangeHeader {
            offset,
            encoding,
            length,
            debug_info_offset,
            entries: rest,
        })
    }

    /// Return the offset of this header within the `.debug_aranges` section.
    #[inline]
    pub fn offset(&self) -> DebugArangesOffset<Offset> {
        self.offset
    }

    /// Return the length of this set of entries, including the header.
    #[inline]
    pub fn length(&self) -> Offset {
        self.length
    }

    /// Return the encoding parameters for this set of entries.
    #[inline]
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Return the offset into the .debug_info section for this set of arange entries.
    #[inline]
    pub fn debug_info_offset(&self) -> DebugInfoOffset<Offset> {
        self.debug_info_offset
    }

    /// Return the arange entries in this set.
    #[inline]
    pub fn entries(&self) -> ArangeEntryIter<R> {
        ArangeEntryIter {
            input: self.entries.clone(),
            encoding: self.encoding,
        }
    }
}

/// An iterator over the aranges from a `.debug_aranges` section.
///
/// Can be [used with
/// `FallibleIterator`](./index.html#using-with-fallibleiterator).
#[derive(Debug, Clone)]
pub struct ArangeEntryIter<R: Reader> {
    input: R,
    encoding: Encoding,
}

impl<R: Reader> ArangeEntryIter<R> {
    /// Advance the iterator and return the next arange.
    ///
    /// Returns the newly parsed arange as `Ok(Some(arange))`. Returns `Ok(None)`
    /// when iteration is complete and all aranges have already been parsed and
    /// yielded. If an error occurs while parsing the next arange, then this error
    /// is returned as `Err(e)`, and all subsequent calls return `Ok(None)`.
    pub fn next(&mut self) -> Result<Option<ArangeEntry>> {
        loop {
            let raw_entry = match self.next_raw()? {
                Some(entry) => entry,
                None => return Ok(None),
            };

            let entry = self.convert_raw(raw_entry)?;
            if entry.is_some() {
                return Ok(entry);
            }
        }
    }

    /// Advance the iterator and return the next arange without validating it.
    ///
    /// The returned entry will have `range.end` set to 0.
    /// This will return tombstone entries as well.
    pub fn next_raw(&mut self) -> Result<Option<ArangeEntry>> {
        if self.input.is_empty() {
            return Ok(None);
        }

        match ArangeEntry::parse(&mut self.input, self.encoding) {
            Ok(Some(entry)) => Ok(Some(entry)),
            Ok(None) => {
                self.input.empty();
                Ok(None)
            }
            Err(e) => {
                self.input.empty();
                Err(e)
            }
        }
    }

    /// Convert a raw range into a range.
    ///
    /// The raw range should have been obtained from `next_raw`.
    #[doc(hidden)]
    pub fn convert_raw(&self, mut entry: ArangeEntry) -> Result<Option<ArangeEntry>> {
        // Skip tombstone entries.
        // DWARF specifies a tombstone value of -1, but many linkers use 0.
        // However, 0 may be a valid address, so the caller must handle that case.
        let address_size = self.encoding.address_size;
        let tombstone_address = !0 >> (64 - self.encoding.address_size * 8);
        if entry.range.begin == tombstone_address {
            return Ok(None);
        }

        // Calculate end now so that we can handle overflow.
        entry.range.end = entry.range.begin.add_sized(entry.length, address_size)?;
        Ok(Some(entry))
    }
}

#[cfg(feature = "fallible-iterator")]
impl<R: Reader> fallible_iterator::FallibleIterator for ArangeEntryIter<R> {
    type Item = ArangeEntry;
    type Error = Error;

    fn next(&mut self) -> ::core::result::Result<Option<Self::Item>, Self::Error> {
        ArangeEntryIter::next(self)
    }
}

/// A single parsed arange.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArangeEntry {
    range: Range,
    length: u64,
}

impl ArangeEntry {
    /// Parse a single arange. Return `None` for the null arange, `Some` for an actual arange.
    fn parse<R: Reader>(input: &mut R, encoding: Encoding) -> Result<Option<Self>> {
        let address_size = encoding.address_size;

        let tuple_length = R::Offset::from_u8(2 * address_size);
        if tuple_length > input.len() {
            input.empty();
            return Ok(None);
        }

        let begin = input.read_address(address_size)?;
        let length = input.read_address(address_size)?;
        let range = Range { begin, end: 0 };

        match (begin, length) {
            // This is meant to be a null terminator, but in practice it can occur
            // before the end, possibly due to a linker omitting a function and
            // leaving an unrelocated entry.
            (0, 0) => Self::parse(input, encoding),
            _ => Ok(Some(ArangeEntry { range, length })),
        }
    }

    /// Return the beginning address of this arange.
    #[inline]
    pub fn address(&self) -> u64 {
        self.range.begin
    }

    /// Return the length of this arange.
    #[inline]
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Return the range.
    #[inline]
    pub fn range(&self) -> Range {
        self.range
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DebugInfoOffset, Format};
    use crate::endianity::LittleEndian;
    use crate::read::EndianSlice;

    #[test]
    fn test_iterate_headers() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 28.
            0x1c, 0x00, 0x00, 0x00,
            // Version.
            0x02, 0x00,
            // Offset.
            0x01, 0x02, 0x03, 0x04,
            // Address size.
            0x04,
            // Segment size.
            0x00,
            // Dummy padding and arange tuples.
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

            // 32-bit length = 36.
            0x24, 0x00, 0x00, 0x00,
            // Version.
            0x02, 0x00,
            // Offset.
            0x11, 0x12, 0x13, 0x14,
            // Address size.
            0x04,
            // Segment size.
            0x00,
            // Dummy padding and arange tuples.
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let debug_aranges = DebugAranges::new(&buf, LittleEndian);
        let mut headers = debug_aranges.headers();

        let header = headers
            .next()
            .expect("should parse header ok")
            .expect("should have a header");
        assert_eq!(header.offset(), DebugArangesOffset(0));
        assert_eq!(header.debug_info_offset(), DebugInfoOffset(0x0403_0201));

        let header = headers
            .next()
            .expect("should parse header ok")
            .expect("should have a header");
        assert_eq!(header.offset(), DebugArangesOffset(0x20));
        assert_eq!(header.debug_info_offset(), DebugInfoOffset(0x1413_1211));
    }

    #[test]
    fn test_parse_header_ok() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 28 (8 bytes header, 4 bytes padding, 16 bytes tuple data).
            0x1c, 0x00, 0x00, 0x00,
            // Version.
            0x02, 0x00,
            // Offset.
            0x01, 0x02, 0x03, 0x04,
            // Address size.
            0x08,
            // Segment size.
            0x00,
            // Length to here = 12, tuple length = 16.
            // Padding to tuple length multiple = 4.
            0x10, 0x00, 0x00, 0x00,

            // Dummy arange tuple data.
            0x20, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next arange.
            0x30, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let rest = &mut EndianSlice::new(&buf, LittleEndian);

        let header =
            ArangeHeader::parse(rest, DebugArangesOffset(0x10)).expect("should parse header ok");

        assert_eq!(
            *rest,
            EndianSlice::new(&buf[buf.len() - 16..], LittleEndian)
        );
        assert_eq!(
            header,
            ArangeHeader {
                offset: DebugArangesOffset(0x10),
                encoding: Encoding {
                    format: Format::Dwarf32,
                    version: 2,
                    address_size: 8,
                },
                length: 0x1c,
                debug_info_offset: DebugInfoOffset(0x0403_0201),
                entries: EndianSlice::new(&buf[buf.len() - 32..buf.len() - 16], LittleEndian),
            }
        );
    }

    #[test]
    fn test_parse_header_overflow_error() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 32.
            0x20, 0x00, 0x00, 0x00,
            // Version.
            0x02, 0x00,
            // Offset.
            0x01, 0x02, 0x03, 0x04,
            // Address size.
            0xff,
            // Segment size.
            0x00,
            // Length to here = 12, tuple length = 20.
            // Padding to tuple length multiple = 4.
            0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy arange tuple data.
            0x20, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next arange.
            0x30, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let rest = &mut EndianSlice::new(&buf, LittleEndian);

        let error = ArangeHeader::parse(rest, DebugArangesOffset(0x10))
            .expect_err("should fail to parse header");
        assert_eq!(error, Error::UnsupportedAddressSize(0xff));
    }

    #[test]
    fn test_parse_header_div_by_zero_error() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 32.
            0x20, 0x00, 0x00, 0x00,
            // Version.
            0x02, 0x00,
            // Offset.
            0x01, 0x02, 0x03, 0x04,
            // Address size = 0. Could cause a division by zero if we aren't
            // careful.
            0x00,
            // Segment size.
            0x00,
            // Length to here = 12, tuple length = 20.
            // Padding to tuple length multiple = 4.
            0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy arange tuple data.
            0x20, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next arange.
            0x30, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let rest = &mut EndianSlice::new(&buf, LittleEndian);

        let error = ArangeHeader::parse(rest, DebugArangesOffset(0x10))
            .expect_err("should fail to parse header");
        assert_eq!(error, Error::UnsupportedAddressSize(0));
    }

    #[test]
    fn test_parse_entry_ok() {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 2,
            address_size: 4,
        };
        let buf = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let mut iter = ArangeEntryIter {
            input: EndianSlice::new(&buf, LittleEndian),
            encoding,
        };
        let entry = iter.next().expect("should parse entry ok");
        assert_eq!(
            iter.input,
            EndianSlice::new(&buf[buf.len() - 1..], LittleEndian)
        );
        assert_eq!(
            entry,
            Some(ArangeEntry {
                range: Range {
                    begin: 0x0403_0201,
                    end: 0x0403_0201 + 0x0807_0605,
                },
                length: 0x0807_0605,
            })
        );
    }

    #[test]
    fn test_parse_entry_zero() {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 2,
            address_size: 4,
        };
        #[rustfmt::skip]
        let buf = [
            // Zero tuple.
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Address.
            0x01, 0x02, 0x03, 0x04,
            // Length.
            0x05, 0x06, 0x07, 0x08,
            // Next tuple.
            0x09
        ];
        let mut iter = ArangeEntryIter {
            input: EndianSlice::new(&buf, LittleEndian),
            encoding,
        };
        let entry = iter.next().expect("should parse entry ok");
        assert_eq!(
            iter.input,
            EndianSlice::new(&buf[buf.len() - 1..], LittleEndian)
        );
        assert_eq!(
            entry,
            Some(ArangeEntry {
                range: Range {
                    begin: 0x0403_0201,
                    end: 0x0403_0201 + 0x0807_0605,
                },
                length: 0x0807_0605,
            })
        );
    }

    #[test]
    fn test_parse_entry_overflow_32() {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 2,
            address_size: 4,
        };
        #[rustfmt::skip]
        let buf = [
            // Address.
            0x01, 0x02, 0x03, 0x84,
            // Length.
            0x05, 0x06, 0x07, 0x88,
            // Next tuple.
            0x09
        ];
        let mut iter = ArangeEntryIter {
            input: EndianSlice::new(&buf, LittleEndian),
            encoding,
        };
        let entry = iter.next();
        assert_eq!(
            iter.input,
            EndianSlice::new(&buf[buf.len() - 1..], LittleEndian)
        );
        assert_eq!(entry, Err(Error::AddressOverflow));
    }

    #[test]
    fn test_parse_entry_overflow_64() {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 2,
            address_size: 8,
        };
        #[rustfmt::skip]
        let buf = [
            // Address.
            0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x00, 0x80,
            // Length.
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x80,
            // Next tuple.
            0x09
        ];
        let mut iter = ArangeEntryIter {
            input: EndianSlice::new(&buf, LittleEndian),
            encoding,
        };
        let entry = iter.next();
        assert_eq!(
            iter.input,
            EndianSlice::new(&buf[buf.len() - 1..], LittleEndian)
        );
        assert_eq!(entry, Err(Error::AddressOverflow));
    }

    #[test]
    fn test_parse_entry_tombstone_32() {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 2,
            address_size: 4,
        };
        #[rustfmt::skip]
        let buf = [
            // Address.
            0xff, 0xff, 0xff, 0xff,
            // Length.
            0x05, 0x06, 0x07, 0x08,
            // Address.
            0x01, 0x02, 0x03, 0x04,
            // Length.
            0x05, 0x06, 0x07, 0x08,
            // Next tuple.
            0x09
        ];

        let mut iter = ArangeEntryIter {
            input: EndianSlice::new(&buf, LittleEndian),
            encoding,
        };
        let entry = iter.next_raw().unwrap();
        assert_eq!(
            iter.input,
            EndianSlice::new(&buf[buf.len() - 9..], LittleEndian)
        );
        assert_eq!(
            entry,
            Some(ArangeEntry {
                range: Range {
                    begin: 0xffff_ffff,
                    end: 0,
                },
                length: 0x0807_0605,
            })
        );

        let mut iter = ArangeEntryIter {
            input: EndianSlice::new(&buf, LittleEndian),
            encoding,
        };
        let entry = iter.next().unwrap();
        assert_eq!(
            iter.input,
            EndianSlice::new(&buf[buf.len() - 1..], LittleEndian)
        );
        assert_eq!(
            entry,
            Some(ArangeEntry {
                range: Range {
                    begin: 0x0403_0201,
                    end: 0x0403_0201 + 0x0807_0605,
                },
                length: 0x0807_0605,
            })
        );
    }

    #[test]
    fn test_parse_entry_tombstone_64() {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 2,
            address_size: 8,
        };
        #[rustfmt::skip]
        let buf = [
            // Address.
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            // Length.
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x00,
            // Address.
            0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00,
            // Length.
            0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x00,
            // Next tuple.
            0x09
        ];

        let mut iter = ArangeEntryIter {
            input: EndianSlice::new(&buf, LittleEndian),
            encoding,
        };
        let entry = iter.next_raw().unwrap();
        assert_eq!(
            iter.input,
            EndianSlice::new(&buf[buf.len() - 17..], LittleEndian)
        );
        assert_eq!(
            entry,
            Some(ArangeEntry {
                range: Range {
                    begin: 0xffff_ffff_ffff_ffff,
                    end: 0,
                },
                length: 0x0807_0605,
            })
        );

        let mut iter = ArangeEntryIter {
            input: EndianSlice::new(&buf, LittleEndian),
            encoding,
        };
        let entry = iter.next().unwrap();
        assert_eq!(
            iter.input,
            EndianSlice::new(&buf[buf.len() - 1..], LittleEndian)
        );
        assert_eq!(
            entry,
            Some(ArangeEntry {
                range: Range {
                    begin: 0x0403_0201,
                    end: 0x0403_0201 + 0x0807_0605,
                },
                length: 0x0807_0605,
            })
        );
    }
}
