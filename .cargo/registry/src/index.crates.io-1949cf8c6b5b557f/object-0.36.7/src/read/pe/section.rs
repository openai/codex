use core::marker::PhantomData;
use core::{cmp, iter, slice, str};

use crate::endian::LittleEndian as LE;
use crate::pe;
use crate::pe::ImageSectionHeader;
use crate::read::{
    self, CompressedData, CompressedFileRange, ObjectSection, ObjectSegment, ReadError, ReadRef,
    Relocation, RelocationMap, Result, SectionFlags, SectionIndex, SectionKind, SegmentFlags,
};

use super::{ImageNtHeaders, PeFile, SectionTable};

/// An iterator for the loadable sections in a [`PeFile32`](super::PeFile32).
pub type PeSegmentIterator32<'data, 'file, R = &'data [u8]> =
    PeSegmentIterator<'data, 'file, pe::ImageNtHeaders32, R>;
/// An iterator for the loadable sections in a [`PeFile64`](super::PeFile64).
pub type PeSegmentIterator64<'data, 'file, R = &'data [u8]> =
    PeSegmentIterator<'data, 'file, pe::ImageNtHeaders64, R>;

/// An iterator for the loadable sections in a [`PeFile`].
#[derive(Debug)]
pub struct PeSegmentIterator<'data, 'file, Pe, R = &'data [u8]>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    pub(super) file: &'file PeFile<'data, Pe, R>,
    pub(super) iter: slice::Iter<'data, pe::ImageSectionHeader>,
}

impl<'data, 'file, Pe, R> Iterator for PeSegmentIterator<'data, 'file, Pe, R>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    type Item = PeSegment<'data, 'file, Pe, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|section| PeSegment {
            file: self.file,
            section,
        })
    }
}

/// A loadable section in a [`PeFile32`](super::PeFile32).
pub type PeSegment32<'data, 'file, R = &'data [u8]> =
    PeSegment<'data, 'file, pe::ImageNtHeaders32, R>;
/// A loadable section in a [`PeFile64`](super::PeFile64).
pub type PeSegment64<'data, 'file, R = &'data [u8]> =
    PeSegment<'data, 'file, pe::ImageNtHeaders64, R>;

/// A loadable section in a [`PeFile`].
///
/// Most functionality is provided by the [`ObjectSegment`] trait implementation.
#[derive(Debug)]
pub struct PeSegment<'data, 'file, Pe, R = &'data [u8]>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    file: &'file PeFile<'data, Pe, R>,
    section: &'data pe::ImageSectionHeader,
}

impl<'data, 'file, Pe, R> PeSegment<'data, 'file, Pe, R>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    /// Get the PE file containing this segment.
    pub fn pe_file(&self) -> &'file PeFile<'data, Pe, R> {
        self.file
    }

    /// Get the raw PE section header.
    pub fn pe_section(&self) -> &'data pe::ImageSectionHeader {
        self.section
    }
}

impl<'data, 'file, Pe, R> read::private::Sealed for PeSegment<'data, 'file, Pe, R>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Pe, R> ObjectSegment<'data> for PeSegment<'data, 'file, Pe, R>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    #[inline]
    fn address(&self) -> u64 {
        u64::from(self.section.virtual_address.get(LE)).wrapping_add(self.file.common.image_base)
    }

    #[inline]
    fn size(&self) -> u64 {
        u64::from(self.section.virtual_size.get(LE))
    }

    #[inline]
    fn align(&self) -> u64 {
        self.file.section_alignment()
    }

    #[inline]
    fn file_range(&self) -> (u64, u64) {
        let (offset, size) = self.section.pe_file_range();
        (u64::from(offset), u64::from(size))
    }

    fn data(&self) -> Result<&'data [u8]> {
        self.section.pe_data(self.file.data)
    }

    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>> {
        Ok(read::util::data_range(
            self.data()?,
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
        Ok(Some(
            str::from_utf8(name)
                .ok()
                .read_error("Non UTF-8 PE section name")?,
        ))
    }

    #[inline]
    fn flags(&self) -> SegmentFlags {
        let characteristics = self.section.characteristics.get(LE);
        SegmentFlags::Coff { characteristics }
    }
}

/// An iterator for the sections in a [`PeFile32`](super::PeFile32).
pub type PeSectionIterator32<'data, 'file, R = &'data [u8]> =
    PeSectionIterator<'data, 'file, pe::ImageNtHeaders32, R>;
/// An iterator for the sections in a [`PeFile64`](super::PeFile64).
pub type PeSectionIterator64<'data, 'file, R = &'data [u8]> =
    PeSectionIterator<'data, 'file, pe::ImageNtHeaders64, R>;

/// An iterator for the sections in a [`PeFile`].
#[derive(Debug)]
pub struct PeSectionIterator<'data, 'file, Pe, R = &'data [u8]>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    pub(super) file: &'file PeFile<'data, Pe, R>,
    pub(super) iter: iter::Enumerate<slice::Iter<'data, pe::ImageSectionHeader>>,
}

