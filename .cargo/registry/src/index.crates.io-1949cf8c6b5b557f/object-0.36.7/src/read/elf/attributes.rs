use core::convert::TryInto;

use crate::elf;
use crate::endian;
use crate::read::{Bytes, Error, ReadError, Result};

use super::FileHeader;

/// An ELF attributes section.
///
/// This may be a GNU attributes section, or an architecture specific attributes section.
///
/// An attributes section contains a series of [`AttributesSubsection`].
///
/// Returned by [`SectionHeader::attributes`](super::SectionHeader::attributes)
/// and [`SectionHeader::gnu_attributes`](super::SectionHeader::gnu_attributes).
#[derive(Debug, Clone)]
pub struct AttributesSection<'data, Elf: FileHeader> {
    endian: Elf::Endian,
    version: u8,
    data: Bytes<'data>,
}

impl<'data, Elf: FileHeader> AttributesSection<'data, Elf> {
    /// Parse an ELF attributes section given the section data.
    pub fn new(endian: Elf::Endian, data: &'data [u8]) -> Result<Self> {
        let mut data = Bytes(data);

        // Skip the version field that is one byte long.
        // If the section is empty then the version doesn't matter.
        let version = data.read::<u8>().cloned().unwrap_or(b'A');

        Ok(AttributesSection {
            endian,
            version,
            data,
        })
    }

    /// Return the version of the attributes section.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Return an iterator over the subsections.
    pub fn subsections(&self) -> Result<AttributesSubsectionIterator<'data, Elf>> {
        // There is currently only one format version.
        if self.version != b'A' {
            return Err(Error("Unsupported ELF attributes section version"));
        }

        Ok(AttributesSubsectionIterator {
            endian: self.endian,
            data: self.data,
        })
    }
}

/// An iterator for the subsections in an [`AttributesSection`].
#[derive(Debug, Clone)]
pub struct AttributesSubsectionIterator<'data, Elf: FileHeader> {
    endian: Elf::Endian,
    data: Bytes<'data>,
}

impl<'data, Elf: FileHeader> AttributesSubsectionIterator<'data, Elf> {
    /// Return the next subsection.
    pub fn next(&mut self) -> Result<Option<AttributesSubsection<'data, Elf>>> {
        if self.data.is_empty() {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.data = Bytes(&[]);
        }
        result
    }

    fn parse(&mut self) -> Result<AttributesSubsection<'data, Elf>> {
        // First read the subsection length.
        let mut data = self.data;
        let length = data
            .read::<endian::U32Bytes<Elf::Endian>>()
            .read_error("ELF attributes section is too short")?
            .get(self.endian);

        // Now read the entire subsection, updating self.data.
        let mut data = self
            .data
            .read_bytes(length as usize)
            .read_error("Invalid ELF attributes subsection length")?;
        // Skip the subsection length field.
        data.skip(4)
            .read_error("Invalid ELF attributes subsection length")?;

        // TODO: errors here should not prevent reading the next subsection.
        let vendor = data
            .read_string()
            .read_error("Invalid ELF attributes vendor")?;

        Ok(AttributesSubsection {
            endian: self.endian,
            length,
            vendor,
            data,
        })
    }
}

impl<'data, Elf: FileHeader> Iterator for AttributesSubsectionIterator<'data, Elf> {
    type Item = Result<AttributesSubsection<'data, Elf>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// A subsection in an [`AttributesSection`].
///
/// A subsection is identified by a vendor name.  It contains a series of
/// [`AttributesSubsubsection`].
#[derive(Debug, Clone)]
pub struct AttributesSubsection<'data, Elf: FileHeader> {
    endian: Elf::Endian,
    length: u32,
    vendor: &'data [u8],
    data: Bytes<'data>,
}

impl<'data, Elf: FileHeader> AttributesSubsection<'data, Elf> {
    /// Return the length of the attributes subsection.
    pub fn length(&self) -> u32 {
        self.length
    }

    /// Return the vendor name of the attributes subsection.
    pub fn vendor(&self) -> &'data [u8] {
        self.vendor
    }

    /// Return an iterator over the sub-subsections.
    pub fn subsubsections(&self) -> AttributesSubsubsectionIterator<'data, Elf> {
        AttributesSubsubsectionIterator {
            endian: self.endian,
            data: self.data,
        }
    }
}

/// An iterator for the sub-subsections in an [`AttributesSubsection`].
#[derive(Debug, Clone)]
pub struct AttributesSubsubsectionIterator<'data, Elf: FileHeader> {
    endian: Elf::Endian,
    data: Bytes<'data>,
}

