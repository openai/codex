use core::convert::TryFrom;
use core::{iter, result, slice, str};

use crate::endian::LittleEndian as LE;
use crate::pe;
use crate::read::util::StringTable;
use crate::read::{
    self, CompressedData, CompressedFileRange, Error, ObjectSection, ObjectSegment, ReadError,
    ReadRef, RelocationMap, Result, SectionFlags, SectionIndex, SectionKind, SegmentFlags,
};

use super::{CoffFile, CoffHeader, CoffRelocationIterator};

/// The table of section headers in a COFF or PE file.
///
/// Returned by [`CoffHeader::sections`] and
/// [`ImageNtHeaders::sections`](crate::read::pe::ImageNtHeaders::sections).
#[derive(Debug, Default, Clone, Copy)]
pub struct SectionTable<'data> {
    sections: &'data [pe::ImageSectionHeader],
}

impl<'data> SectionTable<'data> {
    /// Parse the section table.
    ///
    /// `data` must be the entire file data.
    /// `offset` must be after the optional file header.
    pub fn parse<Coff: CoffHeader, R: ReadRef<'data>>(
        header: &Coff,
        data: R,
        offset: u64,
    ) -> Result<Self> {
        let sections = data
            .read_slice_at(offset, header.number_of_sections() as usize)
            .read_error("Invalid COFF/PE section headers")?;
        Ok(SectionTable { sections })
    }

    /// Iterate over the section headers.
    ///
    /// Warning: section indices start at 1.
    #[inline]
    pub fn iter(&self) -> slice::Iter<'data, pe::ImageSectionHeader> {
        self.sections.iter()
    }

    /// Iterate over the section headers and their indices.
    pub fn enumerate(&self) -> impl Iterator<Item = (SectionIndex, &'data pe::ImageSectionHeader)> {
        self.sections
            .iter()
            .enumerate()
            .map(|(i, section)| (SectionIndex(i + 1), section))
    }

    /// Return true if the section table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    /// The number of section headers.
    #[inline]
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    /// Return the section header at the given index.
    ///
    /// The index is 1-based.
    pub fn section(&self, index: SectionIndex) -> read::Result<&'data pe::ImageSectionHeader> {
        self.sections
            .get(index.0.wrapping_sub(1))
            .read_error("Invalid COFF/PE section index")
    }

    /// Return the section header with the given name.
    ///
    /// The returned index is 1-based.
    ///
    /// Ignores sections with invalid names.
    pub fn section_by_name<R: ReadRef<'data>>(
        &self,
        strings: StringTable<'data, R>,
        name: &[u8],
    ) -> Option<(SectionIndex, &'data pe::ImageSectionHeader)> {
        self.enumerate()
            .find(|(_, section)| section.name(strings) == Ok(name))
    }

    /// Compute the maximum file offset used by sections.
    ///
    /// This will usually match the end of file, unless the PE file has a
    /// [data overlay](https://security.stackexchange.com/questions/77336/how-is-the-file-overlay-read-by-an-exe-virus)
    pub fn max_section_file_offset(&self) -> u64 {
        let mut max = 0;
        for section in self.iter() {
            match (section.pointer_to_raw_data.get(LE) as u64)
                .checked_add(section.size_of_raw_data.get(LE) as u64)
            {
                None => {
                    // This cannot happen, we're suming two u32 into a u64
                    continue;
                }
                Some(end_of_section) => {
                    if end_of_section > max {
                        max = end_of_section;
                    }
                }
            }
        }
        max
    }
}

/// An iterator for the loadable sections in a [`CoffBigFile`](super::CoffBigFile).
pub type CoffBigSegmentIterator<'data, 'file, R = &'data [u8]> =
    CoffSegmentIterator<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// An iterator for the loadable sections in a [`CoffFile`].
#[derive(Debug)]
pub struct CoffSegmentIterator<
    'data,
    'file,
    R: ReadRef<'data> = &'data [u8],
    Coff: CoffHeader = pe::ImageFileHeader,
