use alloc::fmt;
use alloc::vec::Vec;
use core::convert::TryInto;
use core::fmt::Debug;
use core::str;

use super::{CoffCommon, CoffHeader, SectionTable};
use crate::endian::{LittleEndian as LE, U32Bytes};
use crate::pe;
use crate::pod::{bytes_of, bytes_of_slice, Pod};
use crate::read::util::StringTable;
use crate::read::{
    self, Bytes, ObjectSymbol, ObjectSymbolTable, ReadError, ReadRef, Result, SectionIndex,
    SymbolFlags, SymbolIndex, SymbolKind, SymbolMap, SymbolMapEntry, SymbolScope, SymbolSection,
};

/// A table of symbol entries in a COFF or PE file.
///
/// Also includes the string table used for the symbol names.
///
/// Returned by [`CoffHeader::symbols`] and
/// [`ImageNtHeaders::symbols`](crate::read::pe::ImageNtHeaders::symbols).
#[derive(Debug)]
pub struct SymbolTable<'data, R = &'data [u8], Coff = pe::ImageFileHeader>
where
    R: ReadRef<'data>,
    Coff: CoffHeader,
{
    symbols: &'data [Coff::ImageSymbolBytes],
    strings: StringTable<'data, R>,
}

impl<'data, R: ReadRef<'data>, Coff: CoffHeader> Default for SymbolTable<'data, R, Coff> {
    fn default() -> Self {
        Self {
            symbols: &[],
            strings: StringTable::default(),
        }
    }
}

impl<'data, R: ReadRef<'data>, Coff: CoffHeader> SymbolTable<'data, R, Coff> {
    /// Read the symbol table.
    pub fn parse(header: &Coff, data: R) -> Result<Self> {
        // The symbol table may not be present.
        let mut offset = header.pointer_to_symbol_table().into();
        let (symbols, strings) = if offset != 0 {
            let symbols = data
                .read_slice(&mut offset, header.number_of_symbols() as usize)
                .read_error("Invalid COFF symbol table offset or size")?;

            // Note: don't update data when reading length; the length includes itself.
            let length = data
                .read_at::<U32Bytes<_>>(offset)
                .read_error("Missing COFF string table")?
                .get(LE);
            let str_end = offset
                .checked_add(length as u64)
                .read_error("Invalid COFF string table length")?;
            let strings = StringTable::new(data, offset, str_end);

            (symbols, strings)
        } else {
            (&[][..], StringTable::default())
        };

        Ok(SymbolTable { symbols, strings })
    }

    /// Return the string table used for the symbol names.
    #[inline]
    pub fn strings(&self) -> StringTable<'data, R> {
        self.strings
    }

    /// Return true if the symbol table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// The number of symbol table entries.
    ///
    /// This includes auxiliary symbol table entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Iterate over the symbols.
    #[inline]
    pub fn iter<'table>(&'table self) -> SymbolIterator<'data, 'table, R, Coff> {
        SymbolIterator {
            symbols: self,
            index: SymbolIndex(0),
        }
    }

    /// Return the symbol table entry at the given index.
    #[inline]
    pub fn symbol(&self, index: SymbolIndex) -> Result<&'data Coff::ImageSymbol> {
        self.get::<Coff::ImageSymbol>(index, 0)
    }

    /// Return the auxiliary function symbol for the symbol table entry at the given index.
    ///
    /// Note that the index is of the symbol, not the first auxiliary record.
    #[inline]
    pub fn aux_function(&self, index: SymbolIndex) -> Result<&'data pe::ImageAuxSymbolFunction> {
        self.get::<pe::ImageAuxSymbolFunction>(index, 1)
    }

    /// Return the auxiliary section symbol for the symbol table entry at the given index.
    ///
    /// Note that the index is of the symbol, not the first auxiliary record.
    #[inline]
    pub fn aux_section(&self, index: SymbolIndex) -> Result<&'data pe::ImageAuxSymbolSection> {
        self.get::<pe::ImageAuxSymbolSection>(index, 1)
    }

    /// Return the auxiliary file name for the symbol table entry at the given index.
    ///
    /// Note that the index is of the symbol, not the first auxiliary record.
    pub fn aux_file_name(&self, index: SymbolIndex, aux_count: u8) -> Result<&'data [u8]> {
        let entries = index
            .0
            .checked_add(1)
            .and_then(|x| Some(x..x.checked_add(aux_count.into())?))
            .and_then(|x| self.symbols.get(x))
            .read_error("Invalid COFF symbol index")?;
        let bytes = bytes_of_slice(entries);
        // The name is padded with nulls.
        Ok(match memchr::memchr(b'\0', bytes) {
            Some(end) => &bytes[..end],
            None => bytes,
        })
    }

    /// Return the symbol table entry or auxiliary record at the given index and offset.
    pub fn get<T: Pod>(&self, index: SymbolIndex, offset: usize) -> Result<&'data T> {
        let bytes = index
            .0
            .checked_add(offset)
            .and_then(|x| self.symbols.get(x))
            .read_error("Invalid COFF symbol index")?;
        Bytes(bytes_of(bytes))
            .read()
            .read_error("Invalid COFF symbol data")
    }

    /// Construct a map from addresses to a user-defined map entry.
    pub fn map<Entry: SymbolMapEntry, F: Fn(&'data Coff::ImageSymbol) -> Option<Entry>>(
        &self,
        f: F,
    ) -> SymbolMap<Entry> {
        let mut symbols = Vec::with_capacity(self.symbols.len());
        for (_, symbol) in self.iter() {
            if !symbol.is_definition() {
                continue;
            }
            if let Some(entry) = f(symbol) {
                symbols.push(entry);
            }
        }
        SymbolMap::new(symbols)
    }
}

