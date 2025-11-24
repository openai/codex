use core::fmt::Debug;
use core::{iter, slice, str};

use crate::elf;
use crate::endian::{self, Endianness, U32Bytes};
use crate::pod::{self, Pod};
use crate::read::{
    self, gnu_compression, CompressedData, CompressedFileRange, CompressionFormat, Error,
    ObjectSection, ReadError, ReadRef, RelocationMap, SectionFlags, SectionIndex, SectionKind,
    StringTable,
};

use super::{
    AttributesSection, CompressionHeader, ElfFile, ElfSectionRelocationIterator, FileHeader,
    GnuHashTable, HashTable, NoteIterator, RelocationSections, RelrIterator, SymbolTable,
    VerdefIterator, VerneedIterator, VersionTable,
};

/// The table of section headers in an ELF file.
///
/// Also includes the string table used for the section names.
///
/// Returned by [`FileHeader::sections`].
#[derive(Debug, Clone, Copy)]
pub struct SectionTable<'data, Elf: FileHeader, R = &'data [u8]>
where
    R: ReadRef<'data>,
{
    sections: &'data [Elf::SectionHeader],
    strings: StringTable<'data, R>,
}

impl<'data, Elf: FileHeader, R: ReadRef<'data>> Default for SectionTable<'data, Elf, R> {
    fn default() -> Self {
        SectionTable {
            sections: &[],
            strings: StringTable::default(),
        }
    }
}

