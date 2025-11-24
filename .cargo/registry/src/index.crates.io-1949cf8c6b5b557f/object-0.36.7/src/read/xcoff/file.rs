use core::fmt::Debug;
use core::mem;

use alloc::vec::Vec;

use crate::endian::BigEndian as BE;
use crate::pod::Pod;
use crate::read::{
    self, Architecture, Error, Export, FileFlags, Import, NoDynamicRelocationIterator, Object,
    ObjectKind, ObjectSection, ReadError, ReadRef, Result, SectionIndex, SymbolIndex,
};
use crate::xcoff;

use super::{
    CsectAux, FileAux, Rel, SectionHeader, SectionTable, Symbol, SymbolTable, XcoffComdat,
    XcoffComdatIterator, XcoffSection, XcoffSectionIterator, XcoffSegment, XcoffSegmentIterator,
    XcoffSymbol, XcoffSymbolIterator, XcoffSymbolTable,
};

/// A 32-bit XCOFF object file.
///
/// This is a file that starts with [`xcoff::FileHeader32`], and corresponds
/// to [`crate::FileKind::Xcoff32`].
pub type XcoffFile32<'data, R = &'data [u8]> = XcoffFile<'data, xcoff::FileHeader32, R>;
/// A 64-bit XCOFF object file.
///
/// This is a file that starts with [`xcoff::FileHeader64`], and corresponds
/// to [`crate::FileKind::Xcoff64`].
pub type XcoffFile64<'data, R = &'data [u8]> = XcoffFile<'data, xcoff::FileHeader64, R>;

/// A partially parsed XCOFF file.
///
/// Most functionality is provided by the [`Object`] trait implementation.
#[derive(Debug)]
pub struct XcoffFile<'data, Xcoff, R = &'data [u8]>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    pub(super) data: R,
    pub(super) header: &'data Xcoff,
    pub(super) aux_header: Option<&'data Xcoff::AuxHeader>,
    pub(super) sections: SectionTable<'data, Xcoff>,
    pub(super) symbols: SymbolTable<'data, Xcoff, R>,
}

impl<'data, Xcoff, R> XcoffFile<'data, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    /// Parse the raw XCOFF file data.
    pub fn parse(data: R) -> Result<Self> {
        let mut offset = 0;
        let header = Xcoff::parse(data, &mut offset)?;
        let aux_header = header.aux_header(data, &mut offset)?;
        let sections = header.sections(data, &mut offset)?;
        let symbols = header.symbols(data)?;

        Ok(XcoffFile {
            data,
            header,
            aux_header,
            sections,
            symbols,
        })
    }

    /// Returns the raw data.
    pub fn data(&self) -> R {
        self.data
    }

    /// Returns the raw XCOFF file header.
    #[deprecated(note = "Use `xcoff_header` instead")]
    pub fn raw_header(&self) -> &'data Xcoff {
        self.header
    }

    /// Get the raw XCOFF file header.
    pub fn xcoff_header(&self) -> &'data Xcoff {
        self.header
    }

    /// Get the raw XCOFF auxiliary header.
    pub fn xcoff_aux_header(&self) -> Option<&'data Xcoff::AuxHeader> {
        self.aux_header
    }

    /// Get the XCOFF section table.
    pub fn xcoff_section_table(&self) -> &SectionTable<'data, Xcoff> {
        &self.sections
    }

    /// Get the XCOFF symbol table.
    pub fn xcoff_symbol_table(&self) -> &SymbolTable<'data, Xcoff, R> {
        &self.symbols
    }
}

impl<'data, Xcoff, R> read::private::Sealed for XcoffFile<'data, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
}

