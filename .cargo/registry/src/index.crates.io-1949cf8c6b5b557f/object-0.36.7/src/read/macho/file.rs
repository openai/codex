use alloc::vec::Vec;
use core::fmt::Debug;
use core::{mem, str};

use crate::endian::{self, BigEndian, Endian, Endianness};
use crate::macho;
use crate::pod::Pod;
use crate::read::{
    self, Architecture, ByteString, ComdatKind, Error, Export, FileFlags, Import,
    NoDynamicRelocationIterator, Object, ObjectComdat, ObjectKind, ObjectMap, ObjectSection,
    ReadError, ReadRef, Result, SectionIndex, SubArchitecture, SymbolIndex,
};

use super::{
    DyldCacheImage, LoadCommandIterator, MachOSection, MachOSectionInternal, MachOSectionIterator,
    MachOSegment, MachOSegmentInternal, MachOSegmentIterator, MachOSymbol, MachOSymbolIterator,
    MachOSymbolTable, Nlist, Section, Segment, SymbolTable,
};

/// A 32-bit Mach-O object file.
///
/// This is a file that starts with [`macho::MachHeader32`], and corresponds
/// to [`crate::FileKind::MachO32`].
pub type MachOFile32<'data, Endian = Endianness, R = &'data [u8]> =
    MachOFile<'data, macho::MachHeader32<Endian>, R>;
/// A 64-bit Mach-O object file.
///
/// This is a file that starts with [`macho::MachHeader64`], and corresponds
/// to [`crate::FileKind::MachO64`].
pub type MachOFile64<'data, Endian = Endianness, R = &'data [u8]> =
    MachOFile<'data, macho::MachHeader64<Endian>, R>;

/// A partially parsed Mach-O file.
///
/// Most of the functionality of this type is provided by the [`Object`] trait implementation.
#[derive(Debug)]
pub struct MachOFile<'data, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    pub(super) endian: Mach::Endian,
    pub(super) data: R,
    pub(super) header_offset: u64,
    pub(super) header: &'data Mach,
    pub(super) segments: Vec<MachOSegmentInternal<'data, Mach, R>>,
    pub(super) sections: Vec<MachOSectionInternal<'data, Mach, R>>,
    pub(super) symbols: SymbolTable<'data, Mach, R>,
}