impl<'data, Elf: FileHeader, R: ReadRef<'data>> SectionTable<'data, Elf, R> {
    /// Create a new section table.
    #[inline]
    pub fn new(sections: &'data [Elf::SectionHeader], strings: StringTable<'data, R>) -> Self {
        SectionTable { sections, strings }
    }

    /// Iterate over the section headers.
    ///
    /// This includes the null section at index 0, which you will usually need to skip.
    #[inline]
    pub fn iter(&self) -> slice::Iter<'data, Elf::SectionHeader> {
        self.sections.iter()
    }

    /// Iterate over the section headers and their indices.
    ///
    /// This includes the null section at index 0, which you will usually need to skip.
    #[inline]
    pub fn enumerate(&self) -> impl Iterator<Item = (SectionIndex, &'data Elf::SectionHeader)> {
        self.sections
            .iter()
            .enumerate()
            .map(|(i, section)| (SectionIndex(i), section))
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

    /// Get the section header at the given index.
    ///
    /// Returns an error for the null section at index 0.
    pub fn section(&self, index: SectionIndex) -> read::Result<&'data Elf::SectionHeader> {
        if index == SectionIndex(0) {
            return Err(read::Error("Invalid ELF section index"));
        }
        self.sections
            .get(index.0)
            .read_error("Invalid ELF section index")
    }

    /// Return the section header with the given name.
    ///
    /// Ignores sections with invalid names.
    pub fn section_by_name(
        &self,
        endian: Elf::Endian,
        name: &[u8],
    ) -> Option<(SectionIndex, &'data Elf::SectionHeader)> {
        self.enumerate()
            .find(|(_, section)| self.section_name(endian, section) == Ok(name))
    }

    /// Return the section name for the given section header.
    pub fn section_name(
        &self,
        endian: Elf::Endian,
        section: &Elf::SectionHeader,
    ) -> read::Result<&'data [u8]> {
        section.name(endian, self.strings)
    }

    /// Return the string table at the given section index.
    ///
    /// Returns an empty string table if the index is 0.
    /// Returns an error if the section is not a string table.
    #[inline]
    pub fn strings(
        &self,
        endian: Elf::Endian,
        data: R,
        index: SectionIndex,
    ) -> read::Result<StringTable<'data, R>> {
        if index == SectionIndex(0) {
            return Ok(StringTable::default());
        }
        self.section(index)?
            .strings(endian, data)?
            .read_error("Invalid ELF string section type")
    }

    /// Return the symbol table of the given section type.
    ///
    /// Returns an empty symbol table if the symbol table does not exist.
    #[inline]
    pub fn symbols(
        &self,
        endian: Elf::Endian,
        data: R,
        sh_type: u32,
    ) -> read::Result<SymbolTable<'data, Elf, R>> {
        debug_assert!(sh_type == elf::SHT_DYNSYM || sh_type == elf::SHT_SYMTAB);

        let (index, section) = match self.enumerate().find(|s| s.1.sh_type(endian) == sh_type) {
            Some(s) => s,
            None => return Ok(SymbolTable::default()),
        };

        SymbolTable::parse(endian, data, self, index, section)
    }

    /// Return the symbol table at the given section index.
    ///
    /// Returns an error if the section is not a symbol table.
    #[inline]
    pub fn symbol_table_by_index(
        &self,
        endian: Elf::Endian,
        data: R,
        index: SectionIndex,
    ) -> read::Result<SymbolTable<'data, Elf, R>> {
        let section = self.section(index)?;
        match section.sh_type(endian) {
            elf::SHT_DYNSYM | elf::SHT_SYMTAB => {}
            _ => return Err(Error("Invalid ELF symbol table section type")),
        }
        SymbolTable::parse(endian, data, self, index, section)
    }

    /// Create a mapping from section index to associated relocation sections.
    #[inline]
    pub fn relocation_sections(
        &self,
        endian: Elf::Endian,
        symbol_section: SectionIndex,
    ) -> read::Result<RelocationSections> {
        RelocationSections::parse(endian, self, symbol_section)
    }

    /// Return the contents of a dynamic section.
    ///
    /// Also returns the linked string table index.
    ///
    /// Returns `Ok(None)` if there is no `SHT_DYNAMIC` section.
    /// Returns `Err` for invalid values.
    pub fn dynamic(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<(&'data [Elf::Dyn], SectionIndex)>> {
        for section in self.sections {
            if let Some(dynamic) = section.dynamic(endian, data)? {
                return Ok(Some(dynamic));
            }
        }
        Ok(None)
    }

    /// Return the header of a SysV hash section.
    ///
    /// Returns `Ok(None)` if there is no SysV GNU hash section.
    /// Returns `Err` for invalid values.
    pub fn hash_header(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<&'data elf::HashHeader<Elf::Endian>>> {
        for section in self.sections {
            if let Some(hash) = section.hash_header(endian, data)? {
                return Ok(Some(hash));
            }
        }
        Ok(None)
    }

    /// Return the contents of a SysV hash section.
    ///
    /// Also returns the linked symbol table index.
    ///
    /// Returns `Ok(None)` if there is no SysV hash section.
    /// Returns `Err` for invalid values.
    pub fn hash(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<(HashTable<'data, Elf>, SectionIndex)>> {
        for section in self.sections {
            if let Some(hash) = section.hash(endian, data)? {
                return Ok(Some(hash));
            }
        }
        Ok(None)
    }

    /// Return the header of a GNU hash section.
    ///
    /// Returns `Ok(None)` if there is no GNU hash section.
    /// Returns `Err` for invalid values.
    pub fn gnu_hash_header(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<&'data elf::GnuHashHeader<Elf::Endian>>> {
        for section in self.sections {
            if let Some(hash) = section.gnu_hash_header(endian, data)? {
                return Ok(Some(hash));
            }
        }
        Ok(None)
    }

    /// Return the contents of a GNU hash section.
    ///
    /// Also returns the linked symbol table index.
    ///
    /// Returns `Ok(None)` if there is no GNU hash section.
    /// Returns `Err` for invalid values.
    pub fn gnu_hash(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<(GnuHashTable<'data, Elf>, SectionIndex)>> {
        for section in self.sections {
            if let Some(hash) = section.gnu_hash(endian, data)? {
                return Ok(Some(hash));
            }
        }
        Ok(None)
    }

    /// Return the contents of a `SHT_GNU_VERSYM` section.
    ///
    /// Also returns the linked symbol table index.
    ///
    /// Returns `Ok(None)` if there is no `SHT_GNU_VERSYM` section.
    /// Returns `Err` for invalid values.
    pub fn gnu_versym(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<(&'data [elf::Versym<Elf::Endian>], SectionIndex)>> {
        for section in self.sections {
            if let Some(syms) = section.gnu_versym(endian, data)? {
                return Ok(Some(syms));
            }
        }
        Ok(None)
    }

    /// Return the contents of a `SHT_GNU_VERDEF` section.
    ///
    /// Also returns the linked string table index.
    ///
    /// Returns `Ok(None)` if there is no `SHT_GNU_VERDEF` section.
    /// Returns `Err` for invalid values.
    pub fn gnu_verdef(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<(VerdefIterator<'data, Elf>, SectionIndex)>> {
        for section in self.sections {
            if let Some(defs) = section.gnu_verdef(endian, data)? {
                return Ok(Some(defs));
            }
        }
        Ok(None)
    }

    /// Return the contents of a `SHT_GNU_VERNEED` section.
    ///
    /// Also returns the linked string table index.
    ///
    /// Returns `Ok(None)` if there is no `SHT_GNU_VERNEED` section.
    /// Returns `Err` for invalid values.
    pub fn gnu_verneed(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<(VerneedIterator<'data, Elf>, SectionIndex)>> {
        for section in self.sections {
            if let Some(needs) = section.gnu_verneed(endian, data)? {
                return Ok(Some(needs));
            }
        }
        Ok(None)
    }

    /// Returns the symbol version table.
    ///
    /// Returns `Ok(None)` if there is no `SHT_GNU_VERSYM` section.
    /// Returns `Err` for invalid values.
    pub fn versions(
        &self,
        endian: Elf::Endian,
        data: R,
    ) -> read::Result<Option<VersionTable<'data, Elf>>> {
        let (versyms, link) = match self.gnu_versym(endian, data)? {
            Some(val) => val,
            None => return Ok(None),
        };
        let strings = self.symbol_table_by_index(endian, data, link)?.strings();
        // TODO: check links?
        let verdefs = self.gnu_verdef(endian, data)?.map(|x| x.0);
        let verneeds = self.gnu_verneed(endian, data)?.map(|x| x.0);
        VersionTable::parse(endian, versyms, verdefs, verneeds, strings).map(Some)
    }
}

/// An iterator for the sections in an [`ElfFile32`](super::ElfFile32).
pub type ElfSectionIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSectionIterator<'data, 'file, elf::FileHeader32<Endian>, R>;
/// An iterator for the sections in an [`ElfFile64`](super::ElfFile64).
pub type ElfSectionIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSectionIterator<'data, 'file, elf::FileHeader64<Endian>, R>;

/// An iterator for the sections in an [`ElfFile`].
#[derive(Debug)]
pub struct ElfSectionIterator<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    file: &'file ElfFile<'data, Elf, R>,
    iter: iter::Enumerate<slice::Iter<'data, Elf::SectionHeader>>,
}

impl<'data, 'file, Elf, R> ElfSectionIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) fn new(file: &'file ElfFile<'data, Elf, R>) -> Self {
        let mut iter = file.sections.iter().enumerate();
        iter.next(); // Skip null section.
        ElfSectionIterator { file, iter }
    }
}

impl<'data, 'file, Elf, R> Iterator for ElfSectionIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    type Item = ElfSection<'data, 'file, Elf, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(index, section)| ElfSection {
            index: SectionIndex(index),
            file: self.file,
            section,
        })
    }
}