impl<'data, Xcoff, R> Object<'data> for XcoffFile<'data, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    type Segment<'file>
        = XcoffSegment<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type SegmentIterator<'file>
        = XcoffSegmentIterator<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type Section<'file>
        = XcoffSection<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type SectionIterator<'file>
        = XcoffSectionIterator<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type Comdat<'file>
        = XcoffComdat<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type ComdatIterator<'file>
        = XcoffComdatIterator<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type Symbol<'file>
        = XcoffSymbol<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type SymbolIterator<'file>
        = XcoffSymbolIterator<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type SymbolTable<'file>
        = XcoffSymbolTable<'data, 'file, Xcoff, R>
    where
        Self: 'file,
        'data: 'file;
    type DynamicRelocationIterator<'file>
        = NoDynamicRelocationIterator
    where
        Self: 'file,
        'data: 'file;

    fn architecture(&self) -> Architecture {
        if self.is_64() {
            Architecture::PowerPc64
        } else {
            Architecture::PowerPc
        }
    }

    fn is_little_endian(&self) -> bool {
        false
    }

    fn is_64(&self) -> bool {
        self.header.is_type_64()
    }

    fn kind(&self) -> ObjectKind {
        let flags = self.header.f_flags();
        if flags & xcoff::F_EXEC != 0 {
            ObjectKind::Executable
        } else if flags & xcoff::F_SHROBJ != 0 {
            ObjectKind::Dynamic
        } else if flags & xcoff::F_RELFLG == 0 {
            ObjectKind::Relocatable
        } else {
            ObjectKind::Unknown
        }
    }

    fn segments(&self) -> XcoffSegmentIterator<'data, '_, Xcoff, R> {
        XcoffSegmentIterator { file: self }
    }

    fn section_by_name_bytes<'file>(
        &'file self,
        section_name: &[u8],
    ) -> Option<XcoffSection<'data, 'file, Xcoff, R>> {
        self.sections()
            .find(|section| section.name_bytes() == Ok(section_name))
    }

    fn section_by_index(&self, index: SectionIndex) -> Result<XcoffSection<'data, '_, Xcoff, R>> {
        let section = self.sections.section(index)?;
        Ok(XcoffSection {
            file: self,
            section,
            index,
        })
    }

    fn sections(&self) -> XcoffSectionIterator<'data, '_, Xcoff, R> {
        XcoffSectionIterator {
            file: self,
            iter: self.sections.iter().enumerate(),
        }
    }

    fn comdats(&self) -> XcoffComdatIterator<'data, '_, Xcoff, R> {
        XcoffComdatIterator { file: self }
    }

    fn symbol_table(&self) -> Option<XcoffSymbolTable<'data, '_, Xcoff, R>> {
        if self.symbols.is_empty() {
            return None;
        }
        Some(XcoffSymbolTable {
            symbols: &self.symbols,
            file: self,
        })
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> Result<XcoffSymbol<'data, '_, Xcoff, R>> {
        let symbol = self.symbols.symbol(index)?;
        Ok(XcoffSymbol {
            symbols: &self.symbols,
            index,
            symbol,
            file: self,
        })
    }

    fn symbols(&self) -> XcoffSymbolIterator<'data, '_, Xcoff, R> {
        XcoffSymbolIterator {
            file: self,
            symbols: self.symbols.iter(),
        }
    }

    fn dynamic_symbol_table<'file>(
        &'file self,
    ) -> Option<XcoffSymbolTable<'data, 'file, Xcoff, R>> {
        None
    }

    fn dynamic_symbols(&self) -> XcoffSymbolIterator<'data, '_, Xcoff, R> {
        // TODO: return the symbols in the STYP_LOADER section.
        XcoffSymbolIterator {
            file: self,
            symbols: self.symbols.iter_none(),
        }
    }

    fn dynamic_relocations(&self) -> Option<Self::DynamicRelocationIterator<'_>> {
        // TODO: return the relocations in the STYP_LOADER section.
        None
    }

    fn imports(&self) -> Result<alloc::vec::Vec<Import<'data>>> {
        // TODO: return the imports in the STYP_LOADER section.
        Ok(Vec::new())
    }

    fn exports(&self) -> Result<alloc::vec::Vec<Export<'data>>> {
        // TODO: return the exports in the STYP_LOADER section.
        Ok(Vec::new())
    }

    fn has_debug_symbols(&self) -> bool {
        self.section_by_name(".debug").is_some() || self.section_by_name(".dwinfo").is_some()
    }

    fn relative_address_base(&self) -> u64 {
        0
    }

    fn entry(&self) -> u64 {
        if let Some(aux_header) = self.aux_header {
            aux_header.o_entry().into()
        } else {
            0
        }
    }

    fn flags(&self) -> FileFlags {
        FileFlags::Xcoff {
            f_flags: self.header.f_flags(),
        }
    }
}