> {
    pub(super) file: &'file CoffFile<'data, R, Coff>,
    pub(super) iter: slice::Iter<'data, pe::ImageSectionHeader>,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> Iterator
    for CoffSegmentIterator<'data, 'file, R, Coff>
{
    type Item = CoffSegment<'data, 'file, R, Coff>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|section| CoffSegment {
            file: self.file,
            section,
        })
    }
}

/// A loadable section in a [`CoffBigFile`](super::CoffBigFile).
///
/// Most functionality is provided by the [`ObjectSegment`] trait implementation.
pub type CoffBigSegment<'data, 'file, R = &'data [u8]> =
    CoffSegment<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// A loadable section in a [`CoffFile`].
///
/// Most functionality is provided by the [`ObjectSegment`] trait implementation.
#[derive(Debug)]
pub struct CoffSegment<
    'data,
    'file,
    R: ReadRef<'data> = &'data [u8],
    Coff: CoffHeader = pe::ImageFileHeader,
> {
    pub(super) file: &'file CoffFile<'data, R, Coff>,
    pub(super) section: &'data pe::ImageSectionHeader,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> CoffSegment<'data, 'file, R, Coff> {
    /// Get the COFF file containing this segment.
    pub fn coff_file(&self) -> &'file CoffFile<'data, R, Coff> {
        self.file
    }

    /// Get the raw COFF section header.
    pub fn coff_section(&self) -> &'data pe::ImageSectionHeader {
        self.section
    }

    fn bytes(&self) -> Result<&'data [u8]> {
        self.section
            .coff_data(self.file.data)
            .read_error("Invalid COFF section offset or size")
    }
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> read::private::Sealed
    for CoffSegment<'data, 'file, R, Coff>
{
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> ObjectSegment<'data>
    for CoffSegment<'data, 'file, R, Coff>
{
    #[inline]
    fn address(&self) -> u64 {
        u64::from(self.section.virtual_address.get(LE))
    }

    #[inline]
    fn size(&self) -> u64 {
        u64::from(self.section.virtual_size.get(LE))
    }

    #[inline]
    fn align(&self) -> u64 {
        self.section.coff_alignment()
    }

    #[inline]
    fn file_range(&self) -> (u64, u64) {
        let (offset, size) = self.section.coff_file_range().unwrap_or((0, 0));
        (u64::from(offset), u64::from(size))
    }

    fn data(&self) -> Result<&'data [u8]> {
        self.bytes()
    }

    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>> {
        Ok(read::util::data_range(
            self.bytes()?,
            self.address(),
            address,
            size,
        ))
    }

    #[inline]
    fn name_bytes(&self) -> Result<Option<&[u8]>> {
        self.section
            .name(self.file.common.symbols.strings())
            .map(Some)
    }

    #[inline]
    fn name(&self) -> Result<Option<&str>> {
        let name = self.section.name(self.file.common.symbols.strings())?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 COFF section name")
            .map(Some)
    }

    #[inline]
    fn flags(&self) -> SegmentFlags {
        let characteristics = self.section.characteristics.get(LE);
        SegmentFlags::Coff { characteristics }
    }
}

/// An iterator for the sections in a [`CoffBigFile`](super::CoffBigFile).
pub type CoffBigSectionIterator<'data, 'file, R = &'data [u8]> =
    CoffSectionIterator<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// An iterator for the sections in a [`CoffFile`].
#[derive(Debug)]
pub struct CoffSectionIterator<
    'data,
    'file,
    R: ReadRef<'data> = &'data [u8],
    Coff: CoffHeader = pe::ImageFileHeader,
> {
    pub(super) file: &'file CoffFile<'data, R, Coff>,
    pub(super) iter: iter::Enumerate<slice::Iter<'data, pe::ImageSectionHeader>>,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> Iterator
    for CoffSectionIterator<'data, 'file, R, Coff>
{
    type Item = CoffSection<'data, 'file, R, Coff>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(index, section)| CoffSection {
            file: self.file,
            index: SectionIndex(index + 1),
            section,
        })
    }
}