/// A section in an [`ElfFile32`](super::ElfFile32).
pub type ElfSection32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSection<'data, 'file, elf::FileHeader32<Endian>, R>;
/// A section in an [`ElfFile64`](super::ElfFile64).
pub type ElfSection64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSection<'data, 'file, elf::FileHeader64<Endian>, R>;

/// A section in an [`ElfFile`].
///
/// Most functionality is provided by the [`ObjectSection`] trait implementation.
#[derive(Debug)]
pub struct ElfSection<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) file: &'file ElfFile<'data, Elf, R>,
    pub(super) index: SectionIndex,
    pub(super) section: &'data Elf::SectionHeader,
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> ElfSection<'data, 'file, Elf, R> {
    /// Get the ELF file containing this section.
    pub fn elf_file(&self) -> &'file ElfFile<'data, Elf, R> {
        self.file
    }

    /// Get the raw ELF section header.
    pub fn elf_section_header(&self) -> &'data Elf::SectionHeader {
        self.section
    }

    /// Get the index of the relocation section that references this section.
    ///
    /// Returns `None` if there are no relocations.
    /// Returns an error if there are multiple relocation sections that reference this section.
    pub fn elf_relocation_section_index(&self) -> read::Result<Option<SectionIndex>> {
        let Some(relocation_index) = self.file.relocations.get(self.index) else {
            return Ok(None);
        };
        if self.file.relocations.get(relocation_index).is_some() {
            return Err(Error(
                "Unsupported ELF section with multiple relocation sections",
            ));
        }
        Ok(Some(relocation_index))
    }

    /// Get the relocation section that references this section.
    ///
    /// Returns `None` if there are no relocations.
    /// Returns an error if there are multiple relocation sections that reference this section.
    pub fn elf_relocation_section(&self) -> read::Result<Option<&'data Elf::SectionHeader>> {
        let Some(relocation_index) = self.elf_relocation_section_index()? else {
            return Ok(None);
        };
        self.file.sections.section(relocation_index).map(Some)
    }

    /// Get the `Elf::Rel` entries that apply to this section.
    ///
    /// Returns an empty slice if there are no relocations.
    /// Returns an error if there are multiple relocation sections that reference this section.
    pub fn elf_linked_rel(&self) -> read::Result<&'data [Elf::Rel]> {
        let Some(relocation_section) = self.elf_relocation_section()? else {
            return Ok(&[]);
        };
        // The linked symbol table was already checked when self.file.relocations was created.
        let Some((rel, _)) = relocation_section.rel(self.file.endian, self.file.data)? else {
            return Ok(&[]);
        };
        Ok(rel)
    }

    /// Get the `Elf::Rela` entries that apply to this section.
    ///
    /// Returns an empty slice if there are no relocations.
    /// Returns an error if there are multiple relocation sections that reference this section.
    pub fn elf_linked_rela(&self) -> read::Result<&'data [Elf::Rela]> {
        let Some(relocation_section) = self.elf_relocation_section()? else {
            return Ok(&[]);
        };
        // The linked symbol table was already checked when self.file.relocations was created.
        let Some((rela, _)) = relocation_section.rela(self.file.endian, self.file.data)? else {
            return Ok(&[]);
        };
        Ok(rela)
    }

    fn bytes(&self) -> read::Result<&'data [u8]> {
        self.section
            .data(self.file.endian, self.file.data)
            .read_error("Invalid ELF section size or offset")
    }

    fn maybe_compressed(&self) -> read::Result<Option<CompressedFileRange>> {
        let endian = self.file.endian;
        if let Some((header, offset, compressed_size)) =
            self.section.compression(endian, self.file.data)?
        {
            let format = match header.ch_type(endian) {
                elf::ELFCOMPRESS_ZLIB => CompressionFormat::Zlib,
                elf::ELFCOMPRESS_ZSTD => CompressionFormat::Zstandard,
                _ => return Err(Error("Unsupported ELF compression type")),
            };
            let uncompressed_size = header.ch_size(endian).into();
            Ok(Some(CompressedFileRange {
                format,
                offset,
                compressed_size,
                uncompressed_size,
            }))
        } else {
            Ok(None)
        }
    }

    // Try GNU-style "ZLIB" header decompression.
    fn maybe_compressed_gnu(&self) -> read::Result<Option<CompressedFileRange>> {
        if !self
            .name()
            .map_or(false, |name| name.starts_with(".zdebug_"))
        {
            return Ok(None);
        }
        let (section_offset, section_size) = self
            .file_range()
            .read_error("Invalid ELF GNU compressed section type")?;
        gnu_compression::compressed_file_range(self.file.data, section_offset, section_size)
            .map(Some)
    }
}