/// An iterator for symbol entries in a COFF or PE file.
///
/// Yields the index and symbol structure for each symbol.
#[derive(Debug)]
pub struct SymbolIterator<'data, 'table, R = &'data [u8], Coff = pe::ImageFileHeader>
where
    R: ReadRef<'data>,
    Coff: CoffHeader,
{
    symbols: &'table SymbolTable<'data, R, Coff>,
    index: SymbolIndex,
}

impl<'data, 'table, R: ReadRef<'data>, Coff: CoffHeader> Iterator
    for SymbolIterator<'data, 'table, R, Coff>
{
    type Item = (SymbolIndex, &'data Coff::ImageSymbol);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let symbol = self.symbols.symbol(index).ok()?;
        self.index.0 += 1 + symbol.number_of_aux_symbols() as usize;
        Some((index, symbol))
    }
}

/// A symbol table in a [`CoffBigFile`](super::CoffBigFile).
pub type CoffBigSymbolTable<'data, 'file, R = &'data [u8]> =
    CoffSymbolTable<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// A symbol table in a [`CoffFile`](super::CoffFile)
/// or [`PeFile`](crate::read::pe::PeFile).
#[derive(Debug, Clone, Copy)]
pub struct CoffSymbolTable<'data, 'file, R = &'data [u8], Coff = pe::ImageFileHeader>
where
    R: ReadRef<'data>,
    Coff: CoffHeader,
{
    pub(crate) file: &'file CoffCommon<'data, R, Coff>,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> read::private::Sealed
    for CoffSymbolTable<'data, 'file, R, Coff>
{
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> ObjectSymbolTable<'data>
    for CoffSymbolTable<'data, 'file, R, Coff>
{
    type Symbol = CoffSymbol<'data, 'file, R, Coff>;
    type SymbolIterator = CoffSymbolIterator<'data, 'file, R, Coff>;

    fn symbols(&self) -> Self::SymbolIterator {
        CoffSymbolIterator::new(self.file)
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Self::Symbol> {
        let symbol = self.file.symbols.symbol(index)?;
        Ok(CoffSymbol {
            file: self.file,
            index,
            symbol,
        })
    }
}

/// An iterator for the symbols in a [`CoffBigFile`](super::CoffBigFile).
pub type CoffBigSymbolIterator<'data, 'file, R = &'data [u8]> =
    CoffSymbolIterator<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// An iterator for the symbols in a [`CoffFile`](super::CoffFile)
/// or [`PeFile`](crate::read::pe::PeFile).
pub struct CoffSymbolIterator<'data, 'file, R = &'data [u8], Coff = pe::ImageFileHeader>
where
    R: ReadRef<'data>,
    Coff: CoffHeader,
{
    file: &'file CoffCommon<'data, R, Coff>,
    index: SymbolIndex,
}

impl<'data, 'file, R, Coff> CoffSymbolIterator<'data, 'file, R, Coff>
where
    R: ReadRef<'data>,
    Coff: CoffHeader,
{
    pub(crate) fn new(file: &'file CoffCommon<'data, R, Coff>) -> Self {
        Self {
            file,
            index: SymbolIndex(0),
        }
    }

    pub(crate) fn empty(file: &'file CoffCommon<'data, R, Coff>) -> Self {
        Self {
            file,
            index: SymbolIndex(file.symbols.len()),
        }
    }
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> fmt::Debug
    for CoffSymbolIterator<'data, 'file, R, Coff>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoffSymbolIterator").finish()
    }
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> Iterator
    for CoffSymbolIterator<'data, 'file, R, Coff>
{
    type Item = CoffSymbol<'data, 'file, R, Coff>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let symbol = self.file.symbols.symbol(index).ok()?;
        self.index.0 += 1 + symbol.number_of_aux_symbols() as usize;
        Some(CoffSymbol {
            file: self.file,
            index,
            symbol,
        })
    }
}

/// A symbol in a [`CoffBigFile`](super::CoffBigFile).
///
/// Most functionality is provided by the [`ObjectSymbol`] trait implementation.
pub type CoffBigSymbol<'data, 'file, R = &'data [u8]> =
    CoffSymbol<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// A symbol in a [`CoffFile`](super::CoffFile) or [`PeFile`](crate::read::pe::PeFile).
///
/// Most functionality is provided by the [`ObjectSymbol`] trait implementation.
#[derive(Debug, Clone, Copy)]
pub struct CoffSymbol<'data, 'file, R = &'data [u8], Coff = pe::ImageFileHeader>
where
    R: ReadRef<'data>,
    Coff: CoffHeader,
{
    pub(crate) file: &'file CoffCommon<'data, R, Coff>,
    pub(crate) index: SymbolIndex,
    pub(crate) symbol: &'data Coff::ImageSymbol,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> CoffSymbol<'data, 'file, R, Coff> {
    #[inline]
    /// Get the raw `ImageSymbol` struct.
    #[deprecated(note = "Use `coff_symbol` instead")]
    pub fn raw_symbol(&self) -> &'data Coff::ImageSymbol {
        self.symbol
    }

    /// Get the raw `ImageSymbol` struct.
    pub fn coff_symbol(&self) -> &'data Coff::ImageSymbol {
        self.symbol
    }
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> read::private::Sealed
    for CoffSymbol<'data, 'file, R, Coff>
{
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> ObjectSymbol<'data>
    for CoffSymbol<'data, 'file, R, Coff>
{
    #[inline]
    fn index(&self) -> SymbolIndex {
        self.index
    }

    fn name_bytes(&self) -> read::Result<&'data [u8]> {
        if self.symbol.has_aux_file_name() {
            self.file
                .symbols
                .aux_file_name(self.index, self.symbol.number_of_aux_symbols())
        } else {
            self.symbol.name(self.file.symbols.strings())
        }
    }

    fn name(&self) -> read::Result<&'data str> {
        let name = self.name_bytes()?;
        str::from_utf8(name)
            .ok()
            .read_error("Non UTF-8 COFF symbol name")
    }

    fn address(&self) -> u64 {
        self.symbol
            .address(self.file.image_base, &self.file.sections)
            .unwrap_or(None)
            .unwrap_or(0)
    }

    fn size(&self) -> u64 {
        match self.symbol.storage_class() {
            pe::IMAGE_SYM_CLASS_STATIC => {
                // Section symbols may duplicate the size from the section table.
                if self.symbol.has_aux_section() {
                    if let Ok(aux) = self.file.symbols.aux_section(self.index) {
                        u64::from(aux.length.get(LE))
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            pe::IMAGE_SYM_CLASS_EXTERNAL => {
                if self.symbol.section_number() == pe::IMAGE_SYM_UNDEFINED {
                    // For undefined symbols, symbol.value is 0 and the size is 0.
                    // For common data, symbol.value is the size.
                    u64::from(self.symbol.value())
                } else if self.symbol.has_aux_function() {
                    // Function symbols may have a size.
                    if let Ok(aux) = self.file.symbols.aux_function(self.index) {
                        u64::from(aux.total_size.get(LE))
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            // Most symbols don't have sizes.
            _ => 0,
        }
    }

    fn kind(&self) -> SymbolKind {
        let derived_kind = if self.symbol.derived_type() == pe::IMAGE_SYM_DTYPE_FUNCTION {
            SymbolKind::Text
        } else {
            SymbolKind::Data
        };
        match self.symbol.storage_class() {
            pe::IMAGE_SYM_CLASS_STATIC => {
                if self.symbol.has_aux_section() {
                    SymbolKind::Section
                } else {
                    derived_kind
                }
            }
            pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => derived_kind,
            pe::IMAGE_SYM_CLASS_SECTION => SymbolKind::Section,
            pe::IMAGE_SYM_CLASS_FILE => SymbolKind::File,
            pe::IMAGE_SYM_CLASS_LABEL => SymbolKind::Label,
            _ => SymbolKind::Unknown,
        }
    }

    fn section(&self) -> SymbolSection {
        match self.symbol.section_number() {
            pe::IMAGE_SYM_UNDEFINED => {
                if self.symbol.storage_class() == pe::IMAGE_SYM_CLASS_EXTERNAL {
                    if self.symbol.value() == 0 {
                        SymbolSection::Undefined
                    } else {
                        SymbolSection::Common
                    }
                } else if self.symbol.storage_class() == pe::IMAGE_SYM_CLASS_SECTION {
                    SymbolSection::Undefined
                } else {
                    SymbolSection::Unknown
                }
            }
            pe::IMAGE_SYM_ABSOLUTE => SymbolSection::Absolute,
            pe::IMAGE_SYM_DEBUG => {
                if self.symbol.storage_class() == pe::IMAGE_SYM_CLASS_FILE {
                    SymbolSection::None
                } else {
                    SymbolSection::Unknown
                }
            }
            index if index > 0 => SymbolSection::Section(SectionIndex(index as usize)),
            _ => SymbolSection::Unknown,
        }
    }

    #[inline]
    fn is_undefined(&self) -> bool {
        self.symbol.storage_class() == pe::IMAGE_SYM_CLASS_EXTERNAL
            && self.symbol.section_number() == pe::IMAGE_SYM_UNDEFINED
            && self.symbol.value() == 0
    }

    #[inline]
    fn is_definition(&self) -> bool {
        self.symbol.is_definition()
    }

    #[inline]
    fn is_common(&self) -> bool {
        self.symbol.storage_class() == pe::IMAGE_SYM_CLASS_EXTERNAL
            && self.symbol.section_number() == pe::IMAGE_SYM_UNDEFINED
            && self.symbol.value() != 0
    }

    #[inline]
    fn is_weak(&self) -> bool {
        self.symbol.storage_class() == pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL
    }

    #[inline]
    fn scope(&self) -> SymbolScope {
        match self.symbol.storage_class() {
            pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => {
                // TODO: determine if symbol is exported
                SymbolScope::Linkage
            }
            _ => SymbolScope::Compilation,
        }
    }

    #[inline]
    fn is_global(&self) -> bool {
        match self.symbol.storage_class() {
            pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => true,
            _ => false,
        }
    }

    #[inline]
    fn is_local(&self) -> bool {
        !self.is_global()
    }

    fn flags(&self) -> SymbolFlags<SectionIndex, SymbolIndex> {
        if self.symbol.has_aux_section() {
            if let Ok(aux) = self.file.symbols.aux_section(self.index) {
                let number = if Coff::is_type_bigobj() {
                    u32::from(aux.number.get(LE)) | (u32::from(aux.high_number.get(LE)) << 16)
                } else {
                    u32::from(aux.number.get(LE))
                };
                return SymbolFlags::CoffSection {
                    selection: aux.selection,
                    associative_section: if number == 0 {
                        None
                    } else {
                        Some(SectionIndex(number as usize))
                    },
                };
            }
        }
        SymbolFlags::None
    }
}

/// A trait for generic access to [`pe::ImageSymbol`] and [`pe::ImageSymbolEx`].
#[allow(missing_docs)]
pub trait ImageSymbol: Debug + Pod {
    fn raw_name(&self) -> &[u8; 8];
    fn value(&self) -> u32;
    fn section_number(&self) -> i32;
    fn typ(&self) -> u16;
    fn storage_class(&self) -> u8;
    fn number_of_aux_symbols(&self) -> u8;

    /// Parse a COFF symbol name.
    ///
    /// `strings` must be the string table used for symbol names.
    fn name<'data, R: ReadRef<'data>>(
        &'data self,
        strings: StringTable<'data, R>,
    ) -> Result<&'data [u8]> {
        let name = self.raw_name();
        if name[0] == 0 {
            // If the name starts with 0 then the last 4 bytes are a string table offset.
            let offset = u32::from_le_bytes(name[4..8].try_into().unwrap());
            strings
                .get(offset)
                .read_error("Invalid COFF symbol name offset")
        } else {
            // The name is inline and padded with nulls.
            Ok(match memchr::memchr(b'\0', name) {
                Some(end) => &name[..end],
                None => &name[..],
            })
        }
    }

    /// Return the symbol address.
    ///
    /// This takes into account the image base and the section address,
    /// and only returns an address for symbols that have an address.
    fn address(&self, image_base: u64, sections: &SectionTable<'_>) -> Result<Option<u64>> {
        // Only return an address for storage classes that we know use an address.
        match self.storage_class() {
            pe::IMAGE_SYM_CLASS_STATIC
            | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL
            | pe::IMAGE_SYM_CLASS_LABEL
            | pe::IMAGE_SYM_CLASS_EXTERNAL => {}
            _ => return Ok(None),
        }
        let Some(section_index) = self.section() else {
            return Ok(None);
        };
        let section = sections.section(section_index)?;
        let virtual_address = u64::from(section.virtual_address.get(LE));
        let value = u64::from(self.value());
        Ok(Some(image_base + virtual_address + value))
    }

    /// Return the section index for the symbol.
    fn section(&self) -> Option<SectionIndex> {
        let section_number = self.section_number();
        if section_number > 0 {
            Some(SectionIndex(section_number as usize))
        } else {
            None
        }
    }

    /// Return true if the symbol is a definition of a function or data object.
    fn is_definition(&self) -> bool {
        if self.section_number() <= 0 {
            return false;
        }
        match self.storage_class() {
            pe::IMAGE_SYM_CLASS_STATIC => !self.has_aux_section(),
            pe::IMAGE_SYM_CLASS_EXTERNAL | pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL => true,
            _ => false,
        }
    }

    /// Return true if the symbol has an auxiliary file name.
    fn has_aux_file_name(&self) -> bool {
        self.number_of_aux_symbols() > 0 && self.storage_class() == pe::IMAGE_SYM_CLASS_FILE
    }

    /// Return true if the symbol has an auxiliary function symbol.
    fn has_aux_function(&self) -> bool {
        self.number_of_aux_symbols() > 0 && self.derived_type() == pe::IMAGE_SYM_DTYPE_FUNCTION
    }

    /// Return true if the symbol has an auxiliary section symbol.
    fn has_aux_section(&self) -> bool {
        self.number_of_aux_symbols() > 0
            && self.storage_class() == pe::IMAGE_SYM_CLASS_STATIC
            && self.typ() == 0
    }

    fn base_type(&self) -> u16 {
        self.typ() & pe::N_BTMASK
    }

    fn derived_type(&self) -> u16 {
        (self.typ() & pe::N_TMASK) >> pe::N_BTSHFT
    }
}

impl ImageSymbol for pe::ImageSymbol {
    fn raw_name(&self) -> &[u8; 8] {
        &self.name
    }
    fn value(&self) -> u32 {
        self.value.get(LE)
    }
    fn section_number(&self) -> i32 {
        let section_number = self.section_number.get(LE);
        if section_number >= pe::IMAGE_SYM_SECTION_MAX {
            (section_number as i16) as i32
        } else {
            section_number as i32
        }
    }
    fn typ(&self) -> u16 {
        self.typ.get(LE)
    }
    fn storage_class(&self) -> u8 {
        self.storage_class
    }
    fn number_of_aux_symbols(&self) -> u8 {
        self.number_of_aux_symbols
    }
}

impl ImageSymbol for pe::ImageSymbolEx {
    fn raw_name(&self) -> &[u8; 8] {
        &self.name
    }
    fn value(&self) -> u32 {
        self.value.get(LE)
    }
    fn section_number(&self) -> i32 {
        self.section_number.get(LE)
    }
    fn typ(&self) -> u16 {
        self.typ.get(LE)
    }
    fn storage_class(&self) -> u8 {
        self.storage_class
    }
    fn number_of_aux_symbols(&self) -> u8 {
        self.number_of_aux_symbols
    }
}
