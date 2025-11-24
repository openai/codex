use core::fmt::Debug;
use core::{iter, slice, str};

use crate::elf;
use crate::endian::{Endianness, U32Bytes};
use crate::read::{self, ComdatKind, ObjectComdat, ReadError, ReadRef, SectionIndex, SymbolIndex};

use super::{ElfFile, FileHeader, SectionHeader, Sym};

/// An iterator for the COMDAT section groups in an [`ElfFile32`](super::ElfFile32).
pub type ElfComdatIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfComdatIterator<'data, 'file, elf::FileHeader32<Endian>, R>;
/// An iterator for the COMDAT section groups in an [`ElfFile64`](super::ElfFile64).
pub type ElfComdatIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfComdatIterator<'data, 'file, elf::FileHeader64<Endian>, R>;

/// An iterator for the COMDAT section groups in an [`ElfFile`].
#[derive(Debug)]
pub struct ElfComdatIterator<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    file: &'file ElfFile<'data, Elf, R>,
    iter: iter::Enumerate<slice::Iter<'data, Elf::SectionHeader>>,
}

impl<'data, 'file, Elf, R> ElfComdatIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) fn new(
        file: &'file ElfFile<'data, Elf, R>,
    ) -> ElfComdatIterator<'data, 'file, Elf, R> {
        let mut iter = file.sections.iter().enumerate();
        iter.next(); // Skip null section.
        ElfComdatIterator { file, iter }
    }
}

impl<'data, 'file, Elf, R> Iterator for ElfComdatIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    type Item = ElfComdat<'data, 'file, Elf, R>;

    fn next(&mut self) -> Option<Self::Item> {
        for (_index, section) in self.iter.by_ref() {
            if let Some(comdat) = ElfComdat::parse(self.file, section) {
                return Some(comdat);
            }
        }
        None
    }
}

/// A COMDAT section group in an [`ElfFile32`](super::ElfFile32).
pub type ElfComdat32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfComdat<'data, 'file, elf::FileHeader32<Endian>, R>;
/// A COMDAT section group in an [`ElfFile64`](super::ElfFile64).
pub type ElfComdat64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfComdat<'data, 'file, elf::FileHeader64<Endian>, R>;

/// A COMDAT section group in an [`ElfFile`].
///
/// Most functionality is provided by the [`ObjectComdat`] trait implementation.
#[derive(Debug)]
pub struct ElfComdat<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    file: &'file ElfFile<'data, Elf, R>,
    section: &'data Elf::SectionHeader,
    sections: &'data [U32Bytes<Elf::Endian>],
}

impl<'data, 'file, Elf, R> ElfComdat<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    fn parse(
        file: &'file ElfFile<'data, Elf, R>,
        section: &'data Elf::SectionHeader,
    ) -> Option<ElfComdat<'data, 'file, Elf, R>> {
        let (flag, sections) = section.group(file.endian, file.data).ok()??;
        if flag != elf::GRP_COMDAT {
            return None;
        }
        Some(ElfComdat {
            file,
            section,
            sections,
        })
    }

    /// Get the ELF file containing this COMDAT section group.
    pub fn elf_file(&self) -> &'file ElfFile<'data, Elf, R> {
        self.file
    }

    /// Get the raw ELF section header for the COMDAT section group.
    pub fn elf_section_header(&self) -> &'data Elf::SectionHeader {
        self.section
    }
}

impl<'data, 'file, Elf, R> read::private::Sealed for ElfComdat<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Elf, R> ObjectComdat<'data> for ElfComdat<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    type SectionIterator = ElfComdatSectionIterator<'data, 'file, Elf, R>;

    #[inline]
    fn kind(&self) -> ComdatKind {
        ComdatKind::Any
    }

    #[inline]
    fn symbol(&self) -> SymbolIndex {
        SymbolIndex(self.section.sh_info(self.file.endian) as usize)
    }

    fn name_bytes(&self) -> read::Result<&'data [u8]> {
        // FIXME: check sh_link
        let index = self.symbol();
        let symbol = self.file.symbols.symbol(index)?;
        symbol.name(self.file.endian, self.file.symbols.strings())
    }

    fn name(&self) -> read::Result<&'data str> {
        let name = self.name_bytes()?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 ELF COMDAT name")
    }

    fn sections(&self) -> Self::SectionIterator {
        ElfComdatSectionIterator {
            file: self.file,
            sections: self.sections.iter(),
        }
    }
}

/// An iterator for the sections in a COMDAT section group in an [`ElfFile32`](super::ElfFile32).
pub type ElfComdatSectionIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfComdatSectionIterator<'data, 'file, elf::FileHeader32<Endian>, R>;
/// An iterator for the sections in a COMDAT section group in an [`ElfFile64`](super::ElfFile64).
pub type ElfComdatSectionIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfComdatSectionIterator<'data, 'file, elf::FileHeader64<Endian>, R>;

/// An iterator for the sections in a COMDAT section group in an [`ElfFile`].
#[derive(Debug)]
pub struct ElfComdatSectionIterator<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    file: &'file ElfFile<'data, Elf, R>,
    sections: slice::Iter<'data, U32Bytes<Elf::Endian>>,
}

impl<'data, 'file, Elf, R> Iterator for ElfComdatSectionIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.sections.next()?;
        Some(SectionIndex(index.get(self.file.endian) as usize))
    }
}
