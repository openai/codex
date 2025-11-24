use alloc::fmt;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::slice;
use core::str;

use crate::elf;
use crate::endian::{self, Endianness, U32};
use crate::pod::Pod;
use crate::read::util::StringTable;
use crate::read::{
    self, ObjectSymbol, ObjectSymbolTable, ReadError, ReadRef, SectionIndex, SymbolFlags,
    SymbolIndex, SymbolKind, SymbolMap, SymbolMapEntry, SymbolScope, SymbolSection,
};

use super::{FileHeader, SectionHeader, SectionTable};

/// A table of symbol entries in an ELF file.
///
/// Also includes the string table used for the symbol names.
///
/// Returned by [`SectionTable::symbols`].
#[derive(Debug, Clone, Copy)]
pub struct SymbolTable<'data, Elf: FileHeader, R = &'data [u8]>
where
    R: ReadRef<'data>,
{
    section: SectionIndex,
    string_section: SectionIndex,
    shndx_section: SectionIndex,
    symbols: &'data [Elf::Sym],
    strings: StringTable<'data, R>,
    shndx: &'data [U32<Elf::Endian>],
}

impl<'data, Elf: FileHeader, R: ReadRef<'data>> Default for SymbolTable<'data, Elf, R> {
    fn default() -> Self {
        SymbolTable {
            section: SectionIndex(0),
            string_section: SectionIndex(0),
            shndx_section: SectionIndex(0),
            symbols: &[],
            strings: Default::default(),
            shndx: &[],
        }
    }
}

impl<'data, Elf: FileHeader, R: ReadRef<'data>> SymbolTable<'data, Elf, R> {
    /// Parse the given symbol table section.
    pub fn parse(
        endian: Elf::Endian,
        data: R,
        sections: &SectionTable<'data, Elf, R>,
        section_index: SectionIndex,
        section: &Elf::SectionHeader,
    ) -> read::Result<SymbolTable<'data, Elf, R>> {
        debug_assert!(
            section.sh_type(endian) == elf::SHT_DYNSYM
                || section.sh_type(endian) == elf::SHT_SYMTAB
        );

        let symbols = section
            .data_as_array(endian, data)
            .read_error("Invalid ELF symbol table data")?;

        let link = SectionIndex(section.sh_link(endian) as usize);
        let strings = sections.strings(endian, data, link)?;

        let mut shndx_section = SectionIndex(0);
        let mut shndx = &[][..];
        for (i, s) in sections.enumerate() {
            if s.sh_type(endian) == elf::SHT_SYMTAB_SHNDX && s.link(endian) == section_index {
                shndx_section = i;
                shndx = s
                    .data_as_array(endian, data)
                    .read_error("Invalid ELF symtab_shndx data")?;
            }
        }

        Ok(SymbolTable {
            section: section_index,
            string_section: link,
            symbols,
            strings,
            shndx,
            shndx_section,
        })
    }

    /// Return the section index of this symbol table.
    #[inline]
    pub fn section(&self) -> SectionIndex {
        self.section
    }

    /// Return the section index of the shndx table.
    #[inline]
    pub fn shndx_section(&self) -> SectionIndex {
        self.shndx_section
    }

    /// Return the section index of the linked string table.
    #[inline]
    pub fn string_section(&self) -> SectionIndex {
        self.string_section
    }

    /// Return the string table used for the symbol names.
    #[inline]
    pub fn strings(&self) -> StringTable<'data, R> {
        self.strings
    }

    /// Return the symbol table.
    #[inline]
    pub fn symbols(&self) -> &'data [Elf::Sym] {
        self.symbols
    }

    /// Iterate over the symbols.
    ///
    /// This includes the null symbol at index 0, which you will usually need to skip.
    #[inline]
    pub fn iter(&self) -> slice::Iter<'data, Elf::Sym> {
        self.symbols.iter()
    }

    /// Iterate over the symbols and their indices.
    ///
    /// This includes the null symbol at index 0, which you will usually need to skip.
    #[inline]
    pub fn enumerate(&self) -> impl Iterator<Item = (SymbolIndex, &'data Elf::Sym)> {
        self.symbols
            .iter()
            .enumerate()
            .map(|(i, sym)| (SymbolIndex(i), sym))
    }

    /// Return true if the symbol table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// The number of symbols.
    #[inline]
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Get the symbol at the given index.
    ///
    /// Returns an error for null entry at index 0.
    pub fn symbol(&self, index: SymbolIndex) -> read::Result<&'data Elf::Sym> {
        if index == SymbolIndex(0) {
            return Err(read::Error("Invalid ELF symbol index"));
        }
        self.symbols
            .get(index.0)
            .read_error("Invalid ELF symbol index")
    }

    /// Return the extended section index for the given symbol if present.
    #[inline]
    pub fn shndx(&self, endian: Elf::Endian, index: SymbolIndex) -> Option<u32> {
        self.shndx.get(index.0).map(|x| x.get(endian))
    }

    /// Return the section index for the given symbol.
    ///
    /// This uses the extended section index if present.
    pub fn symbol_section(
        &self,
        endian: Elf::Endian,
        symbol: &Elf::Sym,
        index: SymbolIndex,
    ) -> read::Result<Option<SectionIndex>> {
        match symbol.st_shndx(endian) {
            elf::SHN_UNDEF => Ok(None),
            elf::SHN_XINDEX => {
                let shndx = self
                    .shndx(endian, index)
                    .read_error("Missing ELF symbol extended index")?;
                if shndx == 0 {
                    Ok(None)
                } else {
                    Ok(Some(SectionIndex(shndx as usize)))
                }
            }
            shndx if shndx < elf::SHN_LORESERVE => Ok(Some(SectionIndex(shndx.into()))),
            _ => Ok(None),
        }
    }

    /// Return the symbol name for the given symbol.
    pub fn symbol_name(&self, endian: Elf::Endian, symbol: &Elf::Sym) -> read::Result<&'data [u8]> {
        symbol.name(endian, self.strings)
    }

    /// Construct a map from addresses to a user-defined map entry.
    pub fn map<Entry: SymbolMapEntry, F: Fn(&'data Elf::Sym) -> Option<Entry>>(
        &self,
        endian: Elf::Endian,
        f: F,
    ) -> SymbolMap<Entry> {
        let mut symbols = Vec::with_capacity(self.symbols.len());
        for symbol in self.symbols {
            if !symbol.is_definition(endian) {
                continue;
            }
            if let Some(entry) = f(symbol) {
                symbols.push(entry);
            }
        }
        SymbolMap::new(symbols)
    }
}