impl<'data, Mach, R> MachOFile<'data, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    /// Parse the raw Mach-O file data.
    pub fn parse(data: R) -> Result<Self> {
        let header = Mach::parse(data, 0)?;
        let endian = header.endian()?;

        // Build a list of segments and sections to make some operations more efficient.
        let mut segments = Vec::new();
        let mut sections = Vec::new();
        let mut symbols = SymbolTable::default();
        if let Ok(mut commands) = header.load_commands(endian, data, 0) {
            while let Ok(Some(command)) = commands.next() {
                if let Some((segment, section_data)) = Mach::Segment::from_command(command)? {
                    segments.push(MachOSegmentInternal { segment, data });
                    for section in segment.sections(endian, section_data)? {
                        let index = SectionIndex(sections.len() + 1);
                        sections.push(MachOSectionInternal::parse(index, section, data));
                    }
                } else if let Some(symtab) = command.symtab()? {
                    symbols = symtab.symbols(endian, data)?;
                }
            }
        }

        Ok(MachOFile {
            endian,
            data,
            header_offset: 0,
            header,
            segments,
            sections,
            symbols,
        })
    }

    /// Parse the Mach-O file for the given image from the dyld shared cache.
    /// This will read different sections from different subcaches, if necessary.
    pub fn parse_dyld_cache_image<'cache, E: Endian>(
        image: &DyldCacheImage<'data, 'cache, E, R>,
    ) -> Result<Self> {
        let (data, header_offset) = image.image_data_and_offset()?;
        let header = Mach::parse(data, header_offset)?;
        let endian = header.endian()?;

        // Build a list of sections to make some operations more efficient.
        // Also build a list of segments, because we need to remember which ReadRef
        // to read each section's data from. Only the DyldCache knows this information,
        // and we won't have access to it once we've exited this function.
        let mut segments = Vec::new();
        let mut sections = Vec::new();
        let mut linkedit_data: Option<R> = None;
        let mut symtab = None;
        if let Ok(mut commands) = header.load_commands(endian, data, header_offset) {
            while let Ok(Some(command)) = commands.next() {
                if let Some((segment, section_data)) = Mach::Segment::from_command(command)? {
                    // Each segment can be stored in a different subcache. Get the segment's
                    // address and look it up in the cache mappings, to find the correct cache data.
                    // This was observed for the arm64e __LINKEDIT segment in macOS 12.0.1.
                    let addr = segment.vmaddr(endian).into();
                    let (data, _offset) = image
                        .cache
                        .data_and_offset_for_address(addr)
                        .read_error("Could not find segment data in dyld shared cache")?;
                    if segment.name() == macho::SEG_LINKEDIT.as_bytes() {
                        linkedit_data = Some(data);
                    }
                    segments.push(MachOSegmentInternal { segment, data });

                    for section in segment.sections(endian, section_data)? {
                        let index = SectionIndex(sections.len() + 1);
                        sections.push(MachOSectionInternal::parse(index, section, data));
                    }
                } else if let Some(st) = command.symtab()? {
                    symtab = Some(st);
                }
            }
        }

        // The symbols are found in the __LINKEDIT segment, so make sure to read them from the
        // correct subcache.
        let symbols = match (symtab, linkedit_data) {
            (Some(symtab), Some(linkedit_data)) => symtab.symbols(endian, linkedit_data)?,
            _ => SymbolTable::default(),
        };

        Ok(MachOFile {
            endian,
            data,
            header_offset,
            header,
            segments,
            sections,
            symbols,
        })
    }

    /// Return the section at the given index.
    #[inline]
    pub(super) fn section_internal(
        &self,
        index: SectionIndex,
    ) -> Result<&MachOSectionInternal<'data, Mach, R>> {
        index
            .0
            .checked_sub(1)
            .and_then(|index| self.sections.get(index))
            .read_error("Invalid Mach-O section index")
    }

    /// Returns the endianness.
    pub fn endian(&self) -> Mach::Endian {
        self.endian
    }

    /// Returns the raw data.
    pub fn data(&self) -> R {
        self.data
    }

    /// Returns the raw Mach-O file header.
    #[deprecated(note = "Use `macho_header` instead")]
    pub fn raw_header(&self) -> &'data Mach {
        self.header
    }

    /// Get the raw Mach-O file header.
    pub fn macho_header(&self) -> &'data Mach {
        self.header
    }

    /// Get the Mach-O load commands.
    pub fn macho_load_commands(&self) -> Result<LoadCommandIterator<'data, Mach::Endian>> {
        self.header
            .load_commands(self.endian, self.data, self.header_offset)
    }

    /// Get the Mach-O symbol table.
    ///
    /// Returns an empty symbol table if the file has no symbol table.
    pub fn macho_symbol_table(&self) -> &SymbolTable<'data, Mach, R> {
        &self.symbols
    }

    /// Return the `LC_BUILD_VERSION` load command if present.
    pub fn build_version(&self) -> Result<Option<&'data macho::BuildVersionCommand<Mach::Endian>>> {
        let mut commands = self
            .header
            .load_commands(self.endian, self.data, self.header_offset)?;
        while let Some(command) = commands.next()? {
            if let Some(build_version) = command.build_version()? {
                return Ok(Some(build_version));
            }
        }
        Ok(None)
    }
}

impl<'data, Mach, R> read::private::Sealed for MachOFile<'data, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
}