impl<'data, 'file, Elf, R> read::private::Sealed for ElfSection<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Elf, R> ObjectSection<'data> for ElfSection<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    type RelocationIterator = ElfSectionRelocationIterator<'data, 'file, Elf, R>;

    #[inline]
    fn index(&self) -> SectionIndex {
        self.index
    }

    #[inline]
    fn address(&self) -> u64 {
        self.section.sh_addr(self.file.endian).into()
    }

    #[inline]
    fn size(&self) -> u64 {
        self.section.sh_size(self.file.endian).into()
    }

    #[inline]
    fn align(&self) -> u64 {
        self.section.sh_addralign(self.file.endian).into()
    }

    #[inline]
    fn file_range(&self) -> Option<(u64, u64)> {
        self.section.file_range(self.file.endian)
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

    fn compressed_file_range(&self) -> read::Result<CompressedFileRange> {
        Ok(if let Some(data) = self.maybe_compressed()? {
            data
        } else if let Some(data) = self.maybe_compressed_gnu()? {
            data
        } else {
            CompressedFileRange::none(self.file_range())
        })
    }

    fn compressed_data(&self) -> read::Result<CompressedData<'data>> {
        self.compressed_file_range()?.data(self.file.data)
    }

    fn name_bytes(&self) -> read::Result<&'data [u8]> {
        self.file
            .sections
            .section_name(self.file.endian, self.section)
    }

    fn name(&self) -> read::Result<&'data str> {
        let name = self.name_bytes()?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 ELF section name")
    }

    #[inline]
    fn segment_name_bytes(&self) -> read::Result<Option<&[u8]>> {
        Ok(None)
    }

    #[inline]
    fn segment_name(&self) -> read::Result<Option<&str>> {
        Ok(None)
    }

    fn kind(&self) -> SectionKind {
        let flags = self.section.sh_flags(self.file.endian).into();
        let sh_type = self.section.sh_type(self.file.endian);
        match sh_type {
            elf::SHT_PROGBITS => {
                if flags & u64::from(elf::SHF_ALLOC) != 0 {
                    if flags & u64::from(elf::SHF_EXECINSTR) != 0 {
                        SectionKind::Text
                    } else if flags & u64::from(elf::SHF_TLS) != 0 {
                        SectionKind::Tls
                    } else if flags & u64::from(elf::SHF_WRITE) != 0 {
                        SectionKind::Data
                    } else if flags & u64::from(elf::SHF_STRINGS) != 0 {
                        SectionKind::ReadOnlyString
                    } else {
                        SectionKind::ReadOnlyData
                    }
                } else if flags & u64::from(elf::SHF_STRINGS) != 0 {
                    SectionKind::OtherString
                } else {
                    SectionKind::Other
                }
            }
            elf::SHT_NOBITS => {
                if flags & u64::from(elf::SHF_TLS) != 0 {
                    SectionKind::UninitializedTls
                } else {
                    SectionKind::UninitializedData
                }
            }
            elf::SHT_NOTE => SectionKind::Note,
            elf::SHT_NULL
            | elf::SHT_SYMTAB
            | elf::SHT_STRTAB
            | elf::SHT_RELA
            | elf::SHT_HASH
            | elf::SHT_DYNAMIC
            | elf::SHT_REL
            | elf::SHT_DYNSYM
            | elf::SHT_GROUP
            | elf::SHT_SYMTAB_SHNDX
            | elf::SHT_RELR => SectionKind::Metadata,
            _ => SectionKind::Elf(sh_type),
        }
    }

    fn relocations(&self) -> ElfSectionRelocationIterator<'data, 'file, Elf, R> {
        ElfSectionRelocationIterator {
            section_index: self.index,
            file: self.file,
            relocations: None,
        }
    }

    fn relocation_map(&self) -> read::Result<RelocationMap> {
        RelocationMap::new(self.file, self)
    }

    fn flags(&self) -> SectionFlags {
        SectionFlags::Elf {
            sh_flags: self.section.sh_flags(self.file.endian).into(),
        }
    }
}

