use core::fmt::Debug;
use core::{result, slice, str};

use crate::endian::{self, Endianness};
use crate::macho;
use crate::pod::Pod;
use crate::read::{self, ObjectSegment, ReadError, ReadRef, Result, SegmentFlags};

use super::{LoadCommandData, MachHeader, MachOFile, Section};

/// An iterator for the segments in a [`MachOFile32`](super::MachOFile32).
pub type MachOSegmentIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOSegmentIterator<'data, 'file, macho::MachHeader32<Endian>, R>;
/// An iterator for the segments in a [`MachOFile64`](super::MachOFile64).
pub type MachOSegmentIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOSegmentIterator<'data, 'file, macho::MachHeader64<Endian>, R>;

/// An iterator for the segments in a [`MachOFile`].
#[derive(Debug)]
pub struct MachOSegmentIterator<'data, 'file, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    pub(super) file: &'file MachOFile<'data, Mach, R>,
    pub(super) iter: slice::Iter<'file, MachOSegmentInternal<'data, Mach, R>>,
}

impl<'data, 'file, Mach, R> Iterator for MachOSegmentIterator<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    type Item = MachOSegment<'data, 'file, Mach, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|internal| MachOSegment {
            file: self.file,
            internal,
        })
    }
}

/// A segment in a [`MachOFile32`](super::MachOFile32).
pub type MachOSegment32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOSegment<'data, 'file, macho::MachHeader32<Endian>, R>;
/// A segment in a [`MachOFile64`](super::MachOFile64).
pub type MachOSegment64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachOSegment<'data, 'file, macho::MachHeader64<Endian>, R>;

/// A segment in a [`MachOFile`].
///
/// Most functionality is provided by the [`ObjectSegment`] trait implementation.
#[derive(Debug)]
pub struct MachOSegment<'data, 'file, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    file: &'file MachOFile<'data, Mach, R>,
    internal: &'file MachOSegmentInternal<'data, Mach, R>,
}

impl<'data, 'file, Mach, R> MachOSegment<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    /// Get the Mach-O file containing this segment.
    pub fn macho_file(&self) -> &'file MachOFile<'data, Mach, R> {
        self.file
    }

    /// Get the raw Mach-O segment structure.
    pub fn macho_segment(&self) -> &'data Mach::Segment {
        self.internal.segment
    }

    fn bytes(&self) -> Result<&'data [u8]> {
        self.internal
            .segment
            .data(self.file.endian, self.internal.data)
            .read_error("Invalid Mach-O segment size or offset")
    }
}

impl<'data, 'file, Mach, R> read::private::Sealed for MachOSegment<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
}

impl<'data, 'file, Mach, R> ObjectSegment<'data> for MachOSegment<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    #[inline]
    fn address(&self) -> u64 {
        self.internal.segment.vmaddr(self.file.endian).into()
    }

    #[inline]
    fn size(&self) -> u64 {
        self.internal.segment.vmsize(self.file.endian).into()
    }

    #[inline]
    fn align(&self) -> u64 {
        // Page size.
        0x1000
    }

    #[inline]
    fn file_range(&self) -> (u64, u64) {
        self.internal.segment.file_range(self.file.endian)
    }

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

    #[inline]
    fn name_bytes(&self) -> Result<Option<&[u8]>> {
        Ok(Some(self.internal.segment.name()))
    }

    #[inline]
    fn name(&self) -> Result<Option<&str>> {
        Ok(Some(
            str::from_utf8(self.internal.segment.name())
                .ok()
                .read_error("Non UTF-8 Mach-O segment name")?,
        ))
    }

    #[inline]
    fn flags(&self) -> SegmentFlags {
        let flags = self.internal.segment.flags(self.file.endian);
        let maxprot = self.internal.segment.maxprot(self.file.endian);
        let initprot = self.internal.segment.initprot(self.file.endian);
        SegmentFlags::MachO {
            flags,
            maxprot,
            initprot,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MachOSegmentInternal<'data, Mach: MachHeader, R: ReadRef<'data>> {
    pub segment: &'data Mach::Segment,
    /// The data for the file that contains the segment data.
    ///
    /// This is required for dyld caches, where this may be a different subcache
    /// from the file containing the Mach-O load commands.
    pub data: R,
}

/// A trait for generic access to [`macho::SegmentCommand32`] and [`macho::SegmentCommand64`].
#[allow(missing_docs)]
pub trait Segment: Debug + Pod {
    type Word: Into<u64>;
    type Endian: endian::Endian;
    type Section: Section<Endian = Self::Endian>;

    fn from_command(command: LoadCommandData<'_, Self::Endian>) -> Result<Option<(&Self, &[u8])>>;

    fn cmd(&self, endian: Self::Endian) -> u32;
    fn cmdsize(&self, endian: Self::Endian) -> u32;
    fn segname(&self) -> &[u8; 16];
    fn vmaddr(&self, endian: Self::Endian) -> Self::Word;
    fn vmsize(&self, endian: Self::Endian) -> Self::Word;
    fn fileoff(&self, endian: Self::Endian) -> Self::Word;
    fn filesize(&self, endian: Self::Endian) -> Self::Word;
    fn maxprot(&self, endian: Self::Endian) -> u32;
    fn initprot(&self, endian: Self::Endian) -> u32;
    fn nsects(&self, endian: Self::Endian) -> u32;
    fn flags(&self, endian: Self::Endian) -> u32;

    /// Return the `segname` bytes up until the null terminator.
    fn name(&self) -> &[u8] {
        let segname = &self.segname()[..];
        match memchr::memchr(b'\0', segname) {
            Some(end) => &segname[..end],
            None => segname,
        }
    }

    /// Return the offset and size of the segment in the file.
    fn file_range(&self, endian: Self::Endian) -> (u64, u64) {
        (self.fileoff(endian).into(), self.filesize(endian).into())
    }

    /// Get the segment data from the file data.
    ///
    /// Returns `Err` for invalid values.
    fn data<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        data: R,
    ) -> result::Result<&'data [u8], ()> {
        let (offset, size) = self.file_range(endian);
        data.read_bytes_at(offset, size)
    }

    /// Get the array of sections from the data following the segment command.
    ///
    /// Returns `Err` for invalid values.
    fn sections<'data, R: ReadRef<'data>>(
        &self,
        endian: Self::Endian,
        section_data: R,
    ) -> Result<&'data [Self::Section]> {
        section_data
            .read_slice_at(0, self.nsects(endian) as usize)
            .read_error("Invalid Mach-O number of sections")
    }
}

