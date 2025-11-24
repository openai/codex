use core::str;

use crate::endian::LittleEndian as LE;
use crate::pe;
use crate::read::{
    self, ComdatKind, ObjectComdat, ReadError, ReadRef, Result, SectionIndex, SymbolIndex,
};

use super::{CoffFile, CoffHeader, ImageSymbol};

/// An iterator for the COMDAT section groups in a [`CoffBigFile`](super::CoffBigFile).
pub type CoffBigComdatIterator<'data, 'file, R = &'data [u8]> =
    CoffComdatIterator<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// An iterator for the COMDAT section groups in a [`CoffFile`].
#[derive(Debug)]
pub struct CoffComdatIterator<
    'data,
    'file,
    R: ReadRef<'data> = &'data [u8],
    Coff: CoffHeader = pe::ImageFileHeader,
> {
    file: &'file CoffFile<'data, R, Coff>,
    index: SymbolIndex,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> CoffComdatIterator<'data, 'file, R, Coff> {
    pub(crate) fn new(file: &'file CoffFile<'data, R, Coff>) -> Self {
        CoffComdatIterator {
            file,
            index: SymbolIndex(0),
        }
    }
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> Iterator
    for CoffComdatIterator<'data, 'file, R, Coff>
{
    type Item = CoffComdat<'data, 'file, R, Coff>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let index = self.index;
            let symbol = self.file.common.symbols.symbol(index).ok()?;
            self.index.0 += 1 + symbol.number_of_aux_symbols() as usize;
            if let Some(comdat) = CoffComdat::parse(self.file, symbol, index) {
                return Some(comdat);
            }
        }
    }
}

/// A COMDAT section group in a [`CoffBigFile`](super::CoffBigFile).
///
/// Most functionality is provided by the [`ObjectComdat`] trait implementation.
pub type CoffBigComdat<'data, 'file, R = &'data [u8]> =
    CoffComdat<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// A COMDAT section group in a [`CoffFile`].
///
/// Most functionality is provided by the [`ObjectComdat`] trait implementation.
#[derive(Debug)]
pub struct CoffComdat<
    'data,
    'file,
    R: ReadRef<'data> = &'data [u8],
    Coff: CoffHeader = pe::ImageFileHeader,
> {
    file: &'file CoffFile<'data, R, Coff>,
    symbol_index: SymbolIndex,
    symbol: &'data Coff::ImageSymbol,
    selection: u8,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> CoffComdat<'data, 'file, R, Coff> {
    fn parse(
        file: &'file CoffFile<'data, R, Coff>,
        section_symbol: &'data Coff::ImageSymbol,
        index: SymbolIndex,
    ) -> Option<CoffComdat<'data, 'file, R, Coff>> {
        // Must be a section symbol.
        if !section_symbol.has_aux_section() {
            return None;
        }

        // Auxiliary record must have a non-associative selection.
        let aux = file.common.symbols.aux_section(index).ok()?;
        let selection = aux.selection;
        if selection == 0 || selection == pe::IMAGE_COMDAT_SELECT_ASSOCIATIVE {
            return None;
        }

        // Find the COMDAT symbol.
        let mut symbol_index = index;
        let mut symbol = section_symbol;
        let section_number = section_symbol.section_number();
        loop {
            symbol_index.0 += 1 + symbol.number_of_aux_symbols() as usize;
            symbol = file.common.symbols.symbol(symbol_index).ok()?;
            if section_number == symbol.section_number() {
                break;
            }
        }

        Some(CoffComdat {
            file,
            symbol_index,
            symbol,
            selection,
        })
    }
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> read::private::Sealed
    for CoffComdat<'data, 'file, R, Coff>
{
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> ObjectComdat<'data>
    for CoffComdat<'data, 'file, R, Coff>
{
    type SectionIterator = CoffComdatSectionIterator<'data, 'file, R, Coff>;

    #[inline]
    fn kind(&self) -> ComdatKind {
        match self.selection {
            pe::IMAGE_COMDAT_SELECT_NODUPLICATES => ComdatKind::NoDuplicates,
            pe::IMAGE_COMDAT_SELECT_ANY => ComdatKind::Any,
            pe::IMAGE_COMDAT_SELECT_SAME_SIZE => ComdatKind::SameSize,
            pe::IMAGE_COMDAT_SELECT_EXACT_MATCH => ComdatKind::ExactMatch,
            pe::IMAGE_COMDAT_SELECT_LARGEST => ComdatKind::Largest,
            pe::IMAGE_COMDAT_SELECT_NEWEST => ComdatKind::Newest,
            _ => ComdatKind::Unknown,
        }
    }

    #[inline]
    fn symbol(&self) -> SymbolIndex {
        self.symbol_index
    }

    #[inline]
    fn name_bytes(&self) -> Result<&'data [u8]> {
        // Find the name of first symbol referring to the section.
        self.symbol.name(self.file.common.symbols.strings())
    }

    #[inline]
    fn name(&self) -> Result<&'data str> {
        let bytes = self.name_bytes()?;
        str::from_utf8(bytes)
            .ok()
            .read_error("Non UTF-8 COFF COMDAT name")
    }

    #[inline]
    fn sections(&self) -> Self::SectionIterator {
        CoffComdatSectionIterator {
            file: self.file,
            section_number: self.symbol.section_number(),
            index: SymbolIndex(0),
        }
    }
}

/// An iterator for the sections in a COMDAT section group in a [`CoffBigFile`](super::CoffBigFile).
pub type CoffBigComdatSectionIterator<'data, 'file, R = &'data [u8]> =
    CoffComdatSectionIterator<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// An iterator for the sections in a COMDAT section group in a [`CoffFile`].
#[derive(Debug)]
pub struct CoffComdatSectionIterator<
    'data,
    'file,
    R: ReadRef<'data> = &'data [u8],
    Coff: CoffHeader = pe::ImageFileHeader,
> {
    file: &'file CoffFile<'data, R, Coff>,
    section_number: i32,
    index: SymbolIndex,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> Iterator
    for CoffComdatSectionIterator<'data, 'file, R, Coff>
{
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        // Find associated COMDAT symbols.
        // TODO: it seems gcc doesn't use associated symbols for this
        loop {
            let index = self.index;
            let symbol = self.file.common.symbols.symbol(index).ok()?;
            self.index.0 += 1 + symbol.number_of_aux_symbols() as usize;

            // Must be a section symbol.
            if !symbol.has_aux_section() {
                continue;
            }

            let section_number = symbol.section_number();

            let aux = self.file.common.symbols.aux_section(index).ok()?;
            if aux.selection == pe::IMAGE_COMDAT_SELECT_ASSOCIATIVE {
                let number = if Coff::is_type_bigobj() {
                    u32::from(aux.number.get(LE)) | (u32::from(aux.high_number.get(LE)) << 16)
                } else {
                    u32::from(aux.number.get(LE))
                };
                if number as i32 == self.section_number {
                    return Some(SectionIndex(section_number as usize));
                }
            } else if aux.selection != 0 {
                if section_number == self.section_number {
                    return Some(SectionIndex(section_number as usize));
                }
            }
        }
    }
}
