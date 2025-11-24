use alloc::string::String;
use core::char;

use crate::endian::{LittleEndian as LE, U16Bytes};
use crate::pe;
use crate::read::{ReadError, ReadRef, Result};

/// The `.rsrc` section of a PE file.
///
/// Returned by [`DataDirectories::resource_directory`](super::DataDirectories::resource_directory).
#[derive(Debug, Clone, Copy)]
pub struct ResourceDirectory<'data> {
    data: &'data [u8],
}

impl<'data> ResourceDirectory<'data> {
    /// Construct from the data of the `.rsrc` section.
    pub fn new(data: &'data [u8]) -> Self {
        ResourceDirectory { data }
    }

    /// Parses the root resource directory.
    pub fn root(&self) -> Result<ResourceDirectoryTable<'data>> {
        ResourceDirectoryTable::parse(self.data, 0)
    }
}

/// A table of resource entries.
#[derive(Debug, Clone)]
pub struct ResourceDirectoryTable<'data> {
    /// The table header.
    pub header: &'data pe::ImageResourceDirectory,
    /// The table entries.
    pub entries: &'data [pe::ImageResourceDirectoryEntry],
}

impl<'data> ResourceDirectoryTable<'data> {
    fn parse(data: &'data [u8], offset: u32) -> Result<Self> {
        let mut offset = u64::from(offset);
        let header = data
            .read::<pe::ImageResourceDirectory>(&mut offset)
            .read_error("Invalid resource table header")?;
        let entries_count = header.number_of_id_entries.get(LE) as usize
            + header.number_of_named_entries.get(LE) as usize;
        let entries = data
            .read_slice::<pe::ImageResourceDirectoryEntry>(&mut offset, entries_count)
            .read_error("Invalid resource table entries")?;
        Ok(Self { header, entries })
    }
}

impl pe::ImageResourceDirectoryEntry {
    /// Returns true if the entry has a name, rather than an ID.
    pub fn has_name(&self) -> bool {
        self.name_or_id.get(LE) & pe::IMAGE_RESOURCE_NAME_IS_STRING != 0
    }

    /// Returns the section offset of the name.
    ///
    /// Valid if `has_name()` returns true.
    fn name(&self) -> ResourceName {
        let offset = self.name_or_id.get(LE) & !pe::IMAGE_RESOURCE_NAME_IS_STRING;
        ResourceName { offset }
    }

    /// Returns the ID.
    ///
    /// Valid if `has_string_name()` returns false.
    fn id(&self) -> u16 {
        (self.name_or_id.get(LE) & 0x0000_FFFF) as u16
    }

    /// Returns the entry name
    pub fn name_or_id(&self) -> ResourceNameOrId {
        if self.has_name() {
            ResourceNameOrId::Name(self.name())
        } else {
            ResourceNameOrId::Id(self.id())
        }
    }

    /// Returns true if the entry is a subtable.
    pub fn is_table(&self) -> bool {
        self.offset_to_data_or_directory.get(LE) & pe::IMAGE_RESOURCE_DATA_IS_DIRECTORY != 0
    }

    /// Returns the section offset of the associated table or data.
    pub fn data_offset(&self) -> u32 {
        self.offset_to_data_or_directory.get(LE) & !pe::IMAGE_RESOURCE_DATA_IS_DIRECTORY
    }

    /// Returns the data associated to this directory entry.
    pub fn data<'data>(
        &self,
        section: ResourceDirectory<'data>,
    ) -> Result<ResourceDirectoryEntryData<'data>> {
        if self.is_table() {
            ResourceDirectoryTable::parse(section.data, self.data_offset())
                .map(ResourceDirectoryEntryData::Table)
        } else {
            section
                .data
                .read_at::<pe::ImageResourceDataEntry>(self.data_offset().into())
                .read_error("Invalid resource entry")
                .map(ResourceDirectoryEntryData::Data)
        }
    }
}

/// Data associated with a resource directory entry.
#[derive(Debug, Clone)]
pub enum ResourceDirectoryEntryData<'data> {
    /// A subtable entry.
    Table(ResourceDirectoryTable<'data>),
    /// A resource data entry.
    Data(&'data pe::ImageResourceDataEntry),
}

impl<'data> ResourceDirectoryEntryData<'data> {
    /// Converts to an option of table.
    ///
    /// Helper for iterator filtering.
    pub fn table(self) -> Option<ResourceDirectoryTable<'data>> {
        match self {
            Self::Table(dir) => Some(dir),
            _ => None,
        }
    }

    /// Converts to an option of data entry.
    ///
    /// Helper for iterator filtering.
    pub fn data(self) -> Option<&'data pe::ImageResourceDataEntry> {
        match self {
            Self::Data(rsc) => Some(rsc),
            _ => None,
        }
    }
}

/// A resource name.
#[derive(Debug, Clone, Copy)]
pub struct ResourceName {
    offset: u32,
}

impl ResourceName {
    /// Converts to a `String`.
    pub fn to_string_lossy(&self, directory: ResourceDirectory<'_>) -> Result<String> {
        let d = self.data(directory)?.iter().map(|c| c.get(LE));

        Ok(char::decode_utf16(d)
            .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect::<String>())
    }

    /// Returns the string unicode buffer.
    pub fn data<'data>(
        &self,
        directory: ResourceDirectory<'data>,
    ) -> Result<&'data [U16Bytes<LE>]> {
        let mut offset = u64::from(self.offset);
        let len = directory
            .data
            .read::<U16Bytes<LE>>(&mut offset)
            .read_error("Invalid resource name offset")?;
        directory
            .data
            .read_slice::<U16Bytes<LE>>(&mut offset, len.get(LE).into())
            .read_error("Invalid resource name length")
    }

    /// Returns the string buffer as raw bytes.
    pub fn raw_data<'data>(&self, directory: ResourceDirectory<'data>) -> Result<&'data [u8]> {
        self.data(directory).map(crate::pod::bytes_of_slice)
    }
}

/// A resource name or ID.
///
/// Can be either a string or a numeric ID.
#[derive(Debug)]
pub enum ResourceNameOrId {
    /// A resource name.
    Name(ResourceName),
    /// A resource ID.
    Id(u16),
}

impl ResourceNameOrId {
    /// Converts to an option of name.
    ///
    /// Helper for iterator filtering.
    pub fn name(self) -> Option<ResourceName> {
        match self {
            Self::Name(name) => Some(name),
            _ => None,
        }
    }

    /// Converts to an option of ID.
    ///
    /// Helper for iterator filtering.
    pub fn id(self) -> Option<u16> {
        match self {
            Self::Id(id) => Some(id),
            _ => None,
        }
    }
}