impl<'data, 'file, Pe, R> Iterator for PeSectionIterator<'data, 'file, Pe, R>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    type Item = PeSection<'data, 'file, Pe, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(index, section)| PeSection {
            file: self.file,
            index: SectionIndex(index + 1),
            section,
        })
    }
}

/// A section in a [`PeFile32`](super::PeFile32).
pub type PeSection32<'data, 'file, R = &'data [u8]> =
    PeSection<'data, 'file, pe::ImageNtHeaders32, R>;
/// A section in a [`PeFile64`](super::PeFile64).
pub type PeSection64<'data, 'file, R = &'data [u8]> =
    PeSection<'data, 'file, pe::ImageNtHeaders64, R>;

/// A section in a [`PeFile`].
///
/// Most functionality is provided by the [`ObjectSection`] trait implementation.
#[derive(Debug)]
pub struct PeSection<'data, 'file, Pe, R = &'data [u8]>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    pub(super) file: &'file PeFile<'data, Pe, R>,
    pub(super) index: SectionIndex,
    pub(super) section: &'data pe::ImageSectionHeader,
}

impl<'data, 'file, Pe, R> PeSection<'data, 'file, Pe, R>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    /// Get the PE file containing this segment.
    pub fn pe_file(&self) -> &'file PeFile<'data, Pe, R> {
        self.file
    }

    /// Get the raw PE section header.
    pub fn pe_section(&self) -> &'data pe::ImageSectionHeader {
        self.section
    }
}