/// A section in a [`CoffBigFile`](super::CoffBigFile).
///
/// Most functionality is provided by the [`ObjectSection`] trait implementation.
pub type CoffBigSection<'data, 'file, R = &'data [u8]> =
    CoffSection<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// A section in a [`CoffFile`].
///
/// Most functionality is provided by the [`ObjectSection`] trait implementation.
#[derive(Debug)]
pub struct CoffSection<
    'data,
    'file,
    R: ReadRef<'data> = &'data [u8],
    Coff: CoffHeader = pe::ImageFileHeader,
> {
    pub(super) file: &'file CoffFile<'data, R, Coff>,
    pub(super) index: SectionIndex,
    pub(super) section: &'data pe::ImageSectionHeader,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> CoffSection<'data, 'file, R, Coff> {
    /// Get the COFF file containing this section.
    pub fn coff_file(&self) -> &'file CoffFile<'data, R, Coff> {
        self.file
    }

    /// Get the raw COFF section header.
    pub fn coff_section(&self) -> &'data pe::ImageSectionHeader {
        self.section
    }

    /// Get the raw COFF relocations for this section.
    pub fn coff_relocations(&self) -> Result<&'data [pe::ImageRelocation]> {
        self.section.coff_relocations(self.file.data)
    }

    fn bytes(&self) -> Result<&'data [u8]> {
        self.section
            .coff_data(self.file.data)
            .read_error("Invalid COFF section offset or size")
    }
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> read::private::Sealed
    for CoffSection<'data, 'file, R, Coff>
{
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> ObjectSection<'data>
    for CoffSection<'data, 'file, R, Coff>
{
    type RelocationIterator = CoffRelocationIterator<'data, 'file, R, Coff>;

    #[inline]
    fn index(&self) -> SectionIndex {
        self.index
    }

    #[inline]
    fn address(&self) -> u64 {
        u64::from(self.section.virtual_address.get(LE))
    }

    #[inline]
    fn size(&self) -> u64 {
        // TODO: This may need to be the length from the auxiliary symbol for this section.
        u64::from(self.section.size_of_raw_data.get(LE))
    }

    #[inline]
    fn align(&self) -> u64 {
        self.section.coff_alignment()
    }

    #[inline]
    fn file_range(&self) -> Option<(u64, u64)> {
        let (offset, size) = self.section.coff_file_range()?;
        Some((u64::from(offset), u64::from(size)))
    }

    fn data(&self) -> Result<&'data [u8]> {
        self.bytes()
    }

    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>> {
        Ok(read::util::data_range(
            self.bytes()?,
            self.address(),
            address,
            size,
        ))
    }

    #[inline]
    fn compressed_file_range(&self) -> Result<CompressedFileRange> {
        Ok(CompressedFileRange::none(self.file_range()))
    }

    #[inline]
    fn compressed_data(&self) -> Result<CompressedData<'data>> {
        self.data().map(CompressedData::none)
    }

    #[inline]
    fn name_bytes(&self) -> Result<&'data [u8]> {
        self.section.name(self.file.common.symbols.strings())
    }

    #[inline]
    fn name(&self) -> Result<&'data str> {
        let name = self.name_bytes()?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 COFF section name")
    }

    #[inline]
    fn segment_name_bytes(&self) -> Result<Option<&[u8]>> {
        Ok(None)
    }

    #[inline]
    fn segment_name(&self) -> Result<Option<&str>> {
        Ok(None)
    }

    #[inline]
    fn kind(&self) -> SectionKind {
        self.section.kind()
    }

    fn relocations(&self) -> CoffRelocationIterator<'data, 'file, R, Coff> {
        let relocations = self.coff_relocations().unwrap_or(&[]);
        CoffRelocationIterator {
            file: self.file,
            iter: relocations.iter(),
        }
    }

    fn relocation_map(&self) -> read::Result<RelocationMap> {
        RelocationMap::new(self.file, self)
    }

    fn flags(&self) -> SectionFlags {
        SectionFlags::Coff {
            characteristics: self.section.characteristics.get(LE),
        }
    }
}