/// A symbol table in an [`ElfFile32`](super::ElfFile32).
pub type ElfSymbolTable32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSymbolTable<'data, 'file, elf::FileHeader32<Endian>, R>;
/// A symbol table in an [`ElfFile32`](super::ElfFile32).
pub type ElfSymbolTable64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSymbolTable<'data, 'file, elf::FileHeader64<Endian>, R>;

/// A symbol table in an [`ElfFile`](super::ElfFile).
#[derive(Debug, Clone, Copy)]
pub struct ElfSymbolTable<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) endian: Elf::Endian,
    pub(super) symbols: &'file SymbolTable<'data, Elf, R>,
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> read::private::Sealed
    for ElfSymbolTable<'data, 'file, Elf, R>
{
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> ObjectSymbolTable<'data>
    for ElfSymbolTable<'data, 'file, Elf, R>
{
    type Symbol = ElfSymbol<'data, 'file, Elf, R>;
    type SymbolIterator = ElfSymbolIterator<'data, 'file, Elf, R>;

    fn symbols(&self) -> Self::SymbolIterator {
        ElfSymbolIterator::new(self.endian, self.symbols)
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> read::Result<Self::Symbol> {
        let symbol = self.symbols.symbol(index)?;
        Ok(ElfSymbol {
            endian: self.endian,
            symbols: self.symbols,
            index,
            symbol,
        })
    }
}

/// An iterator for the symbols in an [`ElfFile32`](super::ElfFile32).
pub type ElfSymbolIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSymbolIterator<'data, 'file, elf::FileHeader32<Endian>, R>;
/// An iterator for the symbols in an [`ElfFile64`](super::ElfFile64).
pub type ElfSymbolIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSymbolIterator<'data, 'file, elf::FileHeader64<Endian>, R>;

/// An iterator for the symbols in an [`ElfFile`](super::ElfFile).
pub struct ElfSymbolIterator<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    endian: Elf::Endian,
    symbols: &'file SymbolTable<'data, Elf, R>,
    index: SymbolIndex,
}

impl<'data, 'file, Elf, R> ElfSymbolIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) fn new(endian: Elf::Endian, symbols: &'file SymbolTable<'data, Elf, R>) -> Self {
        ElfSymbolIterator {
            endian,
            symbols,
            index: SymbolIndex(1),
        }
    }
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> fmt::Debug
    for ElfSymbolIterator<'data, 'file, Elf, R>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElfSymbolIterator").finish()
    }
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> Iterator
    for ElfSymbolIterator<'data, 'file, Elf, R>
{
    type Item = ElfSymbol<'data, 'file, Elf, R>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let symbol = self.symbols.symbols.get(index.0)?;
        self.index.0 += 1;
        Some(ElfSymbol {
            endian: self.endian,
            symbols: self.symbols,
            index,
            symbol,
        })
    }
}

