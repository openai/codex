use core::fmt::Debug;
use core::{fmt, result, slice, str};

use crate::endian::{self, Endianness};
use crate::macho;
use crate::pod::Pod;
use crate::read::{
    self, gnu_compression, CompressedData, CompressedFileRange, ObjectSection, ReadError, ReadRef,
    RelocationMap, Result, SectionFlags, SectionIndex, SectionKind,
};

use super::{MachHeader, MachOFile, MachORelocationIterator};

/// An iterator for the sections in a [`MachOFile32`](super::MachOFile32).
pub type MachOSectionIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOSectionIterator<'data, 'file, macho::MachHeader32<Endian>, R>;
/// An iterator for the sections in a [`MachOFile64`](super::MachOFile64).
pub type MachOSectionIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOSectionIterator<'data, 'file, macho::MachHeader64<Endian>, R>;

/// An iterator for the sections in a [`MachOFile`].
pub struct MachOSectionIterator<'data, 'file, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    pub(super) file: &'file MachOFile<'data, Mach, R>,
    pub(super) iter: slice::Iter<'file, MachOSectionInternal<'data, Mach, R>>,
}

impl<'data, 'file, Mach, R> fmt::Debug for MachOSectionIterator<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // It's painful to do much better than this
        f.debug_struct("MachOSectionIterator").finish()
    }
}

impl<'data, 'file, Mach, R> Iterator for MachOSectionIterator<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    type Item = MachOSection<'data, 'file, Mach, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|&internal| MachOSection {
            file: self.file,
            internal,
        })
    }
}

/// A section in a [`MachOFile32`](super::MachOFile32).
pub type MachOSection32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOSection<'data, 'file, macho::MachHeader32<Endian>, R>;
/// A section in a [`MachOFile64`](super::MachOFile64).
pub type MachOSection64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOSection<'data, 'file, macho::MachHeader64<Endian>, R>;

/// A section in a [`MachOFile`].
///
/// Most functionality is provided by the [`ObjectSection`] trait implementation.
#[derive(Debug)]
pub struct MachOSection<'data, 'file, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    pub(super) file: &'file MachOFile<'data, Mach, R>,
    pub(super) internal: MachOSectionInternal<'data, Mach, R>,
}

impl<'data, 'file, Mach, R> MachOSection<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    /// Get the Mach-O file containing this section.
    pub fn macho_file(&self) -> &'file MachOFile<'data, Mach, R> {
        self.file
    }

    /// Get the raw Mach-O section structure.
    pub fn macho_section(&self) -> &'data Mach::Section {
        self.internal.section
    }

    /// Get the raw Mach-O relocation entries.
    pub fn macho_relocations(&self) -> Result<&'data [macho::Relocation<Mach::Endian>]> {
        self.internal
            .section
            .relocations(self.file.endian, self.internal.data)
    }

    fn bytes(&self) -> Result<&'data [u8]> {
        self.internal
            .section
            .data(self.file.endian, self.internal.data)
            .read_error("Invalid Mach-O section size or offset")
    }

    // Try GNU-style "ZLIB" header decompression.
    fn maybe_compressed_gnu(&self) -> Result<Option<CompressedFileRange>> {
        if !self
            .name()
            .map_or(false, |name| name.starts_with("__zdebug_"))
        {
            return Ok(None);
        }
        let (section_offset, section_size) = self
            .file_range()
            .read_error("Invalid ELF GNU compressed section type")?;
        gnu_compression::compressed_file_range(self.internal.data, section_offset, section_size)
            .map(Some)
    }
}