impl pe::ImageSectionHeader {
    pub(crate) fn kind(&self) -> SectionKind {
        let characteristics = self.characteristics.get(LE);
        if characteristics & (pe::IMAGE_SCN_CNT_CODE | pe::IMAGE_SCN_MEM_EXECUTE) != 0 {
            SectionKind::Text
        } else if characteristics & pe::IMAGE_SCN_CNT_INITIALIZED_DATA != 0 {
            if characteristics & pe::IMAGE_SCN_MEM_DISCARDABLE != 0 {
                SectionKind::Other
            } else if characteristics & pe::IMAGE_SCN_MEM_WRITE != 0 {
                SectionKind::Data
            } else {
                SectionKind::ReadOnlyData
            }
        } else if characteristics & pe::IMAGE_SCN_CNT_UNINITIALIZED_DATA != 0 {
            SectionKind::UninitializedData
        } else if characteristics & pe::IMAGE_SCN_LNK_INFO != 0 {
            SectionKind::Linker
        } else {
            SectionKind::Unknown
        }
    }
}

impl pe::ImageSectionHeader {
    /// Return the string table offset of the section name.
    ///
    /// Returns `Ok(None)` if the name doesn't use the string table
    /// and can be obtained with `raw_name` instead.
    pub fn name_offset(&self) -> Result<Option<u32>> {
        let bytes = &self.name;
        if bytes[0] != b'/' {
            return Ok(None);
        }

        if bytes[1] == b'/' {
            let mut offset = 0;
            for byte in bytes[2..].iter() {
                let digit = match byte {
                    b'A'..=b'Z' => byte - b'A',
                    b'a'..=b'z' => byte - b'a' + 26,
                    b'0'..=b'9' => byte - b'0' + 52,
                    b'+' => 62,
                    b'/' => 63,
                    _ => return Err(Error("Invalid COFF section name base-64 offset")),
                };
                offset = offset * 64 + digit as u64;
            }
            u32::try_from(offset)
                .ok()
                .read_error("Invalid COFF section name base-64 offset")
                .map(Some)
        } else {
            let mut offset = 0;
            for byte in bytes[1..].iter() {
                let digit = match byte {
                    b'0'..=b'9' => byte - b'0',
                    0 => break,
                    _ => return Err(Error("Invalid COFF section name base-10 offset")),
                };
                offset = offset * 10 + digit as u32;
            }
            Ok(Some(offset))
        }
    }