impl<'data, Elf: FileHeader> AttributesSubsubsectionIterator<'data, Elf> {
    /// Return the next sub-subsection.
    pub fn next(&mut self) -> Result<Option<AttributesSubsubsection<'data>>> {
        if self.data.is_empty() {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.data = Bytes(&[]);
        }
        result
    }

    fn parse(&mut self) -> Result<AttributesSubsubsection<'data>> {
        // The format of a sub-section looks like this:
        //
        // <file-tag> <size> <attribute>*
        // | <section-tag> <size> <section-number>* 0 <attribute>*
        // | <symbol-tag> <size> <symbol-number>* 0 <attribute>*
        let mut data = self.data;
        let tag = *data
            .read::<u8>()
            .read_error("ELF attributes subsection is too short")?;
        let length = data
            .read::<endian::U32Bytes<Elf::Endian>>()
            .read_error("ELF attributes subsection is too short")?
            .get(self.endian);

        // Now read the entire sub-subsection, updating self.data.
        let mut data = self
            .data
            .read_bytes(length as usize)
            .read_error("Invalid ELF attributes sub-subsection length")?;
        // Skip the tag and sub-subsection size field.
        data.skip(1 + 4)
            .read_error("Invalid ELF attributes sub-subsection length")?;

        // TODO: errors here should not prevent reading the next sub-subsection.
        let indices = if tag == elf::Tag_Section || tag == elf::Tag_Symbol {
            data.read_string()
                .map(Bytes)
                .read_error("Missing ELF attributes sub-subsection indices")?
        } else if tag == elf::Tag_File {
            Bytes(&[])
        } else {
            return Err(Error("Unimplemented ELF attributes sub-subsection tag"));
        };

        Ok(AttributesSubsubsection {
            tag,
            length,
            indices,
            data,
        })
    }
}

impl<'data, Elf: FileHeader> Iterator for AttributesSubsubsectionIterator<'data, Elf> {
    type Item = Result<AttributesSubsubsection<'data>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// A sub-subsection in an [`AttributesSubsection`].
///
/// A sub-subsection is identified by a tag.  It contains an optional series of indices,
/// followed by a series of attributes.
#[derive(Debug, Clone)]
pub struct AttributesSubsubsection<'data> {
    tag: u8,
    length: u32,
    indices: Bytes<'data>,
    data: Bytes<'data>,
}

impl<'data> AttributesSubsubsection<'data> {
    /// Return the tag of the attributes sub-subsection.
    pub fn tag(&self) -> u8 {
        self.tag
    }

    /// Return the length of the attributes sub-subsection.
    pub fn length(&self) -> u32 {
        self.length
    }

    /// Return the data containing the indices.
    pub fn indices_data(&self) -> &'data [u8] {
        self.indices.0
    }

    /// Return the indices.
    ///
    /// This will be section indices if the tag is `Tag_Section`,
    /// or symbol indices if the tag is `Tag_Symbol`,
    /// and otherwise it will be empty.
    pub fn indices(&self) -> AttributeIndexIterator<'data> {
        AttributeIndexIterator { data: self.indices }
    }

    /// Return the data containing the attributes.
    pub fn attributes_data(&self) -> &'data [u8] {
        self.data.0
    }

    /// Return a parser for the data containing the attributes.
    pub fn attributes(&self) -> AttributeReader<'data> {
        AttributeReader { data: self.data }
    }
}

/// An iterator over the indices in an [`AttributesSubsubsection`].
#[derive(Debug, Clone)]
pub struct AttributeIndexIterator<'data> {
    data: Bytes<'data>,
}

impl<'data> AttributeIndexIterator<'data> {
    /// Parse the next index.
    pub fn next(&mut self) -> Result<Option<u32>> {
        if self.data.is_empty() {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.data = Bytes(&[]);
        }
        result
    }

    fn parse(&mut self) -> Result<u32> {
        let err = "Invalid ELF attribute index";
        self.data
            .read_uleb128()
            .read_error(err)?
            .try_into()
            .map_err(|_| ())
            .read_error(err)
    }
}

impl<'data> Iterator for AttributeIndexIterator<'data> {
    type Item = Result<u32>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// A parser for the attributes in an [`AttributesSubsubsection`].
///
/// The parser relies on the caller to know the format of the data for each attribute tag.
#[derive(Debug, Clone)]
pub struct AttributeReader<'data> {
    data: Bytes<'data>,
}

impl<'data> AttributeReader<'data> {
    /// Parse a tag.
    pub fn read_tag(&mut self) -> Result<Option<u64>> {
        if self.data.is_empty() {
            return Ok(None);
        }
        let err = "Invalid ELF attribute tag";
        self.data.read_uleb128().read_error(err).map(Some)
    }

    /// Parse an integer value.
    pub fn read_integer(&mut self) -> Result<u64> {
        let err = "Invalid ELF attribute integer value";
        self.data.read_uleb128().read_error(err)
    }

    /// Parse a string value.
    pub fn read_string(&mut self) -> Result<&'data [u8]> {
        let err = "Invalid ELF attribute string value";
        self.data.read_string().read_error(err)
    }
}
