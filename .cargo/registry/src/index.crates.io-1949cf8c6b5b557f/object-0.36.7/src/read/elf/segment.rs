use core::fmt::Debug;
use core::{slice, str};

use crate::elf;
use crate::endian::{self, Endianness};
use crate::pod::{self, Pod};
use crate::read::{self, ObjectSegment, ReadError, ReadRef, SegmentFlags};

use super::{ElfFile, FileHeader, NoteIterator};

/// An iterator for the segments in an [`ElfFile32`](super::ElfFile32).
pub type ElfSegmentIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSegmentIterator<'data, 'file, elf::FileHeader32<Endian>, R>;
/// An iterator for the segments in an [`ElfFile64`](super::ElfFile64).
pub type ElfSegmentIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSegmentIterator<'data, 'file, elf::FileHeader64<Endian>, R>;

/// An iterator for the segments in an [`ElfFile`].
#[derive(Debug)]
pub struct ElfSegmentIterator<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) file: &'file ElfFile<'data, Elf, R>,
    pub(super) iter: slice::Iter<'data, Elf::ProgramHeader>,
}

impl<'data, 'file, Elf, R> Iterator for ElfSegmentIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    type Item = ElfSegment<'data, 'file, Elf, R>;

    fn next(&mut self) -> Option<Self::Item> {
        for segment in self.iter.by_ref() {
            if segment.p_type(self.file.endian) == elf::PT_LOAD {
                return Some(ElfSegment {
                    file: self.file,
                    segment,
                });
            }
        }
        None
    }
}

/// A segment in an [`ElfFile32`](super::ElfFile32).
pub type ElfSegment32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSegment<'data, 'file, elf::FileHeader32<Endian>, R>;
/// A segment in an [`ElfFile64`](super::ElfFile64).
pub type ElfSegment64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSegment<'data, 'file, elf::FileHeader64<Endian>, R>;

/// A segment in an [`ElfFile`].
///
/// Most functionality is provided by the [`ObjectSegment`] trait implementation.
#[derive(Debug)]
pub struct ElfSegment<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) file: &'file ElfFile<'data, Elf, R>,
    pub(super) segment: &'data Elf::ProgramHeader,
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> ElfSegment<'data, 'file, Elf, R> {
    /// Get the ELF file containing this segment.
    pub fn elf_file(&self) -> &'file ElfFile<'data, Elf, R> {
        self.file
    }

    /// Get the raw ELF program header for the segment.
    pub fn elf_program_header(&self) -> &'data Elf::ProgramHeader {
        self.segment
    }

    fn bytes(&self) -> read::Result<&'data [u8]> {
        self.segment
            .data(self.file.endian, self.file.data)
            .read_error("Invalid ELF segment size or offset")
    }
}

impl<'data, 'file, Elf, R> read::private::Sealed for ElfSegment<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Elf, R> ObjectSegment<'data> for ElfSegment<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    #[inline]
    fn address(&self) -> u64 {
        self.segment.p_vaddr(self.file.endian).into()
    }

    #[inline]
    fn size(&self) -> u64 {
        self.segment.p_memsz(self.file.endian).into()
    }

    #[inline]
    fn align(&self) -> u64 {
        self.segment.p_align(self.file.endian).into()
    }

    #[inline]
    fn file_range(&self) -> (u64, u64) {
        self.segment.file_range(self.file.endian)
    }

    #[inline]
    fn data(&self) -> read::Result<&'data [u8]> {
        self.bytes()
    }

    fn data_range(&self, address: u64, size: u64) -> read::Result<Option<&'data [u8]>> {
        Ok(read::util::data_range(
            self.bytes()?,
            self.address(),
            address,
            size,
        ))
    }

    #[inline]
    fn name_bytes(&self) -> read::Result<Option<&[u8]>> {
        Ok(None)
    }

    #[inline]
    fn name(&self) -> read::Result<Option<&str>> {
        Ok(None)
    }

    #[inline]
    fn flags(&self) -> SegmentFlags {
        let p_flags = self.segment.p_flags(self.file.endian);
        SegmentFlags::Elf { p_flags }
    }
}

/// A trait for generic access to [`elf::ProgramHeader32`] and [`elf::ProgramHeader64`].
#[allow(missing_docs)]
pub trait ProgramHeader: Debug + Pod {
    type Elf: FileHeader<ProgramHeader = Self, Endian = Self::Endian, Word = Self::Word>;
    type Word: Into<u64>;
    type Endian: endian::Endian;

    fn p_type(&self, endian: Self::Endian) -> u32;
    fn p_flags(&self, endian: Self::Endian) -> u32;
    fn p_offset(&self, endian: Self::Endian) -> Self::Word;
    fn p_vaddr(&self, endian: Self::Endian) -> Self::Word;
    fn p_paddr(&self, endian: Self::Endian) -> Self::Word;
    fn p_filesz(&self, endian: Self::Endian) -> Self::Word;
    fn p_memsz(&self, endian: Self::Endian) -> Self::Word;
    fn p_align(&self, endian: Self::Endian) -> Self::Word;

    /// Return the offset and size of the segment in the file.
    fn file_range(&self, endian: Self::Endian) -> (u64, u64) {
        (self.p_offset(endian).into(), self.p_filesz(endian).into())
    }