    /// Return the section name.
    ///
    /// This handles decoding names that are offsets into the symbol string table.
    pub fn name<'data, R: ReadRef<'data>>(
        &'data self,
        strings: StringTable<'data, R>,
    ) -> Result<&'data [u8]> {
        if let Some(offset) = self.name_offset()? {
            strings
                .get(offset)
                .read_error("Invalid COFF section name offset")
        } else {
            Ok(self.raw_name())
        }
    }

    /// Return the raw section name.
    pub fn raw_name(&self) -> &[u8] {
        let bytes = &self.name;
        match memchr::memchr(b'\0', bytes) {
            Some(end) => &bytes[..end],
            None => &bytes[..],
        }
    }

    /// Return the offset and size of the section in a COFF file.
    ///
    /// Returns `None` for sections that have no data in the file.
    pub fn coff_file_range(&self) -> Option<(u32, u32)> {
        if self.characteristics.get(LE) & pe::IMAGE_SCN_CNT_UNINITIALIZED_DATA != 0 {
            None
        } else {
            let offset = self.pointer_to_raw_data.get(LE);
            // Note: virtual size is not used for COFF.
            let size = self.size_of_raw_data.get(LE);
            Some((offset, size))
        }
    }

    /// Return the section data in a COFF file.
    ///
    /// Returns `Ok(&[])` if the section has no data.
    /// Returns `Err` for invalid values.
    pub fn coff_data<'data, R: ReadRef<'data>>(&self, data: R) -> result::Result<&'data [u8], ()> {
        if let Some((offset, size)) = self.coff_file_range() {
            data.read_bytes_at(offset.into(), size.into())
        } else {
            Ok(&[])
        }
    }

    /// Return the section alignment in bytes.
    ///
    /// This is only valid for sections in a COFF file.
    pub fn coff_alignment(&self) -> u64 {
        match self.characteristics.get(LE) & pe::IMAGE_SCN_ALIGN_MASK {
            pe::IMAGE_SCN_ALIGN_1BYTES => 1,
            pe::IMAGE_SCN_ALIGN_2BYTES => 2,
            pe::IMAGE_SCN_ALIGN_4BYTES => 4,
            pe::IMAGE_SCN_ALIGN_8BYTES => 8,
            pe::IMAGE_SCN_ALIGN_16BYTES => 16,
            pe::IMAGE_SCN_ALIGN_32BYTES => 32,
            pe::IMAGE_SCN_ALIGN_64BYTES => 64,
            pe::IMAGE_SCN_ALIGN_128BYTES => 128,
            pe::IMAGE_SCN_ALIGN_256BYTES => 256,
            pe::IMAGE_SCN_ALIGN_512BYTES => 512,
            pe::IMAGE_SCN_ALIGN_1024BYTES => 1024,
            pe::IMAGE_SCN_ALIGN_2048BYTES => 2048,
            pe::IMAGE_SCN_ALIGN_4096BYTES => 4096,
            pe::IMAGE_SCN_ALIGN_8192BYTES => 8192,
            _ => 16,
        }
    }

    /// Read the relocations in a COFF file.
    ///
    /// `data` must be the entire file data.
    pub fn coff_relocations<'data, R: ReadRef<'data>>(
        &self,
        data: R,
    ) -> read::Result<&'data [pe::ImageRelocation]> {
        let mut pointer = self.pointer_to_relocations.get(LE).into();
        let mut number: usize = self.number_of_relocations.get(LE).into();
        if number == u16::MAX.into()
            && self.characteristics.get(LE) & pe::IMAGE_SCN_LNK_NRELOC_OVFL != 0
        {
            // Extended relocations. Read first relocation (which contains extended count) & adjust
            // relocations pointer.
            let extended_relocation_info = data
                .read_at::<pe::ImageRelocation>(pointer)
                .read_error("Invalid COFF relocation offset or number")?;
            number = extended_relocation_info.virtual_address.get(LE) as usize;
            if number == 0 {
                return Err(Error("Invalid COFF relocation number"));
            }
            pointer += core::mem::size_of::<pe::ImageRelocation>() as u64;
            // Extended relocation info does not contribute to the count of sections.
            number -= 1;
        }
        data.read_slice_at(pointer, number)
            .read_error("Invalid COFF relocation offset or number")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_offset() {
        let mut section = pe::ImageSectionHeader::default();
        section.name = *b"xxxxxxxx";
        assert_eq!(section.name_offset(), Ok(None));
        section.name = *b"/0\0\0\0\0\0\0";
        assert_eq!(section.name_offset(), Ok(Some(0)));
        section.name = *b"/9999999";
        assert_eq!(section.name_offset(), Ok(Some(999_9999)));
        section.name = *b"//AAAAAA";
        assert_eq!(section.name_offset(), Ok(Some(0)));
        section.name = *b"//D/////";
        assert_eq!(section.name_offset(), Ok(Some(0xffff_ffff)));
        section.name = *b"//EAAAAA";
        assert!(section.name_offset().is_err());
        section.name = *b"////////";
        assert!(section.name_offset().is_err());
    }
}