impl<'data, Mach, R> Object<'data> for MachOFile<'data, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    type Segment<'file>
        = MachOSegment<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type SegmentIterator<'file>
        = MachOSegmentIterator<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type Section<'file>
        = MachOSection<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type SectionIterator<'file>
        = MachOSectionIterator<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type Comdat<'file>
        = MachOComdat<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type ComdatIterator<'file>
        = MachOComdatIterator<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type Symbol<'file>
        = MachOSymbol<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type SymbolIterator<'file>
        = MachOSymbolIterator<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type SymbolTable<'file>
        = MachOSymbolTable<'data, 'file, Mach, R>
    where
        Self: 'file,
        'data: 'file;
    type DynamicRelocationIterator<'file>
        = NoDynamicRelocationIterator
    where
        Self: 'file,
        'data: 'file;

    fn architecture(&self) -> Architecture {
        match self.header.cputype(self.endian) {
            macho::CPU_TYPE_ARM => Architecture::Arm,
            macho::CPU_TYPE_ARM64 => Architecture::Aarch64,
            macho::CPU_TYPE_ARM64_32 => Architecture::Aarch64_Ilp32,
            macho::CPU_TYPE_X86 => Architecture::I386,
            macho::CPU_TYPE_X86_64 => Architecture::X86_64,
            macho::CPU_TYPE_MIPS => Architecture::Mips,
            macho::CPU_TYPE_POWERPC => Architecture::PowerPc,
            macho::CPU_TYPE_POWERPC64 => Architecture::PowerPc64,
            _ => Architecture::Unknown,
        }
    }

    fn sub_architecture(&self) -> Option<SubArchitecture> {
        match (
            self.header.cputype(self.endian),
            self.header.cpusubtype(self.endian),
        ) {
            (macho::CPU_TYPE_ARM64, macho::CPU_SUBTYPE_ARM64E) => Some(SubArchitecture::Arm64E),
            _ => None,
        }
    }

    #[inline]
    fn is_little_endian(&self) -> bool {
        self.header.is_little_endian()
    }

    #[inline]
    fn is_64(&self) -> bool {
        self.header.is_type_64()
    }

    fn kind(&self) -> ObjectKind {
        match self.header.filetype(self.endian) {
            macho::MH_OBJECT => ObjectKind::Relocatable,
            macho::MH_EXECUTE => ObjectKind::Executable,
            macho::MH_CORE => ObjectKind::Core,
            macho::MH_DYLIB => ObjectKind::Dynamic,
            _ => ObjectKind::Unknown,
        }
    }

    fn segments(&self) -> MachOSegmentIterator<'data, '_, Mach, R> {
        MachOSegmentIterator {
            file: self,
            iter: self.segments.iter(),
        }
    }

    fn section_by_name_bytes<'file>(
        &'file self,
        section_name: &[u8],
    ) -> Option<MachOSection<'data, 'file, Mach, R>> {
        // Translate the section_name by stripping the query_prefix to construct
        // a function that matches names starting with name_prefix, taking into
        // consideration the maximum section name length.
        let make_prefix_matcher = |query_prefix: &'static [u8], name_prefix: &'static [u8]| {
            const MAX_SECTION_NAME_LEN: usize = 16;
            let suffix = section_name.strip_prefix(query_prefix).map(|suffix| {
                let max_len = MAX_SECTION_NAME_LEN - name_prefix.len();
                &suffix[..suffix.len().min(max_len)]
            });
            move |name: &[u8]| suffix.is_some() && name.strip_prefix(name_prefix) == suffix
        };
        // Matches "__text" when searching for ".text" and "__debug_str_offs"
        // when searching for ".debug_str_offsets", as is common in
        // macOS/Mach-O.
        let matches_underscores_prefix = make_prefix_matcher(b".", b"__");
        // Matches "__zdebug_info" when searching for ".debug_info" and
        // "__zdebug_str_off" when searching for ".debug_str_offsets", as is
        // used by Go when using GNU-style compression.
        let matches_zdebug_prefix = make_prefix_matcher(b".debug_", b"__zdebug_");
        self.sections().find(|section| {
            section.name_bytes().map_or(false, |name| {
                name == section_name
                    || matches_underscores_prefix(name)
                    || matches_zdebug_prefix(name)
            })
        })
    }

    fn section_by_index(&self, index: SectionIndex) -> Result<MachOSection<'data, '_, Mach, R>> {
        let internal = *self.section_internal(index)?;
        Ok(MachOSection {
            file: self,
            internal,
        })
    }

    fn sections(&self) -> MachOSectionIterator<'data, '_, Mach, R> {
        MachOSectionIterator {
            file: self,
            iter: self.sections.iter(),
        }
    }

    fn comdats(&self) -> MachOComdatIterator<'data, '_, Mach, R> {
        MachOComdatIterator { file: self }
    }

    fn symbol_by_index(&self, index: SymbolIndex) -> Result<MachOSymbol<'data, '_, Mach, R>> {
        let nlist = self.symbols.symbol(index)?;
        MachOSymbol::new(self, index, nlist).read_error("Unsupported Mach-O symbol index")
    }

    fn symbols(&self) -> MachOSymbolIterator<'data, '_, Mach, R> {
        MachOSymbolIterator::new(self)
    }

    #[inline]
    fn symbol_table(&self) -> Option<MachOSymbolTable<'data, '_, Mach, R>> {
        Some(MachOSymbolTable { file: self })
    }

    fn dynamic_symbols(&self) -> MachOSymbolIterator<'data, '_, Mach, R> {
        MachOSymbolIterator::empty(self)
    }

    #[inline]
    fn dynamic_symbol_table(&self) -> Option<MachOSymbolTable<'data, '_, Mach, R>> {
        None
    }

    fn object_map(&self) -> ObjectMap<'data> {
        self.symbols.object_map(self.endian)
    }

    fn imports(&self) -> Result<Vec<Import<'data>>> {
        let mut dysymtab = None;
        let mut libraries = Vec::new();
        let twolevel = self.header.flags(self.endian) & macho::MH_TWOLEVEL != 0;
        if twolevel {
            libraries.push(&[][..]);
        }
        let mut commands = self
            .header
            .load_commands(self.endian, self.data, self.header_offset)?;
        while let Some(command) = commands.next()? {
            if let Some(command) = command.dysymtab()? {
                dysymtab = Some(command);
            }
            if twolevel {
                if let Some(dylib) = command.dylib()? {
                    libraries.push(command.string(self.endian, dylib.dylib.name)?);
                }
            }
        }

        let mut imports = Vec::new();
        if let Some(dysymtab) = dysymtab {
            let index = dysymtab.iundefsym.get(self.endian) as usize;
            let number = dysymtab.nundefsym.get(self.endian) as usize;
            for i in index..(index.wrapping_add(number)) {
                let symbol = self.symbols.symbol(SymbolIndex(i))?;
                let name = symbol.name(self.endian, self.symbols.strings())?;
                let library = if twolevel {
                    libraries
                        .get(symbol.library_ordinal(self.endian) as usize)
                        .copied()
                        .read_error("Invalid Mach-O symbol library ordinal")?
                } else {
                    &[]
                };
                imports.push(Import {
                    name: ByteString(name),
                    library: ByteString(library),
                });
            }
        }
        Ok(imports)
    }

    fn exports(&self) -> Result<Vec<Export<'data>>> {
        let mut dysymtab = None;
        let mut commands = self
            .header
            .load_commands(self.endian, self.data, self.header_offset)?;
        while let Some(command) = commands.next()? {
            if let Some(command) = command.dysymtab()? {
                dysymtab = Some(command);
                break;
            }
        }

        let mut exports = Vec::new();
        if let Some(dysymtab) = dysymtab {
            let index = dysymtab.iextdefsym.get(self.endian) as usize;
            let number = dysymtab.nextdefsym.get(self.endian) as usize;
            for i in index..(index.wrapping_add(number)) {
                let symbol = self.symbols.symbol(SymbolIndex(i))?;
                let name = symbol.name(self.endian, self.symbols.strings())?;
                let address = symbol.n_value(self.endian).into();
                exports.push(Export {
                    name: ByteString(name),
                    address,
                });
            }
        }
        Ok(exports)
    }

    #[inline]
    fn dynamic_relocations(&self) -> Option<NoDynamicRelocationIterator> {
        None
    }

    fn has_debug_symbols(&self) -> bool {
        self.section_by_name(".debug_info").is_some()
    }

    fn mach_uuid(&self) -> Result<Option<[u8; 16]>> {
        self.header.uuid(self.endian, self.data, self.header_offset)
    }

    fn relative_address_base(&self) -> u64 {
        0
    }

    fn entry(&self) -> u64 {
        if let Ok(mut commands) =
            self.header
                .load_commands(self.endian, self.data, self.header_offset)
        {
            while let Ok(Some(command)) = commands.next() {
                if let Ok(Some(command)) = command.entry_point() {
                    return command.entryoff.get(self.endian);
                }
            }
        }
        0
    }

    fn flags(&self) -> FileFlags {
        FileFlags::MachO {
            flags: self.header.flags(self.endian),
        }
    }
}

