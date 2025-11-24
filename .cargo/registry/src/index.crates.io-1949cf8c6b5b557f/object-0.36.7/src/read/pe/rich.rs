//! PE rich header handling

use core::mem;

use crate::endian::{LittleEndian as LE, U32};
use crate::pe;
use crate::pod::bytes_of_slice;
use crate::read::{Bytes, ReadRef};

/// Parsed information about a Rich Header.
#[derive(Debug, Clone, Copy)]
pub struct RichHeaderInfo<'data> {
    /// The offset at which the rich header starts.
    pub offset: usize,
    /// The length (in bytes) of the rich header.
    ///
    /// This includes the payload, but also the 16-byte start sequence and the
    /// 8-byte final "Rich" and XOR key.
    pub length: usize,
    /// The XOR key used to mask the rich header.
    ///
    /// Unless the file has been tampered with, it should be equal to a checksum
    /// of the file header.
    pub xor_key: u32,
    masked_entries: &'data [pe::MaskedRichHeaderEntry],
}

/// A PE rich header entry after it has been unmasked.
///
/// See [`pe::MaskedRichHeaderEntry`].
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RichHeaderEntry {
    /// ID of the component.
    pub comp_id: u32,
    /// Number of times this component has been used when building this PE.
    pub count: u32,
}

impl<'data> RichHeaderInfo<'data> {
    /// Try to locate a rich header and its entries in the current PE file.
    pub fn parse<R: ReadRef<'data>>(data: R, nt_header_offset: u64) -> Option<Self> {
        // Locate the rich header, if any.
        // It ends with the "Rich" string and an XOR key, before the NT header.
        let data = data.read_bytes_at(0, nt_header_offset).map(Bytes).ok()?;
        let end_marker_offset = memmem(data.0, b"Rich", 4)?;
        let xor_key = *data.read_at::<U32<LE>>(end_marker_offset + 4).ok()?;

        // It starts at the masked "DanS" string and 3 masked zeroes.
        let masked_start_marker = U32::new(LE, 0x536e_6144 ^ xor_key.get(LE));
        let start_header = [masked_start_marker, xor_key, xor_key, xor_key];
        let start_sequence = bytes_of_slice(&start_header);
        let start_marker_offset = memmem(&data.0[..end_marker_offset], start_sequence, 4)?;

        // Extract the items between the markers.
        let items_offset = start_marker_offset + start_sequence.len();
        let items_len = end_marker_offset - items_offset;
        let item_count = items_len / mem::size_of::<pe::MaskedRichHeaderEntry>();
        let items = data.read_slice_at(items_offset, item_count).ok()?;
        Some(RichHeaderInfo {
            offset: start_marker_offset,
            // Includes "Rich" marker and the XOR key.
            length: end_marker_offset - start_marker_offset + 8,
            xor_key: xor_key.get(LE),
            masked_entries: items,
        })
    }

    /// Returns an iterator over the unmasked entries.
    pub fn unmasked_entries(&self) -> impl Iterator<Item = RichHeaderEntry> + 'data {
        let xor_key = self.xor_key;
        self.masked_entries
            .iter()
            .map(move |entry| RichHeaderEntry {
                comp_id: entry.masked_comp_id.get(LE) ^ xor_key,
                count: entry.masked_count.get(LE) ^ xor_key,
            })
    }
}

/// Find the offset of the first occurrence of needle in the data.
///
/// The offset must have the given alignment.
fn memmem(data: &[u8], needle: &[u8], align: usize) -> Option<usize> {
    let mut offset = 0;
    loop {
        if data.get(offset..)?.get(..needle.len())? == needle {
            return Some(offset);
        }
        offset += align;
    }
}