/// A trait for generic access to [`xcoff::FileHeader32`] and [`xcoff::FileHeader64`].
#[allow(missing_docs)]
pub trait FileHeader: Debug + Pod {
    type Word: Into<u64>;
    type AuxHeader: AuxHeader<Word = Self::Word>;
    type SectionHeader: SectionHeader<Word = Self::Word, Rel = Self::Rel>;
    type Symbol: Symbol<Word = Self::Word>;
    type FileAux: FileAux;
    type CsectAux: CsectAux;
    type Rel: Rel<Word = Self::Word>;

    /// Return true if this type is a 64-bit header.
    fn is_type_64(&self) -> bool;

    fn f_magic(&self) -> u16;
    fn f_nscns(&self) -> u16;
    fn f_timdat(&self) -> u32;
    fn f_symptr(&self) -> Self::Word;
    fn f_nsyms(&self) -> u32;
    fn f_opthdr(&self) -> u16;
    fn f_flags(&self) -> u16;

    // Provided methods.

    /// Read the file header.
    ///
    /// Also checks that the magic field in the file header is a supported format.
    fn parse<'data, R: ReadRef<'data>>(data: R, offset: &mut u64) -> Result<&'data Self> {
        let header = data
            .read::<Self>(offset)
            .read_error("Invalid XCOFF header size or alignment")?;
        if !header.is_supported() {
            return Err(Error("Unsupported XCOFF header"));
        }
        Ok(header)
    }

    fn is_supported(&self) -> bool {
        (self.is_type_64() && self.f_magic() == xcoff::MAGIC_64)
            || (!self.is_type_64() && self.f_magic() == xcoff::MAGIC_32)
    }

    /// Read the auxiliary file header.
    fn aux_header<'data, R: ReadRef<'data>>(
        &self,
        data: R,
        offset: &mut u64,
    ) -> Result<Option<&'data Self::AuxHeader>> {
        let aux_header_size = self.f_opthdr();
        if self.f_flags() & xcoff::F_EXEC == 0 {
            // No auxiliary header is required for an object file that is not an executable.
            // TODO: Some AIX programs generate auxiliary headers for 32-bit object files
            // that end after the data_start field.
            *offset += u64::from(aux_header_size);
            return Ok(None);
        }
        // Executables, however, must have auxiliary headers that include the
        // full structure definitions.
        if aux_header_size != mem::size_of::<Self::AuxHeader>() as u16 {
            *offset += u64::from(aux_header_size);
            return Ok(None);
        }
        let aux_header = data
            .read::<Self::AuxHeader>(offset)
            .read_error("Invalid XCOFF auxiliary header size")?;
        Ok(Some(aux_header))
    }

    /// Read the section table.
    #[inline]
    fn sections<'data, R: ReadRef<'data>>(
        &self,
        data: R,
        offset: &mut u64,
    ) -> Result<SectionTable<'data, Self>> {
        SectionTable::parse(self, data, offset)
    }

    /// Return the symbol table.
    #[inline]
    fn symbols<'data, R: ReadRef<'data>>(&self, data: R) -> Result<SymbolTable<'data, Self, R>> {
        SymbolTable::parse(*self, data)
    }
}

impl FileHeader for xcoff::FileHeader32 {
    type Word = u32;
    type AuxHeader = xcoff::AuxHeader32;
    type SectionHeader = xcoff::SectionHeader32;
    type Symbol = xcoff::Symbol32;
    type FileAux = xcoff::FileAux32;
    type CsectAux = xcoff::CsectAux32;
    type Rel = xcoff::Rel32;

    fn is_type_64(&self) -> bool {
        false
    }

    fn f_magic(&self) -> u16 {
        self.f_magic.get(BE)
    }

    fn f_nscns(&self) -> u16 {
        self.f_nscns.get(BE)
    }

    fn f_timdat(&self) -> u32 {
        self.f_timdat.get(BE)
    }

    fn f_symptr(&self) -> Self::Word {
        self.f_symptr.get(BE)
    }

    fn f_nsyms(&self) -> u32 {
        self.f_nsyms.get(BE)
    }

    fn f_opthdr(&self) -> u16 {
        self.f_opthdr.get(BE)
    }

    fn f_flags(&self) -> u16 {
        self.f_flags.get(BE)
    }
}