/// A symbol in an [`ElfFile32`](super::ElfFile32).
pub type ElfSymbol32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSymbol<'data, 'file, elf::FileHeader32<Endian>, R>;
/// A symbol in an [`ElfFile64`](super::ElfFile64).
pub type ElfSymbol64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSymbol<'data, 'file, elf::FileHeader64<Endian>, R>;

/// A symbol in an [`ElfFile`](super::ElfFile).
///
/// Most functionality is provided by the [`ObjectSymbol`] trait implementation.
#[derive(Debug, Clone, Copy)]
pub struct ElfSymbol<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) endian: Elf::Endian,
    pub(super) symbols: &'file SymbolTable<'data, Elf, R>,
    pub(super) index: SymbolIndex,
    pub(super) symbol: &'data Elf::Sym,
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> ElfSymbol<'data, 'file, Elf, R> {
    /// Get the endianness of the ELF file.
    pub fn endian(&self) -> Elf::Endian {
        self.endian
    }

    /// Return a reference to the raw symbol structure.
    #[inline]
    #[deprecated(note = "Use `elf_symbol` instead")]
    pub fn raw_symbol(&self) -> &'data Elf::Sym {
        self.symbol
    }

    /// Get the raw ELF symbol structure.
    pub fn elf_symbol(&self) -> &'data Elf::Sym {
        self.symbol
    }
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> read::private::Sealed
    for ElfSymbol<'data, 'file, Elf, R>
{
}

impl<'data, 'file, Elf: FileHeader, R: ReadRef<'data>> ObjectSymbol<'data>
    for ElfSymbol<'data, 'file, Elf, R>
{
    #[inline]
    fn index(&self) -> SymbolIndex {
        self.index
    }

    fn name_bytes(&self) -> read::Result<&'data [u8]> {
        self.symbol.name(self.endian, self.symbols.strings())
    }

    fn name(&self) -> read::Result<&'data str> {
        let name = self.name_bytes()?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 ELF symbol name")
    }

    #[inline]
    fn address(&self) -> u64 {
        self.symbol.st_value(self.endian).into()
    }

    #[inline]
    fn size(&self) -> u64 {
        self.symbol.st_size(self.endian).into()
    }

    fn kind(&self) -> SymbolKind {
        match self.symbol.st_type() {
            elf::STT_NOTYPE => SymbolKind::Unknown,
            elf::STT_OBJECT | elf::STT_COMMON => SymbolKind::Data,
            elf::STT_FUNC | elf::STT_GNU_IFUNC => SymbolKind::Text,
            elf::STT_SECTION => SymbolKind::Section,
            elf::STT_FILE => SymbolKind::File,
            elf::STT_TLS => SymbolKind::Tls,
            _ => SymbolKind::Unknown,
        }
    }

    fn section(&self) -> SymbolSection {
        match self.symbol.st_shndx(self.endian) {
            elf::SHN_UNDEF => SymbolSection::Undefined,
            elf::SHN_ABS => {
                if self.symbol.st_type() == elf::STT_FILE {
                    SymbolSection::None
                } else {
                    SymbolSection::Absolute
                }
            }
            elf::SHN_COMMON => SymbolSection::Common,
            elf::SHN_XINDEX => match self.symbols.shndx(self.endian, self.index) {
                Some(0) => SymbolSection::None,
                Some(index) => SymbolSection::Section(SectionIndex(index as usize)),
                None => SymbolSection::Unknown,
            },
            index if index < elf::SHN_LORESERVE => {
                SymbolSection::Section(SectionIndex(index as usize))
            }
            _ => SymbolSection::Unknown,
        }
    }

    #[inline]
    fn is_undefined(&self) -> bool {
        self.symbol.is_undefined(self.endian)
    }

    #[inline]
    fn is_definition(&self) -> bool {
        self.symbol.is_definition(self.endian)
    }

    #[inline]
    fn is_common(&self) -> bool {
        self.symbol.is_common(self.endian)
    }

    #[inline]
    fn is_weak(&self) -> bool {
        self.symbol.is_weak()
    }

    fn scope(&self) -> SymbolScope {
        if self.symbol.st_shndx(self.endian) == elf::SHN_UNDEF {
            SymbolScope::Unknown
        } else {
            match self.symbol.st_bind() {
                elf::STB_LOCAL => SymbolScope::Compilation,
                elf::STB_GLOBAL | elf::STB_WEAK => {
                    if self.symbol.st_visibility() == elf::STV_HIDDEN {
                        SymbolScope::Linkage
                    } else {
                        SymbolScope::Dynamic
                    }
                }
                _ => SymbolScope::Unknown,
            }
        }
    }

    #[inline]
    fn is_global(&self) -> bool {
        !self.symbol.is_local()
    }

    #[inline]
    fn is_local(&self) -> bool {
        self.symbol.is_local()
    }

    #[inline]
    fn flags(&self) -> SymbolFlags<SectionIndex, SymbolIndex> {
        SymbolFlags::Elf {
            st_info: self.symbol.st_info(),
            st_other: self.symbol.st_other(),
        }
    }
}

