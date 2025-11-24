use core::fmt::Debug;
use core::mem;

use crate::elf;
use crate::endian::{self, U32};
use crate::pod::Pod;
use crate::read::util;
use crate::read::{self, Bytes, Error, ReadError};

use super::FileHeader;

/// An iterator over the notes in an ELF section or segment.
///
/// Returned [`ProgramHeader::notes`](super::ProgramHeader::notes)
/// and [`SectionHeader::notes`](super::SectionHeader::notes).
#[derive(Debug)]
pub struct NoteIterator<'data, Elf>
where
    Elf: FileHeader,
{
    endian: Elf::Endian,
    align: usize,
    data: Bytes<'data>,
}

impl<'data, Elf> NoteIterator<'data, Elf>
where
    Elf: FileHeader,
{
    /// An iterator over the notes in an ELF section or segment.
    ///
    /// `align` should be from the `p_align` field of the segment,
    /// or the `sh_addralign` field of the section. Supported values are
    /// either 4 or 8, but values less than 4 are treated as 4.
    /// This matches the behaviour of binutils.
    ///
    /// Returns `Err` if `align` is invalid.
    pub fn new(endian: Elf::Endian, align: Elf::Word, data: &'data [u8]) -> read::Result<Self> {
        let align = match align.into() {
            0u64..=4 => 4,
            8 => 8,
            _ => return Err(Error("Invalid ELF note alignment")),
        };
        // TODO: check data alignment?
        Ok(NoteIterator {
            endian,
            align,
            data: Bytes(data),
        })
    }

    /// Returns the next note.
    pub fn next(&mut self) -> read::Result<Option<Note<'data, Elf>>> {
        if self.data.is_empty() {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.data = Bytes(&[]);
        }
        result
    }

    fn parse(&mut self) -> read::Result<Note<'data, Elf>> {
        let header = self
            .data
            .read_at::<Elf::NoteHeader>(0)
            .read_error("ELF note is too short")?;

        // The name has no alignment requirement.
        let offset = mem::size_of::<Elf::NoteHeader>();
        let namesz = header.n_namesz(self.endian) as usize;
        let name = self
            .data
            .read_bytes_at(offset, namesz)
            .read_error("Invalid ELF note namesz")?
            .0;

        // The descriptor must be aligned.
        let offset = util::align(offset + namesz, self.align);
        let descsz = header.n_descsz(self.endian) as usize;
        let desc = self
            .data
            .read_bytes_at(offset, descsz)
            .read_error("Invalid ELF note descsz")?
            .0;

        // The next note (if any) must be aligned.
        let offset = util::align(offset + descsz, self.align);
        if self.data.skip(offset).is_err() {
            self.data = Bytes(&[]);
        }

        Ok(Note { header, name, desc })
    }
}

impl<'data, Elf: FileHeader> Iterator for NoteIterator<'data, Elf> {
    type Item = read::Result<Note<'data, Elf>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// A parsed [`NoteHeader`].
#[derive(Debug)]
pub struct Note<'data, Elf>
where
    Elf: FileHeader,
{
    header: &'data Elf::NoteHeader,
    name: &'data [u8],
    desc: &'data [u8],
}

impl<'data, Elf: FileHeader> Note<'data, Elf> {
    /// Return the `n_type` field of the `NoteHeader`.
    ///
    /// The meaning of this field is determined by `name`.
    pub fn n_type(&self, endian: Elf::Endian) -> u32 {
        self.header.n_type(endian)
    }

    /// Return the `n_namesz` field of the `NoteHeader`.
    pub fn n_namesz(&self, endian: Elf::Endian) -> u32 {
        self.header.n_namesz(endian)
    }

    /// Return the `n_descsz` field of the `NoteHeader`.
    pub fn n_descsz(&self, endian: Elf::Endian) -> u32 {
        self.header.n_descsz(endian)
    }

    /// Return the bytes for the name field following the `NoteHeader`.
    ///
    /// This field is usually a string including one or more trailing null bytes
    /// (but it is not required to be).
    ///
    /// The length of this field is given by `n_namesz`.
    pub fn name_bytes(&self) -> &'data [u8] {
        self.name
    }