/// A trait for generic access to [`elf::SectionHeader32`] and [`elf::SectionHeader64`].
#[allow(missing_docs)]
pub trait SectionHeader: Debug + Pod {
    type Elf: FileHeader<SectionHeader = Self, Endian = Self::Endian, Word = Self::Word>;
    type Word: Into<u64>;
    type Endian: endian::Endian;

    fn sh_name(&self, endian: Self::Endian) -> u32;
    fn sh_type(&self, endian: Self::Endian) -> u32;
    fn sh_flags(&self, endian: Self::Endian) -> Self::Word;
    fn sh_addr(&self, endian: Self::Endian) -> Self::Word;
    fn sh_offset(&self, endian: Self::Endian) -> Self::Word;
    fn sh_size(&self, endian: Self::Endian) -> Self::Word;
    fn sh_link(&self, endian: Self::Endian) -> u32;
    fn sh_info(&self, endian: Self::Endian) -> u32;
    fn sh_addralign(&self, endian: Self::Endian) -> Self::Word;
    fn sh_entsize(&self, endian: Self::Endian) -> Self::Word;

    /// Parse the section name from the string table.
    fn name<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        strings: StringTable<'data, R>,
    ) -> read::Result<&'data [u8]> {
        strings
            .get(self.sh_name(endian))
            .read_error("Invalid ELF section name offset")
    }

    /// Get the `sh_link` field as a section index.
    ///
    /// This may return a null section index, and does not check for validity.
    fn link(&self, endian: Self::Endian) -> SectionIndex {
        SectionIndex(self.sh_link(endian) as usize)
    }

    /// Return true if the `SHF_INFO_LINK` flag is set.
    fn has_info_link(&self, endian: Self::Endian) -> bool {
        self.sh_flags(endian).into() & u64::from(elf::SHF_INFO_LINK) != 0
    }

    /// Get the `sh_info` field as a section index.
    ///
    /// This does not check the `SHF_INFO_LINK` flag.
    /// This may return a null section index, and does not check for validity.
    fn info_link(&self, endian: Self::Endian) -> SectionIndex {
        SectionIndex(self.sh_info(endian) as usize)
    }

    /// Return the offset and size of the section in the file.
    ///
    /// Returns `None` for sections that have no data in the file.
    fn file_range(&self, endian: Self::Endian) -> Option<(u64, u64)> {
        if self.sh_type(endian) == elf::SHT_NOBITS {
            None
        } else {
            Some((self.sh_offset(endian).into(), self.sh_size(endian).into()))
        }
    }

    /// Return the section data.
    ///
    /// Returns `Ok(&[])` if the section has no data.
    /// Returns `Err` for invalid values.
    fn data<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<&'data [u8]> {
        if let Some((offset, size)) = self.file_range(endian) {
            data.read_bytes_at(offset, size)
                .read_error("Invalid ELF section size or offset")
        } else {
            Ok(&[])
        }
    }

    /// Return the section data as a slice of the given type.
    ///
    /// Allows padding at the end of the data.
    /// Returns `Ok(&[])` if the section has no data.
    /// Returns `Err` for invalid values, including bad alignment.
    fn data_as_array<'data, T: Pod, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<&'data [T]> {
        pod::slice_from_all_bytes(self.data(endian, data)?)
            .read_error("Invalid ELF section size or offset")
    }

    /// Return the strings in the section.
    ///
    /// Returns `Ok(None)` if the section does not contain strings.
    /// Returns `Err` for invalid values.
    fn strings<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<StringTable<'data, R>>> {
        if self.sh_type(endian) != elf::SHT_STRTAB {
            return Ok(None);
        }
        let str_offset = self.sh_offset(endian).into();
        let str_size = self.sh_size(endian).into();
        let str_end = str_offset
            .checked_add(str_size)
            .read_error("Invalid ELF string section offset or size")?;
        Ok(Some(StringTable::new(data, str_offset, str_end)))
    }

    /// Return the symbols in the section.
    ///
    /// Also finds the linked string table in `sections`.
    ///
    /// `section_index` must be the 0-based index of this section, and is used
    /// to find the corresponding extended section index table in `sections`.
    ///
    /// Returns `Ok(None)` if the section does not contain symbols.
    /// Returns `Err` for invalid values.
    fn symbols<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
        sections: &SectionTable<'data, Self::Elf, R>,
        section_index: SectionIndex,
    ) -> read::Result<Option<SymbolTable<'data, Self::Elf, R>>> {
        let sh_type = self.sh_type(endian);
        if sh_type != elf::SHT_SYMTAB && sh_type != elf::SHT_DYNSYM {
            return Ok(None);
        }
        SymbolTable::parse(endian, data, sections, section_index, self).map(Some)
    }

    /// Return the `Elf::Rel` entries in the section.
    ///
    /// Also returns the linked symbol table index.
    ///
    /// Returns `Ok(None)` if the section does not contain relocations.
    /// Returns `Err` for invalid values.
    fn rel<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(&'data [<Self::Elf as FileHeader>::Rel], SectionIndex)>> {
        if self.sh_type(endian) != elf::SHT_REL {
            return Ok(None);
        }
        let rel = self
            .data_as_array(endian, data)
            .read_error("Invalid ELF relocation section offset or size")?;
        Ok(Some((rel, self.link(endian))))
    }

    /// Return the `Elf::Rela` entries in the section.
    ///
    /// Also returns the linked symbol table index.
    ///
    /// Returns `Ok(None)` if the section does not contain relocations.
    /// Returns `Err` for invalid values.
    fn rela<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(&'data [<Self::Elf as FileHeader>::Rela], SectionIndex)>> {
        if self.sh_type(endian) != elf::SHT_RELA {
            return Ok(None);
        }
        let rela = self
            .data_as_array(endian, data)
            .read_error("Invalid ELF relocation section offset or size")?;
        Ok(Some((rela, self.link(endian))))
    }

    /// Return the `Elf::Relr` entries in the section.
    ///
    /// Returns `Ok(None)` if the section does not contain relative relocations.
    /// Returns `Err` for invalid values.
    fn relr<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<RelrIterator<'data, Self::Elf>>> {
        if self.sh_type(endian) != elf::SHT_RELR {
            return Ok(None);
        }
        let data = self
            .data_as_array(endian, data)
            .read_error("Invalid ELF relocation section offset or size")?;
        let relrs = RelrIterator::new(endian, data);
        Ok(Some(relrs))
    }

    /// Return entries in a dynamic section.
    ///
    /// Also returns the linked string table index.
    ///
    /// Returns `Ok(None)` if the section type is not `SHT_DYNAMIC`.
    /// Returns `Err` for invalid values.
    fn dynamic<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(&'data [<Self::Elf as FileHeader>::Dyn], SectionIndex)>> {
        if self.sh_type(endian) != elf::SHT_DYNAMIC {
            return Ok(None);
        }
        let dynamic = self
            .data_as_array(endian, data)
            .read_error("Invalid ELF dynamic section offset or size")?;
        Ok(Some((dynamic, self.link(endian))))
    }

    /// Return a note iterator for the section data.
    ///
    /// Returns `Ok(None)` if the section does not contain notes.
    /// Returns `Err` for invalid values.
    fn notes<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<NoteIterator<'data, Self::Elf>>> {
        if self.sh_type(endian) != elf::SHT_NOTE {
            return Ok(None);
        }
        let data = self
            .data(endian, data)
            .read_error("Invalid ELF note section offset or size")?;
        let notes = NoteIterator::new(endian, self.sh_addralign(endian), data)?;
        Ok(Some(notes))
    }

    /// Return the contents of a group section.
    ///
    /// The first value is a `GRP_*` value, and the remaining values
    /// are section indices.
    ///
    /// Returns `Ok(None)` if the section does not define a group.
    /// Returns `Err` for invalid values.
    fn group<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(u32, &'data [U32Bytes<Self::Endian>])>> {
        if self.sh_type(endian) != elf::SHT_GROUP {
            return Ok(None);
        }
        let msg = "Invalid ELF group section offset or size";
        let data = self.data(endian, data).read_error(msg)?;
        let (flag, data) = pod::from_bytes::<U32Bytes<_>>(data).read_error(msg)?;
        let sections = pod::slice_from_all_bytes(data).read_error(msg)?;
        Ok(Some((flag.get(endian), sections)))
    }

    /// Return the header of a SysV hash section.
    ///
    /// Returns `Ok(None)` if the section does not contain a SysV hash.
    /// Returns `Err` for invalid values.
    fn hash_header<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<&'data elf::HashHeader<Self::Endian>>> {
        if self.sh_type(endian) != elf::SHT_HASH {
            return Ok(None);
        }
        let data = self
            .data(endian, data)
            .read_error("Invalid ELF hash section offset or size")?;
        let header = data
            .read_at::<elf::HashHeader<Self::Endian>>(0)
            .read_error("Invalid hash header")?;
        Ok(Some(header))
    }

    /// Return the contents of a SysV hash section.
    ///
    /// Also returns the linked symbol table index.
    ///
    /// Returns `Ok(None)` if the section does not contain a SysV hash.
    /// Returns `Err` for invalid values.
    fn hash<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(HashTable<'data, Self::Elf>, SectionIndex)>> {
        if self.sh_type(endian) != elf::SHT_HASH {
            return Ok(None);
        }
        let data = self
            .data(endian, data)
            .read_error("Invalid ELF hash section offset or size")?;
        let hash = HashTable::parse(endian, data)?;
        Ok(Some((hash, self.link(endian))))
    }

    /// Return the header of a GNU hash section.
    ///
    /// Returns `Ok(None)` if the section does not contain a GNU hash.
    /// Returns `Err` for invalid values.
    fn gnu_hash_header<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<&'data elf::GnuHashHeader<Self::Endian>>> {
        if self.sh_type(endian) != elf::SHT_GNU_HASH {
            return Ok(None);
        }
        let data = self
            .data(endian, data)
            .read_error("Invalid ELF GNU hash section offset or size")?;
        let header = data
            .read_at::<elf::GnuHashHeader<Self::Endian>>(0)
            .read_error("Invalid GNU hash header")?;
        Ok(Some(header))
    }

    /// Return the contents of a GNU hash section.
    ///
    /// Also returns the linked symbol table index.
    ///
    /// Returns `Ok(None)` if the section does not contain a GNU hash.
    /// Returns `Err` for invalid values.
    fn gnu_hash<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(GnuHashTable<'data, Self::Elf>, SectionIndex)>> {
        if self.sh_type(endian) != elf::SHT_GNU_HASH {
            return Ok(None);
        }
        let data = self
            .data(endian, data)
            .read_error("Invalid ELF GNU hash section offset or size")?;
        let hash = GnuHashTable::parse(endian, data)?;
        Ok(Some((hash, self.link(endian))))
    }

    /// Return the contents of a `SHT_GNU_VERSYM` section.
    ///
    /// Also returns the linked symbol table index.
    ///
    /// Returns `Ok(None)` if the section type is not `SHT_GNU_VERSYM`.
    /// Returns `Err` for invalid values.
    fn gnu_versym<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(&'data [elf::Versym<Self::Endian>], SectionIndex)>> {
        if self.sh_type(endian) != elf::SHT_GNU_VERSYM {
            return Ok(None);
        }
        let versym = self
            .data_as_array(endian, data)
            .read_error("Invalid ELF GNU versym section offset or size")?;
        Ok(Some((versym, self.link(endian))))
    }

    /// Return an iterator for the entries of a `SHT_GNU_VERDEF` section.
    ///
    /// Also returns the linked string table index.
    ///
    /// Returns `Ok(None)` if the section type is not `SHT_GNU_VERDEF`.
    /// Returns `Err` for invalid values.
    fn gnu_verdef<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(VerdefIterator<'data, Self::Elf>, SectionIndex)>> {
        if self.sh_type(endian) != elf::SHT_GNU_VERDEF {
            return Ok(None);
        }
        let verdef = self
            .data(endian, data)
            .read_error("Invalid ELF GNU verdef section offset or size")?;
        Ok(Some((
            VerdefIterator::new(endian, verdef),
            self.link(endian),
        )))
    }

    /// Return an iterator for the entries of a `SHT_GNU_VERNEED` section.
    ///
    /// Also returns the linked string table index.
    ///
    /// Returns `Ok(None)` if the section type is not `SHT_GNU_VERNEED`.
    /// Returns `Err` for invalid values.
    fn gnu_verneed<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<(VerneedIterator<'data, Self::Elf>, SectionIndex)>> {
        if self.sh_type(endian) != elf::SHT_GNU_VERNEED {
            return Ok(None);
        }
        let verneed = self
            .data(endian, data)
            .read_error("Invalid ELF GNU verneed section offset or size")?;
        Ok(Some((
            VerneedIterator::new(endian, verneed),
            self.link(endian),
        )))
    }

    /// Return the contents of a `SHT_GNU_ATTRIBUTES` section.
    ///
    /// Returns `Ok(None)` if the section type is not `SHT_GNU_ATTRIBUTES`.
    /// Returns `Err` for invalid values.
    fn gnu_attributes<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<Option<AttributesSection<'data, Self::Elf>>> {
        if self.sh_type(endian) != elf::SHT_GNU_ATTRIBUTES {
            return Ok(None);
        }
        self.attributes(endian, data).map(Some)
    }

    /// Parse the contents of the section as attributes.
    ///
    /// This function does not check whether section type corresponds
    /// to a section that contains attributes.
    ///
    /// Returns `Err` for invalid values.
    fn attributes<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<AttributesSection<'data, Self::Elf>> {
        let data = self.data(endian, data)?;
        AttributesSection::new(endian, data)
    }

    /// Parse the compression header if present.
    ///
    /// Returns the header, and the offset and size of the compressed section data
    /// in the file.
    ///
    /// Returns `Ok(None)` if the section flags do not have `SHF_COMPRESSED`.
    /// Returns `Err` for invalid values.
    fn compression<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> read::Result<
        Option<(
            &'data <Self::Elf as FileHeader>::CompressionHeader,
            u64,
            u64,
        )>,
    > {
        if (self.sh_flags(endian).into() & u64::from(elf::SHF_COMPRESSED)) == 0 {
            return Ok(None);
        }
        let (section_offset, section_size) = self
            .file_range(endian)
            .read_error("Invalid ELF compressed section type")?;
        let mut offset = section_offset;
        let header = data
            .read::<<Self::Elf as FileHeader>::CompressionHeader>(&mut offset)
            .read_error("Invalid ELF compressed section offset")?;
        let compressed_size = section_size
            .checked_sub(offset - section_offset)
            .read_error("Invalid ELF compressed section size")?;
        Ok(Some((header, offset, compressed_size)))
    }
}