    /// Return the segment data.
    ///
    /// Returns `Err` for invalid values.
    fn data<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> Result<&'data [u8], ()> {
        let (offset, size) = self.file_range(endian);
        data.read_bytes_at(offset, size)
    }

    /// Return the segment data as a slice of the given type.
    ///
    /// Allows padding at the end of the data.
    /// Returns `Ok(&[])` if the segment has no data.
    /// Returns `Err` for invalid values, including bad alignment.
    fn data_as_array<'data, T: Pod, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> Result<&'data [T], ()> {
        pod::slice_from_all_bytes(self.data(endian, data)?)
    }

    /// Return the segment data in the given virtual address range
    ///
    /// Returns `Ok(None)` if the segment does not contain the address.
    /// Returns `Err` for invalid values.
    fn data_range<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
        address: u64,
        size: u64,
    ) -> Result<Option<&'data [u8]>, ()> {
        Ok(read::util::data_range(
            self.data(endian, data)?,
            self.p_vaddr(endian).into(),
            address,
            size,
        ))
    }

    /// Return entries in a dynamic segment.
    ///
    /// Returns `Ok(None)` if the segment is not `PT_DYNAMIC`.
    /// Returns `Err` for invalid values.
    fn dynamic<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<&'data [<Self::Elf as FileHeader>::Dyn]>> {
        if self.p_type(endian) != elf::PT_DYNAMIC {
            return Ok(None);
        }
        let dynamic = self
            .data_as_array(endian, data)
            .read_error("Invalid ELF dynamic segment offset or size")?;
        Ok(Some(dynamic))
    }

    /// Return the data in an interpreter segment.
    ///
    /// Returns `Ok(None)` if the segment is not `PT_INTERP`.
    /// Returns `Err` for invalid values.
    fn interpreter<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<&'data [u8]>> {
        if self.p_type(endian) != elf::PT_INTERP {
            return Ok(None);
        }
        let data = self
            .data(endian, data)
            .read_error("Invalid ELF interpreter segment offset or size")?;
        let len = data
            .iter()
            .position(|&b| b == 0)
            .read_error("Invalid ELF interpreter segment data")?;
        Ok(Some(&data[..len]))
    }

    /// Return a note iterator for the segment data.
    ///
    /// Returns `Ok(None)` if the segment does not contain notes.
    /// Returns `Err` for invalid values.
    fn notes<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<NoteIterator<'data, Self::Elf>>> {
        if self.p_type(endian) != elf::PT_NOTE {
            return Ok(None);
        }
        let data = self
            .data(endian, data)
            .read_error("Invalid ELF note segment offset or size")?;
        let notes = NoteIterator::new(endian, self.p_align(endian), data)?;
        Ok(Some(notes))
    }
}

impl<Endian: endian::Endian> ProgramHeader for elf::ProgramHeader32<Endian> {
    type Word = u32;
    type Endian = Endian;
    type Elf = elf::FileHeader32<Endian>;

    #[inline]
    fn p_type(&self, endian: Self::Endian) -> u32 {
        self.p_type.get(endian)
    }

    #[inline]
    fn p_flags(&self, endian: Self::Endian) -> u32 {
        self.p_flags.get(endian)
    }

    #[inline]
    fn p_offset(&self, endian: Self::Endian) -> Self::Word {
        self.p_offset.get(endian)
    }

    #[inline]
    fn p_vaddr(&self, endian: Self::Endian) -> Self::Word {
        self.p_vaddr.get(endian)
    }

    #[inline]
    fn p_paddr(&self, endian: Self::Endian) -> Self::Word {
        self.p_paddr.get(endian)
    }

    #[inline]
    fn p_filesz(&self, endian: Self::Endian) -> Self::Word {
        self.p_filesz.get(endian)
    }

    #[inline]
    fn p_memsz(&self, endian: Self::Endian) -> Self::Word {
        self.p_memsz.get(endian)
    }

    #[inline]
    fn p_align(&self, endian: Self::Endian) -> Self::Word {
        self.p_align.get(endian)
    }
}

impl<Endian: endian::Endian> ProgramHeader for elf::ProgramHeader64<Endian> {
    type Word = u64;
    type Endian = Endian;
    type Elf = elf::FileHeader64<Endian>;

    #[inline]
    fn p_type(&self, endian: Self::Endian) -> u32 {
        self.p_type.get(endian)
    }

    #[inline]
    fn p_flags(&self, endian: Self::Endian) -> u32 {
        self.p_flags.get(endian)
    }

    #[inline]
    fn p_offset(&self, endian: Self::Endian) -> Self::Word {
        self.p_offset.get(endian)
    }

    #[inline]
    fn p_vaddr(&self, endian: Self::Endian) -> Self::Word {
        self.p_vaddr.get(endian)
    }

    #[inline]
    fn p_paddr(&self, endian: Self::Endian) -> Self::Word {
        self.p_paddr.get(endian)
    }

    #[inline]
    fn p_filesz(&self, endian: Self::Endian) -> Self::Word {
        self.p_filesz.get(endian)
    }

    #[inline]
    fn p_memsz(&self, endian: Self::Endian) -> Self::Word {
        self.p_memsz.get(endian)
    }

    #[inline]
    fn p_align(&self, endian: Self::Endian) -> Self::Word {
        self.p_align.get(endian)
    }
}