impl<'data, 'file, Pe, R> read::private::Sealed for PeSection<'data, 'file, Pe, R>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Pe, R> ObjectSection<'data> for PeSection<'data, 'file, Pe, R>
where
    Pe: ImageNtHeaders,
    R: ReadRef<'data>,
{
    type RelocationIterator = PeRelocationIterator<'data, 'file, R>;

    #[inline]
    fn index(&self) -> SectionIndex {
        self.index
    }

    #[inline]
    fn address(&self) -> u64 {
        u64::from(self.section.virtual_address.get(LE)).wrapping_add(self.file.common.image_base)
    }

    #[inline]
    fn size(&self) -> u64 {
        u64::from(self.section.virtual_size.get(LE))
    }

    #[inline]
    fn align(&self) -> u64 {
        self.file.section_alignment()
    }

    #[inline]
    fn file_range(&self) -> Option<(u64, u64)> {
        let (offset, size) = self.section.pe_file_range();
        if size == 0 {
            None
        } else {
            Some((u64::from(offset), u64::from(size)))
        }
    }

    fn data(&self) -> Result<&'data [u8]> {
        self.section.pe_data(self.file.data)
    }

    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>> {
        Ok(read::util::data_range(
            self.data()?,
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
            .read_error("Non UTF-8 PE section name")
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

    fn relocations(&self) -> PeRelocationIterator<'data, 'file, R> {
        PeRelocationIterator(PhantomData)
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

impl<'data> SectionTable<'data> {
    /// Return the file offset of the given virtual address, and the size up
    /// to the end of the section containing it.
    ///
    /// Returns `None` if no section contains the address.
    pub fn pe_file_range_at(&self, va: u32) -> Option<(u32, u32)> {
        self.iter().find_map(|section| section.pe_file_range_at(va))
    }

    /// Return the data starting at the given virtual address, up to the end of the
    /// section containing it.
    ///
    /// Ignores sections with invalid data.
    ///
    /// Returns `None` if no section contains the address.
    pub fn pe_data_at<R: ReadRef<'data>>(&self, data: R, va: u32) -> Option<&'data [u8]> {
        self.iter().find_map(|section| section.pe_data_at(data, va))
    }

    /// Return the data of the section that contains the given virtual address in a PE file.
    ///
    /// Also returns the virtual address of that section.
    ///
    /// Ignores sections with invalid data.
    pub fn pe_data_containing<R: ReadRef<'data>>(
        &self,
        data: R,
        va: u32,
    ) -> Option<(&'data [u8], u32)> {
        self.iter()
            .find_map(|section| section.pe_data_containing(data, va))
    }

    /// Return the section that contains a given virtual address.
    pub fn section_containing(&self, va: u32) -> Option<&'data ImageSectionHeader> {
        self.iter().find(|section| section.contains_rva(va))
    }
}

impl pe::ImageSectionHeader {
    /// Return the offset and size of the section in a PE file.
    ///
    /// The size of the range will be the minimum of the file size and virtual size.
    pub fn pe_file_range(&self) -> (u32, u32) {
        // Pointer and size will be zero for uninitialized data; we don't need to validate this.
        let offset = self.pointer_to_raw_data.get(LE);
        let size = cmp::min(self.virtual_size.get(LE), self.size_of_raw_data.get(LE));
        (offset, size)
    }

    /// Return the file offset of the given virtual address, and the remaining size up
    /// to the end of the section.
    ///
    /// Returns `None` if the section does not contain the address.
    pub fn pe_file_range_at(&self, va: u32) -> Option<(u32, u32)> {
        let section_va = self.virtual_address.get(LE);
        let offset = va.checked_sub(section_va)?;
        let (section_offset, section_size) = self.pe_file_range();
        // Address must be within section (and not at its end).
        if offset < section_size {
            Some((section_offset.checked_add(offset)?, section_size - offset))
        } else {
            None
        }
    }

    /// Return the virtual address and size of the section.
    pub fn pe_address_range(&self) -> (u32, u32) {
        (self.virtual_address.get(LE), self.virtual_size.get(LE))
    }

    /// Return the section data in a PE file.
    ///
    /// The length of the data will be the minimum of the file size and virtual size.
    pub fn pe_data<'data, R: ReadRef<'data>>(&self, data: R) -> Result<&'data [u8]> {
        let (offset, size) = self.pe_file_range();
        data.read_bytes_at(offset.into(), size.into())
            .read_error("Invalid PE section offset or size")
    }

    /// Return the data starting at the given virtual address, up to the end of the
    /// section.
    ///
    /// Ignores sections with invalid data.
    ///
    /// Returns `None` if the section does not contain the address.
    pub fn pe_data_at<'data, R: ReadRef<'data>>(&self, data: R, va: u32) -> Option<&'data [u8]> {
        let (offset, size) = self.pe_file_range_at(va)?;
        data.read_bytes_at(offset.into(), size.into()).ok()
    }

    /// Tests whether a given RVA is part of this section
    pub fn contains_rva(&self, va: u32) -> bool {
        let section_va = self.virtual_address.get(LE);
        match va.checked_sub(section_va) {
            None => false,
            Some(offset) => {
                // Address must be within section (and not at its end).
                offset < self.virtual_size.get(LE)
            }
        }
    }

    /// Return the section data if it contains the given virtual address.
    ///
    /// Also returns the virtual address of that section.
    ///
    /// Ignores sections with invalid data.
    pub fn pe_data_containing<'data, R: ReadRef<'data>>(
        &self,
        data: R,
        va: u32,
    ) -> Option<(&'data [u8], u32)> {
        let section_va = self.virtual_address.get(LE);
        let offset = va.checked_sub(section_va)?;
        let (section_offset, section_size) = self.pe_file_range();
        // Address must be within section (and not at its end).
        if offset < section_size {
            let section_data = data
                .read_bytes_at(section_offset.into(), section_size.into())
                .ok()?;
            Some((section_data, section_va))
        } else {
            None
        }
    }
}

/// An iterator for the relocations in an [`PeSection`].
///
/// This is a stub that doesn't implement any functionality.
#[derive(Debug)]
pub struct PeRelocationIterator<'data, 'file, R = &'data [u8]>(
    PhantomData<(&'data (), &'file (), R)>,
);

impl<'data, 'file, R> Iterator for PeRelocationIterator<'data, 'file, R> {
    type Item = (u64, Relocation);

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