impl<Endian: endian::Endian> Segment for macho::SegmentCommand32<Endian> {
    type Word = u32;
    type Endian = Endian;
    type Section = macho::Section32<Self::Endian>;

    fn from_command(command: LoadCommandData<'_, Self::Endian>) -> Result<Option<(&Self, &[u8])>> {
        command.segment_32()
    }

    fn cmd(&self, endian: Self::Endian) -> u32 {
        self.cmd.get(endian)
    }
    fn cmdsize(&self, endian: Self::Endian) -> u32 {
        self.cmdsize.get(endian)
    }
    fn segname(&self) -> &[u8; 16] {
        &self.segname
    }
    fn vmaddr(&self, endian: Self::Endian) -> Self::Word {
        self.vmaddr.get(endian)
    }
    fn vmsize(&self, endian: Self::Endian) -> Self::Word {
        self.vmsize.get(endian)
    }
    fn fileoff(&self, endian: Self::Endian) -> Self::Word {
        self.fileoff.get(endian)
    }
    fn filesize(&self, endian: Self::Endian) -> Self::Word {
        self.filesize.get(endian)
    }
    fn maxprot(&self, endian: Self::Endian) -> u32 {
        self.maxprot.get(endian)
    }
    fn initprot(&self, endian: Self::Endian) -> u32 {
        self.initprot.get(endian)
    }
    fn nsects(&self, endian: Self::Endian) -> u32 {
        self.nsects.get(endian)
    }
    fn flags(&self, endian: Self::Endian) -> u32 {
        self.flags.get(endian)
    }
}

impl<Endian: endian::Endian> Segment for macho::SegmentCommand64<Endian> {
    type Word = u64;
    type Endian = Endian;
    type Section = macho::Section64<Self::Endian>;

    fn from_command(command: LoadCommandData<'_, Self::Endian>) -> Result<Option<(&Self, &[u8])>> {
        command.segment_64()
    }

    fn cmd(&self, endian: Self::Endian) -> u32 {
        self.cmd.get(endian)
    }
    fn cmdsize(&self, endian: Self::Endian) -> u32 {
        self.cmdsize.get(endian)
    }
    fn segname(&self) -> &[u8; 16] {
        &self.segname
    }
    fn vmaddr(&self, endian: Self::Endian) -> Self::Word {
        self.vmaddr.get(endian)
    }
    fn vmsize(&self, endian: Self::Endian) -> Self::Word {
        self.vmsize.get(endian)
    }
    fn fileoff(&self, endian: Self::Endian) -> Self::Word {
        self.fileoff.get(endian)
    }
    fn filesize(&self, endian: Self::Endian) -> Self::Word {
        self.filesize.get(endian)
    }
    fn maxprot(&self, endian: Self::Endian) -> u32 {
        self.maxprot.get(endian)
    }
    fn initprot(&self, endian: Self::Endian) -> u32 {
        self.initprot.get(endian)
    }
    fn nsects(&self, endian: Self::Endian) -> u32 {
        self.nsects.get(endian)
    }
    fn flags(&self, endian: Self::Endian) -> u32 {
        self.flags.get(endian)
    }
}
