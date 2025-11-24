use core::slice;

use crate::endian::{LittleEndian as LE, U16};
use crate::pe;
use crate::read::{Bytes, Error, ReadError, Result};

/// An iterator over the relocation blocks in the `.reloc` section of a PE file.
///
/// Returned by [`DataDirectories::relocation_blocks`](super::DataDirectories::relocation_blocks).
#[derive(Debug, Default, Clone, Copy)]
pub struct RelocationBlockIterator<'data> {
    data: Bytes<'data>,
}

impl<'data> RelocationBlockIterator<'data> {
    /// Construct a new iterator from the data of the `.reloc` section.
    pub fn new(data: &'data [u8]) -> Self {
        RelocationBlockIterator { data: Bytes(data) }
    }

    /// Read the next relocation page.
    pub fn next(&mut self) -> Result<Option<RelocationIterator<'data>>> {
        if self.data.is_empty() {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.data = Bytes(&[]);
        }
        result
    }

    fn parse(&mut self) -> Result<RelocationIterator<'data>> {
        let header = self
            .data
            .read::<pe::ImageBaseRelocation>()
            .read_error("Invalid PE reloc section size")?;
        let virtual_address = header.virtual_address.get(LE);
        let size = header.size_of_block.get(LE);
        if size <= 8 || size & 3 != 0 {
            return Err(Error("Invalid PE reloc block size"));
        }
        let count = (size - 8) / 2;
        let relocs = self
            .data
            .read_slice::<U16<LE>>(count as usize)
            .read_error("Invalid PE reloc block size")?
            .iter();
        Ok(RelocationIterator {
            virtual_address,
            size,
            relocs,
        })
    }
}

impl<'data> Iterator for RelocationBlockIterator<'data> {
    type Item = Result<RelocationIterator<'data>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// An iterator of the relocations in a block in the `.reloc` section of a PE file.
#[derive(Debug, Clone)]
pub struct RelocationIterator<'data> {
    virtual_address: u32,
    size: u32,
    relocs: slice::Iter<'data, U16<LE>>,
}

impl<'data> RelocationIterator<'data> {
    /// Return the virtual address of the page that this block of relocations applies to.
    pub fn virtual_address(&self) -> u32 {
        self.virtual_address
    }

    /// Return the size in bytes of this block of relocations.
    pub fn size(&self) -> u32 {
        self.size
    }
}

impl<'data> Iterator for RelocationIterator<'data> {
    type Item = Relocation;

    fn next(&mut self) -> Option<Relocation> {
        loop {
            let reloc = self.relocs.next()?.get(LE);
            if reloc != 0 {
                return Some(Relocation {
                    virtual_address: self.virtual_address.wrapping_add((reloc & 0xfff) as u32),
                    typ: reloc >> 12,
                });
            }
        }
    }
}

/// A relocation in the `.reloc` section of a PE file.
#[derive(Debug, Default, Clone, Copy)]
pub struct Relocation {
    /// The virtual address of the relocation.
    pub virtual_address: u32,
    /// One of the `pe::IMAGE_REL_BASED_*` constants.
    pub typ: u16,
}
