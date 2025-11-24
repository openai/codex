use crate::common::{Encoding, Format};
use crate::read::{Error, Reader, Result};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ListsHeader {
    encoding: Encoding,
    #[allow(dead_code)]
    offset_entry_count: u32,
}

impl Default for ListsHeader {
    fn default() -> Self {
        ListsHeader {
            encoding: Encoding {
                format: Format::Dwarf32,
                version: 5,
                address_size: 0,
            },
            offset_entry_count: 0,
        }
    }
}

impl ListsHeader {
    /// Return the serialized size of the table header.
    #[allow(dead_code)]
    #[inline]
    fn size(self) -> u8 {
        // initial_length + version + address_size + segment_selector_size + offset_entry_count
        ListsHeader::size_for_encoding(self.encoding)
    }

    /// Return the serialized size of the table header.
    #[inline]
    pub(crate) fn size_for_encoding(encoding: Encoding) -> u8 {
        // initial_length + version + address_size + segment_selector_size + offset_entry_count
        encoding.format.initial_length_size() + 2 + 1 + 1 + 4
    }
}

// TODO: add an iterator over headers in the appropriate sections section
#[allow(dead_code)]
fn parse_header<R: Reader>(input: &mut R) -> Result<ListsHeader> {
    let (length, format) = input.read_initial_length()?;
    input.truncate(length)?;

    let version = input.read_u16()?;
    if version != 5 {
        return Err(Error::UnknownVersion(u64::from(version)));
    }

    let address_size = input.read_address_size()?;
    let segment_selector_size = input.read_u8()?;
    if segment_selector_size != 0 {
        return Err(Error::UnsupportedSegmentSize);
    }
    let offset_entry_count = input.read_u32()?;

    let encoding = Encoding {
        format,
        version,
        address_size,
    };
    Ok(ListsHeader {
        encoding,
        offset_entry_count,
    })
}