impl<'data, 'file, Mach, R> read::private::Sealed for MachOSection<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Mach, R> ObjectSection<'data> for MachOSection<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    type RelocationIterator = MachORelocationIterator<'data, 'file, Mach, R>;

    #[inline]
    fn index(&self) -> SectionIndex {
        self.internal.index
    }

    #[inline]
    fn address(&self) -> u64 {
        self.internal.section.addr(self.file.endian).into()
    }

    #[inline]
    fn size(&self) -> u64 {
        self.internal.section.size(self.file.endian).into()
    }

    #[inline]
    fn align(&self) -> u64 {
        let align = self.internal.section.align(self.file.endian);
        if align < 64 {
            1 << align
        } else {
            0
        }
    }

    #[inline]
    fn file_range(&self) -> Option<(u64, u64)> {
        self.internal.section.file_range(self.file.endian)
    }

    #[inline]
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

    fn compressed_file_range(&self) -> Result<CompressedFileRange> {
        Ok(if let Some(data) = self.maybe_compressed_gnu()? {
            data
        } else {
            CompressedFileRange::none(self.file_range())
        })
    }

    fn compressed_data(&self) -> read::Result<CompressedData<'data>> {
        self.compressed_file_range()?.data(self.file.data)
    }

    #[inline]
    fn name_bytes(&self) -> Result<&'data [u8]> {
        Ok(self.internal.section.name())
    }

    #[inline]
    fn name(&self) -> Result<&'data str> {
        str::from_utf8(self.internal.section.name())
            .ok()
            .read_error("Non UTF-8 Mach-O section name")
    }

    #[inline]
    fn segment_name_bytes(&self) -> Result<Option<&[u8]>> {
        Ok(Some(self.internal.section.segment_name()))
    }

    #[inline]
    fn segment_name(&self) -> Result<Option<&str>> {
        Ok(Some(
            str::from_utf8(self.internal.section.segment_name())
                .ok()
                .read_error("Non UTF-8 Mach-O segment name")?,
        ))
    }

    fn kind(&self) -> SectionKind {
        self.internal.kind
    }

    fn relocations(&self) -> MachORelocationIterator<'data, 'file, Mach, R> {
        MachORelocationIterator {
            file: self.file,
            relocations: self.macho_relocations().unwrap_or(&[]).iter(),
        }
    }

    fn relocation_map(&self) -> read::Result<RelocationMap> {
        RelocationMap::new(self.file, self)
    }

    fn flags(&self) -> SectionFlags {
        SectionFlags::MachO {
            flags: self.internal.section.flags(self.file.endian),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MachOSectionInternal<'data, Mach: MachHeader, R: ReadRef<'data>> {
    pub index: SectionIndex,
    pub kind: SectionKind,
    pub section: &'data Mach::Section,
    /// The data for the file that contains the section data.
    ///
    /// This is required for dyld caches, where this may be a different subcache
    /// from the file containing the Mach-O load commands.
    pub data: R,
}

impl<'data, Mach: MachHeader, R: ReadRef<'data>> MachOSectionInternal<'data, Mach, R> {
    pub(super) fn parse(index: SectionIndex, section: &'data Mach::Section, data: R) -> Self {
        // TODO: we don't validate flags, should we?
        let kind = match (section.segment_name(), section.name()) {
            (b"__TEXT", b"__text") => SectionKind::Text,
            (b"__TEXT", b"__const") => SectionKind::ReadOnlyData,
            (b"__TEXT", b"__cstring") => SectionKind::ReadOnlyString,
            (b"__TEXT", b"__literal4") => SectionKind::ReadOnlyData,
            (b"__TEXT", b"__literal8") => SectionKind::ReadOnlyData,
            (b"__TEXT", b"__literal16") => SectionKind::ReadOnlyData,
            (b"__TEXT", b"__eh_frame") => SectionKind::ReadOnlyData,
            (b"__TEXT", b"__gcc_except_tab") => SectionKind::ReadOnlyData,
            (b"__DATA", b"__data") => SectionKind::Data,
            (b"__DATA", b"__const") => SectionKind::ReadOnlyData,
            (b"__DATA", b"__bss") => SectionKind::UninitializedData,
            (b"__DATA", b"__common") => SectionKind::Common,
            (b"__DATA", b"__thread_data") => SectionKind::Tls,
            (b"__DATA", b"__thread_bss") => SectionKind::UninitializedTls,
            (b"__DATA", b"__thread_vars") => SectionKind::TlsVariables,
            (b"__DWARF", _) => SectionKind::Debug,
            _ => SectionKind::Unknown,
        };
        MachOSectionInternal {
            index,
            kind,
            section,
            data,
        }
    }
}

/// A trait for generic access to [`macho::Section32`] and [`macho::Section64`].
#[allow(missing_docs)]
pub trait Section: Debug + Pod {
    type Word: Into<u64>;
    type Endian: endian::Endian;

    fn sectname(&self) -> &[u8; 16];
    fn segname(&self) -> &[u8; 16];
    fn addr(&self, endian: Self::Endian) -> Self::Word;
    fn size(&self, endian: Self::Endian) -> Self::Word;
    fn offset(&self, endian: Self::Endian) -> u32;
    fn align(&self, endian: Self::Endian) -> u32;
    fn reloff(&self, endian: Self::Endian) -> u32;
    fn nreloc(&self, endian: Self::Endian) -> u32;
    fn flags(&self, endian: Self::Endian) -> u32;

    /// Return the `sectname` bytes up until the null terminator.
    fn name(&self) -> &[u8] {
        let sectname = &self.sectname()[..];
        match memchr::memchr(b'\0', sectname) {
            Some(end) => &sectname[..end],
            None => sectname,
        }
    }

    /// Return the `segname` bytes up until the null terminator.
    fn segment_name(&self) -> &[u8] {
        let segname = &self.segname()[..];
        match memchr::memchr(b'\0', segname) {
            Some(end) => &segname[..end],
            None => segname,
        }
    }

    /// Return the offset and size of the section in the file.
    ///
    /// Returns `None` for sections that have no data in the file.
    fn file_range(&self, endian: Self::Endian) -> Option<(u64, u64)> {
        match self.flags(endian) & macho::SECTION_TYPE {
            macho::S_ZEROFILL | macho::S_GB_ZEROFILL | macho::S_THREAD_LOCAL_ZEROFILL => None,
            _ => Some((self.offset(endian).into(), self.size(endian).into())),
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
    ) -> result::Result<&'data [u8], ()> {
        if let Some((offset, size)) = self.file_range(endian) {
            data.read_bytes_at(offset, size)
        } else {
            Ok(&[])
        }
    }

    /// Return the relocation array.
    ///
    /// Returns `Err` for invalid values.
    fn relocations<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> Result<&'data [macho::Relocation<Self::Endian>]> {
        data.read_slice_at(self.reloff(endian).into(), self.nreloc(endian) as usize)
            .read_error("Invalid Mach-O relocations offset or number")
    }
}