/// A trait for generic access to [`elf::Sym32`] and [`elf::Sym64`].
#[allow(missing_docs)]
pub trait Sym: Debug + Pod {
    type Word: Into<u64>;
    type Endian: endian::Endian;

    fn st_name(&self, endian: Self::Endian) -> u32;
    fn st_info(&self) -> u8;
    fn st_bind(&self) -> u8;
    fn st_type(&self) -> u8;
    fn st_other(&self) -> u8;
    fn st_visibility(&self) -> u8;
    fn st_shndx(&self, endian: Self::Endian) -> u16;
    fn st_value(&self, endian: Self::Endian) -> Self::Word;
    fn st_size(&self, endian: Self::Endian) -> Self::Word;

    /// Parse the symbol name from the string table.
    fn name<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        strings: StringTable<'data, R>,
    ) -> read::Result<&'data [u8]> {
        strings
            .get(self.st_name(endian))
            .read_error("Invalid ELF symbol name offset")
    }

    /// Return true if the symbol section is `SHN_UNDEF`.
    #[inline]
    fn is_undefined(&self, endian: Self::Endian) -> bool {
        self.st_shndx(endian) == elf::SHN_UNDEF
    }

    /// Return true if the symbol is a definition of a function or data object.
    fn is_definition(&self, endian: Self::Endian) -> bool {
        let shndx = self.st_shndx(endian);
        if shndx == elf::SHN_UNDEF || (shndx >= elf::SHN_LORESERVE && shndx != elf::SHN_XINDEX) {
            return false;
        }
        match self.st_type() {
            elf::STT_NOTYPE => self.st_size(endian).into() != 0,
            elf::STT_FUNC | elf::STT_OBJECT => true,
            _ => false,
        }
    }

    /// Return true if the symbol section is `SHN_COMMON`.
    fn is_common(&self, endian: Self::Endian) -> bool {
        self.st_shndx(endian) == elf::SHN_COMMON
    }

    /// Return true if the symbol section is `SHN_ABS`.
    fn is_absolute(&self, endian: Self::Endian) -> bool {
        self.st_shndx(endian) == elf::SHN_ABS
    }

    /// Return true if the symbol binding is `STB_LOCAL`.
    fn is_local(&self) -> bool {
        self.st_bind() == elf::STB_LOCAL
    }

    /// Return true if the symbol binding is `STB_WEAK`.
    fn is_weak(&self) -> bool {
        self.st_bind() == elf::STB_WEAK
    }
}

impl<Endian: endian::Endian> Sym for elf::Sym32<Endian> {
    type Word = u32;
    type Endian = Endian;

    #[inline]
    fn st_name(&self, endian: Self::Endian) -> u32 {
        self.st_name.get(endian)
    }

    #[inline]
    fn st_info(&self) -> u8 {
        self.st_info
    }

    #[inline]
    fn st_bind(&self) -> u8 {
        self.st_bind()
    }

    #[inline]
    fn st_type(&self) -> u8 {
        self.st_type()
    }

    #[inline]
    fn st_other(&self) -> u8 {
        self.st_other
    }

    #[inline]
    fn st_visibility(&self) -> u8 {
        self.st_visibility()
    }

    #[inline]
    fn st_shndx(&self, endian: Self::Endian) -> u16 {
        self.st_shndx.get(endian)
    }

    #[inline]
    fn st_value(&self, endian: Self::Endian) -> Self::Word {
        self.st_value.get(endian)
    }

    #[inline]
    fn st_size(&self, endian: Self::Endian) -> Self::Word {
        self.st_size.get(endian)
    }
}

impl<Endian: endian::Endian> Sym for elf::Sym64<Endian> {
    type Word = u64;
    type Endian = Endian;

    #[inline]
    fn st_name(&self, endian: Self::Endian) -> u32 {
        self.st_name.get(endian)
    }

    #[inline]
    fn st_info(&self) -> u8 {
        self.st_info
    }

    #[inline]
    fn st_bind(&self) -> u8 {
        self.st_bind()
    }

    #[inline]
    fn st_type(&self) -> u8 {
        self.st_type()
    }

    #[inline]
    fn st_other(&self) -> u8 {
        self.st_other
    }

    #[inline]
    fn st_visibility(&self) -> u8 {
        self.st_visibility()
    }

    #[inline]
    fn st_shndx(&self, endian: Self::Endian) -> u16 {
        self.st_shndx.get(endian)
    }

    #[inline]
    fn st_value(&self, endian: Self::Endian) -> Self::Word {
        self.st_value.get(endian)
    }

    #[inline]
    fn st_size(&self, endian: Self::Endian) -> Self::Word {
        self.st_size.get(endian)
    }
}