impl<Endian: endian::Endian> SectionHeader for elf::SectionHeader32<Endian> {
    type Elf = elf::FileHeader32<Endian>;
    type Word = u32;
    type Endian = Endian;

    #[inline]
    fn sh_name(&self, endian: Self::Endian) -> u32 {
        self.sh_name.get(endian)
    }

    #[inline]
    fn sh_type(&self, endian: Self::Endian) -> u32 {
        self.sh_type.get(endian)
    }

    #[inline]
    fn sh_flags(&self, endian: Self::Endian) -> Self::Word {
        self.sh_flags.get(endian)
    }

    #[inline]
    fn sh_addr(&self, endian: Self::Endian) -> Self::Word {
        self.sh_addr.get(endian)
    }

    #[inline]
    fn sh_offset(&self, endian: Self::Endian) -> Self::Word {
        self.sh_offset.get(endian)
    }

    #[inline]
    fn sh_size(&self, endian: Self::Endian) -> Self::Word {
        self.sh_size.get(endian)
    }

    #[inline]
    fn sh_link(&self, endian: Self::Endian) -> u32 {
        self.sh_link.get(endian)
    }

    #[inline]
    fn sh_info(&self, endian: Self::Endian) -> u32 {
        self.sh_info.get(endian)
    }

    #[inline]
    fn sh_addralign(&self, endian: Self::Endian) -> Self::Word {
        self.sh_addralign.get(endian)
    }