impl FileHeader for xcoff::FileHeader64 {
    type Word = u64;
    type AuxHeader = xcoff::AuxHeader64;
    type SectionHeader = xcoff::SectionHeader64;
    type Symbol = xcoff::Symbol64;
    type FileAux = xcoff::FileAux64;
    type CsectAux = xcoff::CsectAux64;
    type Rel = xcoff::Rel64;

    fn is_type_64(&self) -> bool {
        true
    }

    fn f_magic(&self) -> u16 {
        self.f_magic.get(BE)
    }

    fn f_nscns(&self) -> u16 {
        self.f_nscns.get(BE)
    }

    fn f_timdat(&self) -> u32 {
        self.f_timdat.get(BE)
    }

    fn f_symptr(&self) -> Self::Word {
        self.f_symptr.get(BE)
    }

    fn f_nsyms(&self) -> u32 {
        self.f_nsyms.get(BE)
    }

    fn f_opthdr(&self) -> u16 {
        self.f_opthdr.get(BE)
    }

    fn f_flags(&self) -> u16 {
        self.f_flags.get(BE)
    }
}

/// A trait for generic access to [`xcoff::AuxHeader32`] and [`xcoff::AuxHeader64`].
#[allow(missing_docs)]
pub trait AuxHeader: Debug + Pod {
    type Word: Into<u64>;

    fn o_mflag(&self) -> u16;
    fn o_vstamp(&self) -> u16;
    fn o_tsize(&self) -> Self::Word;
    fn o_dsize(&self) -> Self::Word;
    fn o_bsize(&self) -> Self::Word;
    fn o_entry(&self) -> Self::Word;
    fn o_text_start(&self) -> Self::Word;
    fn o_data_start(&self) -> Self::Word;
    fn o_toc(&self) -> Self::Word;
    fn o_snentry(&self) -> u16;
    fn o_sntext(&self) -> u16;
    fn o_sndata(&self) -> u16;
    fn o_sntoc(&self) -> u16;
    fn o_snloader(&self) -> u16;
    fn o_snbss(&self) -> u16;
    fn o_algntext(&self) -> u16;
    fn o_algndata(&self) -> u16;
    fn o_modtype(&self) -> u16;
    fn o_cpuflag(&self) -> u8;
    fn o_cputype(&self) -> u8;
    fn o_maxstack(&self) -> Self::Word;
    fn o_maxdata(&self) -> Self::Word;
    fn o_debugger(&self) -> u32;
    fn o_textpsize(&self) -> u8;
    fn o_datapsize(&self) -> u8;
    fn o_stackpsize(&self) -> u8;
    fn o_flags(&self) -> u8;
    fn o_sntdata(&self) -> u16;
    fn o_sntbss(&self) -> u16;
    fn o_x64flags(&self) -> Option<u16>;
}

impl AuxHeader for xcoff::AuxHeader32 {
    type Word = u32;

    fn o_mflag(&self) -> u16 {
        self.o_mflag.get(BE)
    }

    fn o_vstamp(&self) -> u16 {
        self.o_vstamp.get(BE)
    }

    fn o_tsize(&self) -> Self::Word {
        self.o_tsize.get(BE)
    }

    fn o_dsize(&self) -> Self::Word {
        self.o_dsize.get(BE)
    }

    fn o_bsize(&self) -> Self::Word {
        self.o_bsize.get(BE)
    }

    fn o_entry(&self) -> Self::Word {
        self.o_entry.get(BE)
    }

    fn o_text_start(&self) -> Self::Word {
        self.o_text_start.get(BE)
    }

    fn o_data_start(&self) -> Self::Word {
        self.o_data_start.get(BE)
    }

    fn o_toc(&self) -> Self::Word {
        self.o_toc.get(BE)
    }

    fn o_snentry(&self) -> u16 {
        self.o_snentry.get(BE)
    }

    fn o_sntext(&self) -> u16 {
        self.o_sntext.get(BE)
    }

    fn o_sndata(&self) -> u16 {
        self.o_sndata.get(BE)
    }

    fn o_sntoc(&self) -> u16 {
        self.o_sntoc.get(BE)
    }

    fn o_snloader(&self) -> u16 {
        self.o_snloader.get(BE)
    }

    fn o_snbss(&self) -> u16 {
        self.o_snbss.get(BE)
    }