    /// Return the bytes for the name field following the `NoteHeader`,
    /// excluding all trailing null bytes.
    pub fn name(&self) -> &'data [u8] {
        let mut name = self.name;
        while let [rest @ .., 0] = name {
            name = rest;
        }
        name
    }

    /// Return the bytes for the desc field following the `NoteHeader`.
    ///
    /// The length of this field is given by `n_descsz`. The meaning
    /// of this field is determined by `name` and `n_type`.
    pub fn desc(&self) -> &'data [u8] {
        self.desc
    }

    /// Return an iterator for properties if this note's type is [`elf::NT_GNU_PROPERTY_TYPE_0`].
    pub fn gnu_properties(
        &self,
        endian: Elf::Endian,
    ) -> Option<GnuPropertyIterator<'data, Elf::Endian>> {
        if self.name() != elf::ELF_NOTE_GNU || self.n_type(endian) != elf::NT_GNU_PROPERTY_TYPE_0 {
            return None;
        }
        // Use the ELF class instead of the section alignment.
        // This matches what other parsers do.
        let align = if Elf::is_type_64_sized() { 8 } else { 4 };
        Some(GnuPropertyIterator {
            endian,
            align,
            data: Bytes(self.desc),
        })
    }
}

/// A trait for generic access to [`elf::NoteHeader32`] and [`elf::NoteHeader64`].
#[allow(missing_docs)]
pub trait NoteHeader: Debug + Pod {
    type Endian: endian::Endian;

    fn n_namesz(&self, endian: Self::Endian) -> u32;
    fn n_descsz(&self, endian: Self::Endian) -> u32;
    fn n_type(&self, endian: Self::Endian) -> u32;
}

impl<Endian: endian::Endian> NoteHeader for elf::NoteHeader32<Endian> {
    type Endian = Endian;

    #[inline]
    fn n_namesz(&self, endian: Self::Endian) -> u32 {
        self.n_namesz.get(endian)
    }

    #[inline]
    fn n_descsz(&self, endian: Self::Endian) -> u32 {
        self.n_descsz.get(endian)
    }

    #[inline]
    fn n_type(&self, endian: Self::Endian) -> u32 {
        self.n_type.get(endian)
    }
}

impl<Endian: endian::Endian> NoteHeader for elf::NoteHeader64<Endian> {
    type Endian = Endian;

    #[inline]
    fn n_namesz(&self, endian: Self::Endian) -> u32 {
        self.n_namesz.get(endian)
    }

    #[inline]
    fn n_descsz(&self, endian: Self::Endian) -> u32 {
        self.n_descsz.get(endian)
    }

    #[inline]
    fn n_type(&self, endian: Self::Endian) -> u32 {
        self.n_type.get(endian)
    }
}

/// An iterator for the properties in a [`elf::NT_GNU_PROPERTY_TYPE_0`] note.
///
/// Returned by [`Note::gnu_properties`].
#[derive(Debug)]
pub struct GnuPropertyIterator<'data, Endian: endian::Endian> {
    endian: Endian,
    align: usize,
    data: Bytes<'data>,
}

impl<'data, Endian: endian::Endian> GnuPropertyIterator<'data, Endian> {
    /// Returns the next property.
    pub fn next(&mut self) -> read::Result<Option<GnuProperty<'data>>> {
        if self.data.is_empty() {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.data = Bytes(&[]);
        }
        result
    }

    fn parse(&mut self) -> read::Result<GnuProperty<'data>> {
        (|| -> Result<_, ()> {
            let pr_type = self.data.read_at::<U32<Endian>>(0)?.get(self.endian);
            let pr_datasz = self.data.read_at::<U32<Endian>>(4)?.get(self.endian) as usize;
            let pr_data = self.data.read_bytes_at(8, pr_datasz)?.0;
            self.data.skip(util::align(8 + pr_datasz, self.align))?;
            Ok(GnuProperty { pr_type, pr_data })
        })()
        .read_error("Invalid ELF GNU property")
    }
}

impl<'data, Endian: endian::Endian> Iterator for GnuPropertyIterator<'data, Endian> {
    type Item = read::Result<GnuProperty<'data>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// A property in a [`elf::NT_GNU_PROPERTY_TYPE_0`] note.
#[derive(Debug)]
pub struct GnuProperty<'data> {
    pr_type: u32,
    pr_data: &'data [u8],
}

impl<'data> GnuProperty<'data> {
    /// Return the property type.
    ///
    /// This is one of the `GNU_PROPERTY_*` constants.
    pub fn pr_type(&self) -> u32 {
        self.pr_type
    }

    /// Return the property data.
    pub fn pr_data(&self) -> &'data [u8] {
        self.pr_data
    }

    /// Parse the property data as an unsigned 32-bit integer.
    pub fn data_u32<E: endian::Endian>(&self, endian: E) -> read::Result<u32> {
        Bytes(self.pr_data)
            .read_at::<U32<E>>(0)
            .read_error("Invalid ELF GNU property data")
            .map(|val| val.get(endian))
    }
}