/// An iterator for the COMDAT section groups in a [`MachOFile64`].
pub type MachOComdatIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOComdatIterator<'data, 'file, macho::MachHeader32<Endian>, R>;
/// An iterator for the COMDAT section groups in a [`MachOFile64`].
pub type MachOComdatIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOComdatIterator<'data, 'file, macho::MachHeader64<Endian>, R>;

/// An iterator for the COMDAT section groups in a [`MachOFile`].
///
/// This is a stub that doesn't implement any functionality.
#[derive(Debug)]
pub struct MachOComdatIterator<'data, 'file, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    #[allow(unused)]
    file: &'file MachOFile<'data, Mach, R>,
}

impl<'data, 'file, Mach, R> Iterator for MachOComdatIterator<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    type Item = MachOComdat<'data, 'file, Mach, R>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// A COMDAT section group in a [`MachOFile32`].
pub type MachOComdat32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOComdat<'data, 'file, macho::MachHeader32<Endian>, R>;

/// A COMDAT section group in a [`MachOFile64`].
pub type MachOComdat64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOComdat<'data, 'file, macho::MachHeader64<Endian>, R>;

/// A COMDAT section group in a [`MachOFile`].
///
/// This is a stub that doesn't implement any functionality.
#[derive(Debug)]
pub struct MachOComdat<'data, 'file, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    #[allow(unused)]
    file: &'file MachOFile<'data, Mach, R>,
}