impl<Endian: endian::Endian> Section for macho::Section32<Endian> {
    type Word = u32;
    type Endian = Endian;

    fn sectname(&self) -> &[u8; 16] {
        &self.sectname
    }
    fn segname(&self) -> &[u8; 16] {
        &self.segname
    }
    fn addr(&self, endian: Self::Endian) -> Self::Word {
        self.addr.get(endian)
    }
    fn size(&self, endian: Self::Endian) -> Self::Word {
        self.size.get(endian)
    }
    fn offset(&self, endian: Self::Endian) -> u32 {
        self.offset.get(endian)
    }
    fn align(&self, endian: Self::Endian) -> u32 {
        self.align.get(endian)
    }
    fn reloff(&self, endian: Self::Endian) -> u32 {
        self.reloff.get(endian)
    }
    fn nreloc(&self, endian: Self::Endian) -> u32 {
        self.nreloc.get(endian)
    }
    fn flags(&self, endian: Self::Endian) -> u32 {
        self.flags.get(endian)
    }
}

impl<Endian: endian::Endian> Section for macho::Section64<Endian> {
    type Word = u64;
    type Endian = Endian;

    fn sectname(&self) -> &[u8; 16] {
        &self.sectname
    }
    fn segname(&self) -> &[u8; 16] {
        &self.segname
    }
    fn addr(&self, endian: Self::Endian) -> Self::Word {
        self.addr.get(endian)
    }
    fn size(&self, endian: Self::Endian) -> Self::Word {
        self.size.get(endian)
    }
    fn offset(&self, endian: Self::Endian) -> u32 {
        self.offset.get(endian)
    }
    fn align(&self, endian: Self::Endian) -> u32 {
        self.align.get(endian)
    }
    fn reloff(&self, endian: Self::Endian) -> u32 {
        self.reloff.get(endian)
    }
    fn nreloc(&self, endian: Self::Endian) -> u32 {
        self.nreloc.get(endian)
    }
    fn flags(&self, endian: Self::Endian) -> u32 {
        self.flags.get(endian)
    }
}