    fn o_algntext(&self) -> u16 {
        self.o_algntext.get(BE)
    }

    fn o_algndata(&self) -> u16 {
        self.o_algndata.get(BE)
    }

    fn o_modtype(&self) -> u16 {
        self.o_modtype.get(BE)
    }

    fn o_cpuflag(&self) -> u8 {
        self.o_cpuflag
    }

    fn o_cputype(&self) -> u8 {
        self.o_cputype
    }

    fn o_maxstack(&self) -> Self::Word {
        self.o_maxstack.get(BE)
    }

    fn o_maxdata(&self) -> Self::Word {
        self.o_maxdata.get(BE)
    }

    fn o_debugger(&self) -> u32 {
        self.o_debugger.get(BE)
    }

    fn o_textpsize(&self) -> u8 {
        self.o_textpsize
    }

    fn o_datapsize(&self) -> u8 {
        self.o_datapsize
    }

    fn o_stackpsize(&self) -> u8 {
        self.o_stackpsize
    }

    fn o_flags(&self) -> u8 {
        self.o_flags
    }

    fn o_sntdata(&self) -> u16 {
        self.o_sntdata.get(BE)
    }

    fn o_sntbss(&self) -> u16 {
        self.o_sntbss.get(BE)
    }

    fn o_x64flags(&self) -> Option<u16> {
        None
    }
}

impl AuxHeader for xcoff::AuxHeader64 {
    type Word = u64;

    fn o_mflag(&self) -> u16 {
        self.o_mflag.get(BE)
    }

    fn o_vstamp(&self) -> u16 {
        self.o_vstamp.get(BE)
    }

    fn o_tsize(&self) -> Self::Word {
        self.o_tsize.get(BE)
    }

    fn o_dsize(&self) -> Self::Word {
        self.o_dsize.get(BE)
    }

    fn o_bsize(&self) -> Self::Word {
        self.o_bsize.get(BE)
    }

    fn o_entry(&self) -> Self::Word {
        self.o_entry.get(BE)
    }

    fn o_text_start(&self) -> Self::Word {
        self.o_text_start.get(BE)
    }

    fn o_data_start(&self) -> Self::Word {
        self.o_data_start.get(BE)
    }

    fn o_toc(&self) -> Self::Word {
        self.o_toc.get(BE)
    }

    fn o_snentry(&self) -> u16 {
        self.o_snentry.get(BE)
    }

    fn o_sntext(&self) -> u16 {
        self.o_sntext.get(BE)
    }

    fn o_sndata(&self) -> u16 {
        self.o_sndata.get(BE)
    }

    fn o_sntoc(&self) -> u16 {
        self.o_sntoc.get(BE)
    }

    fn o_snloader(&self) -> u16 {
        self.o_snloader.get(BE)
    }

    fn o_snbss(&self) -> u16 {
        self.o_snbss.get(BE)
    }

    fn o_algntext(&self) -> u16 {
        self.o_algntext.get(BE)
    }

    fn o_algndata(&self) -> u16 {
        self.o_algndata.get(BE)
    }

    fn o_modtype(&self) -> u16 {
        self.o_modtype.get(BE)
    }

    fn o_cpuflag(&self) -> u8 {
        self.o_cpuflag
    }

    fn o_cputype(&self) -> u8 {
        self.o_cputype
    }

    fn o_maxstack(&self) -> Self::Word {
        self.o_maxstack.get(BE)
    }

    fn o_maxdata(&self) -> Self::Word {
        self.o_maxdata.get(BE)
    }

    fn o_debugger(&self) -> u32 {
        self.o_debugger.get(BE)
    }

    fn o_textpsize(&self) -> u8 {
        self.o_textpsize
    }

    fn o_datapsize(&self) -> u8 {
        self.o_datapsize
    }

    fn o_stackpsize(&self) -> u8 {
        self.o_stackpsize
    }

    fn o_flags(&self) -> u8 {
        self.o_flags
    }

    fn o_sntdata(&self) -> u16 {
        self.o_sntdata.get(BE)
    }

    fn o_sntbss(&self) -> u16 {
        self.o_sntbss.get(BE)
    }

    fn o_x64flags(&self) -> Option<u16> {
        Some(self.o_x64flags.get(BE))
    }
}