impl<'data, 'file, Mach, R> read::private::Sealed for MachOComdat<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Mach, R> ObjectComdat<'data> for MachOComdat<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    type SectionIterator = MachOComdatSectionIterator<'data, 'file, Mach, R>;

    #[inline]
    fn kind(&self) -> ComdatKind {
        unreachable!();
    }

    #[inline]
    fn symbol(&self) -> SymbolIndex {
        unreachable!();
    }

    #[inline]
    fn name_bytes(&self) -> Result<&'data [u8]> {
        unreachable!();
    }

    #[inline]
    fn name(&self) -> Result<&'data str> {
        unreachable!();
    }

    #[inline]
    fn sections(&self) -> Self::SectionIterator {
        unreachable!();
    }
}

/// An iterator for the sections in a COMDAT section group in a [`MachOFile32`].
pub type MachOComdatSectionIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOComdatSectionIterator<'data, 'file, macho::MachHeader32<Endian>, R>;
/// An iterator for the sections in a COMDAT section group in a [`MachOFile64`].
pub type MachOComdatSectionIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOComdatSectionIterator<'data, 'file, macho::MachHeader64<Endian>, R>;

/// An iterator for the sections in a COMDAT section group in a [`MachOFile`].
///
/// This is a stub that doesn't implement any functionality.
#[derive(Debug)]
pub struct MachOComdatSectionIterator<'data, 'file, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    #[allow(unused)]
    file: &'file MachOFile<'data, Mach, R>,
}

impl<'data, 'file, Mach, R> Iterator for MachOComdatSectionIterator<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    type Item = SectionIndex;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// A trait for generic access to [`macho::MachHeader32`] and [`macho::MachHeader64`].
#[allow(missing_docs)]
pub trait MachHeader: Debug + Pod {
    type Word: Into<u64>;
    type Endian: endian::Endian;
    type Segment: Segment<Endian = Self::Endian, Section = Self::Section>;
    type Section: Section<Endian = Self::Endian>;
    type Nlist: Nlist<Endian = Self::Endian>;

    /// Return true if this type is a 64-bit header.
    ///
    /// This is a property of the type, not a value in the header data.
    fn is_type_64(&self) -> bool;

    /// Return true if the `magic` field signifies big-endian.
    fn is_big_endian(&self) -> bool;

    /// Return true if the `magic` field signifies little-endian.
    fn is_little_endian(&self) -> bool;

    fn magic(&self) -> u32;
    fn cputype(&self, endian: Self::Endian) -> u32;
    fn cpusubtype(&self, endian: Self::Endian) -> u32;
    fn filetype(&self, endian: Self::Endian) -> u32;
    fn ncmds(&self, endian: Self::Endian) -> u32;
    fn sizeofcmds(&self, endian: Self::Endian) -> u32;
    fn flags(&self, endian: Self::Endian) -> u32;

    // Provided methods.