    #[inline]
    fn sh_entsize(&self, endian: Self::Endian) -> Self::Word {
        self.sh_entsize.get(endian)
    }
}

impl<Endian: endian::Endian> SectionHeader for elf::SectionHeader64<Endian> {
    type Word = u64;
    type Endian = Endian;
    type Elf = elf::FileHeader64<Endian>;

    #[inline]
    fn sh_name(&self, endian: Self::Endian) -> u32 {
        self.sh_name.get(endian)
    }

    #[inline]
    fn sh_type(&self, endian: Self::Endian) -> u32 {
        self.sh_type.get(endian)
    }

    #[inline]
    fn sh_flags(&self, endian: Self::Endian) -> Self::Word {
        self.sh_flags.get(endian)
    }

    #[inline]
    fn sh_addr(&self, endian: Self::Endian) -> Self::Word {
        self.sh_addr.get(endian)
    }

    #[inline]
    fn sh_offset(&self, endian: Self::Endian) -> Self::Word {
        self.sh_offset.get(endian)
    }

    #[inline]
    fn sh_size(&self, endian: Self::Endian) -> Self::Word {
        self.sh_size.get(endian)
    }

    #[inline]
    fn sh_link(&self, endian: Self::Endian) -> u32 {
        self.sh_link.get(endian)
    }

    #[inline]
    fn sh_info(&self, endian: Self::Endian) -> u32 {
        self.sh_info.get(endian)
    }

    #[inline]
    fn sh_addralign(&self, endian: Self::Endian) -> Self::Word {
        self.sh_addralign.get(endian)
    }

    #[inline]
    fn sh_entsize(&self, endian: Self::Endian) -> Self::Word {
        self.sh_entsize.get(endian)
    }
}