    /// Read the file header.
    ///
    /// Also checks that the magic field in the file header is a supported format.
    fn parse<'data, R: ReadRef<'data>>(data: R, offset: u64) -> read::Result<&'data Self> {
        let header = data
            .read_at::<Self>(offset)
            .read_error("Invalid Mach-O header size or alignment")?;
        if !header.is_supported() {
            return Err(Error("Unsupported Mach-O header"));
        }
        Ok(header)
    }

    fn is_supported(&self) -> bool {
        self.is_little_endian() || self.is_big_endian()
    }

    fn endian(&self) -> Result<Self::Endian> {
        Self::Endian::from_big_endian(self.is_big_endian()).read_error("Unsupported Mach-O endian")
    }

    fn load_commands<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
        header_offset: u64,
    ) -> Result<LoadCommandIterator<'data, Self::Endian>> {
        let data = data
            .read_bytes_at(
                header_offset + mem::size_of::<Self>() as u64,
                self.sizeofcmds(endian).into(),
            )
            .read_error("Invalid Mach-O load command table size")?;
        Ok(LoadCommandIterator::new(endian, data, self.ncmds(endian)))
    }

    /// Return the UUID from the `LC_UUID` load command, if one is present.
    fn uuid<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
        header_offset: u64,
    ) -> Result<Option<[u8; 16]>> {
        let mut commands = self.load_commands(endian, data, header_offset)?;
        while let Some(command) = commands.next()? {
            if let Ok(Some(uuid)) = command.uuid() {
                return Ok(Some(uuid.uuid));
            }
        }
        Ok(None)
    }
}

impl<Endian: endian::Endian> MachHeader for macho::MachHeader32<Endian> {
    type Word = u32;
    type Endian = Endian;
    type Segment = macho::SegmentCommand32<Endian>;
    type Section = macho::Section32<Endian>;
    type Nlist = macho::Nlist32<Endian>;

    fn is_type_64(&self) -> bool {
        false
    }

    fn is_big_endian(&self) -> bool {
        self.magic() == macho::MH_MAGIC
    }

    fn is_little_endian(&self) -> bool {
        self.magic() == macho::MH_CIGAM
    }

    fn magic(&self) -> u32 {
        self.magic.get(BigEndian)
    }

    fn cputype(&self, endian: Self::Endian) -> u32 {
        self.cputype.get(endian)
    }

    fn cpusubtype(&self, endian: Self::Endian) -> u32 {
        self.cpusubtype.get(endian)
    }

    fn filetype(&self, endian: Self::Endian) -> u32 {
        self.filetype.get(endian)
    }

    fn ncmds(&self, endian: Self::Endian) -> u32 {
        self.ncmds.get(endian)
    }

    fn sizeofcmds(&self, endian: Self::Endian) -> u32 {
        self.sizeofcmds.get(endian)
    }

    fn flags(&self, endian: Self::Endian) -> u32 {
        self.flags.get(endian)
    }
}

impl<Endian: endian::Endian> MachHeader for macho::MachHeader64<Endian> {
    type Word = u64;
    type Endian = Endian;
    type Segment = macho::SegmentCommand64<Endian>;
    type Section = macho::Section64<Endian>;
    type Nlist = macho::Nlist64<Endian>;

    fn is_type_64(&self) -> bool {
        true
    }

    fn is_big_endian(&self) -> bool {
        self.magic() == macho::MH_MAGIC_64
    }

    fn is_little_endian(&self) -> bool {
        self.magic() == macho::MH_CIGAM_64
    }

    fn magic(&self) -> u32 {
        self.magic.get(BigEndian)
    }

    fn cputype(&self, endian: Self::Endian) -> u32 {
        self.cputype.get(endian)
    }

    fn cpusubtype(&self, endian: Self::Endian) -> u32 {
        self.cpusubtype.get(endian)
    }

    fn filetype(&self, endian: Self::Endian) -> u32 {
        self.filetype.get(endian)
    }

    fn ncmds(&self, endian: Self::Endian) -> u32 {
        self.ncmds.get(endian)
    }

    fn sizeofcmds(&self, endian: Self::Endian) -> u32 {
        self.sizeofcmds.get(endian)
    }

    fn flags(&self, endian: Self::Endian) -> u32 {
        self.flags.get(endian)
    }
}
