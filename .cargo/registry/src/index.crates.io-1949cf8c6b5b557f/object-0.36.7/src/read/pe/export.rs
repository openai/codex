use alloc::vec::Vec;
use core::fmt::Debug;

use crate::endian::{LittleEndian as LE, U16Bytes, U32Bytes};
use crate::pe;
use crate::read::{ByteString, Bytes, Error, ReadError, ReadRef, Result};

/// Where an export is pointing to.
#[derive(Clone, Copy)]
pub enum ExportTarget<'data> {
    /// The address of the export, relative to the image base.
    Address(u32),
    /// Forwarded to an export ordinal in another DLL.
    ///
    /// This gives the name of the DLL, and the ordinal.
    ForwardByOrdinal(&'data [u8], u32),
    /// Forwarded to an export name in another DLL.
    ///
    /// This gives the name of the DLL, and the export name.
    ForwardByName(&'data [u8], &'data [u8]),
}

impl<'data> ExportTarget<'data> {
    /// Returns true if the target is an address.
    pub fn is_address(&self) -> bool {
        match self {
            ExportTarget::Address(_) => true,
            _ => false,
        }
    }

    /// Returns true if the export is forwarded to another DLL.
    pub fn is_forward(&self) -> bool {
        !self.is_address()
    }
}

/// An export from a PE file.
///
/// There are multiple kinds of PE exports (with or without a name, and local or forwarded).
#[derive(Clone, Copy)]
pub struct Export<'data> {
    /// The ordinal of the export.
    ///
    /// These are sequential, starting at a base specified in the DLL.
    pub ordinal: u32,
    /// The name of the export, if known.
    pub name: Option<&'data [u8]>,
    /// The target of this export.
    pub target: ExportTarget<'data>,
}

impl<'a> Debug for Export<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::result::Result<(), core::fmt::Error> {
        f.debug_struct("Export")
            .field("ordinal", &self.ordinal)
            .field("name", &self.name.map(ByteString))
            .field("target", &self.target)
            .finish()
    }
}

impl<'a> Debug for ExportTarget<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::result::Result<(), core::fmt::Error> {
        match self {
            ExportTarget::Address(address) => write!(f, "Address({:#x})", address),
            ExportTarget::ForwardByOrdinal(library, ordinal) => write!(
                f,
                "ForwardByOrdinal({:?}.#{})",
                ByteString(library),
                ordinal
            ),
            ExportTarget::ForwardByName(library, name) => write!(
                f,
                "ForwardByName({:?}.{:?})",
                ByteString(library),
                ByteString(name)
            ),
        }
    }
}

/// A partially parsed PE export table.
///
/// Returned by [`DataDirectories::export_table`](super::DataDirectories::export_table).
#[derive(Debug, Clone)]
pub struct ExportTable<'data> {
    data: Bytes<'data>,
    virtual_address: u32,
    directory: &'data pe::ImageExportDirectory,
    addresses: &'data [U32Bytes<LE>],
    names: &'data [U32Bytes<LE>],
    name_ordinals: &'data [U16Bytes<LE>],
}

impl<'data> ExportTable<'data> {
    /// Parse the export table given its section data and address.
    pub fn parse(data: &'data [u8], virtual_address: u32) -> Result<Self> {
        let directory = Self::parse_directory(data)?;
        let data = Bytes(data);

        let mut addresses = &[][..];
        let address_of_functions = directory.address_of_functions.get(LE);
        if address_of_functions != 0 {
            addresses = data
                .read_slice_at::<U32Bytes<_>>(
                    address_of_functions.wrapping_sub(virtual_address) as usize,
                    directory.number_of_functions.get(LE) as usize,
                )
                .read_error("Invalid PE export address table")?;
        }

        let mut names = &[][..];
        let mut name_ordinals = &[][..];
        let address_of_names = directory.address_of_names.get(LE);
        let address_of_name_ordinals = directory.address_of_name_ordinals.get(LE);
        if address_of_names != 0 {
            if address_of_name_ordinals == 0 {
                return Err(Error("Missing PE export ordinal table"));
            }

            let number = directory.number_of_names.get(LE) as usize;
            names = data
                .read_slice_at::<U32Bytes<_>>(
                    address_of_names.wrapping_sub(virtual_address) as usize,
                    number,
                )
                .read_error("Invalid PE export name pointer table")?;
            name_ordinals = data
                .read_slice_at::<U16Bytes<_>>(
                    address_of_name_ordinals.wrapping_sub(virtual_address) as usize,
                    number,
                )
                .read_error("Invalid PE export ordinal table")?;
        }

        Ok(ExportTable {
            data,
            virtual_address,
            directory,
            addresses,
            names,
            name_ordinals,
        })
    }

    /// Parse the export directory given its section data.
    pub fn parse_directory(data: &'data [u8]) -> Result<&'data pe::ImageExportDirectory> {
        data.read_at::<pe::ImageExportDirectory>(0)
            .read_error("Invalid PE export dir size")
    }

    /// Returns the header of the export table.
    pub fn directory(&self) -> &'data pe::ImageExportDirectory {
        self.directory
    }

    /// Returns the base value of ordinals.
    ///
    /// Adding this to an address index will give an ordinal.
    pub fn ordinal_base(&self) -> u32 {
        self.directory.base.get(LE)
    }

    /// Returns the unparsed address table.
    ///
    /// An address table entry may be a local address, or the address of a forwarded export entry.
    /// See [`Self::is_forward`] and [`Self::target_from_address`].
    pub fn addresses(&self) -> &'data [U32Bytes<LE>] {
        self.addresses
    }

    /// Returns the unparsed name pointer table.
    ///
    /// A name pointer table entry can be used with [`Self::name_from_pointer`].
    pub fn name_pointers(&self) -> &'data [U32Bytes<LE>] {
        self.names
    }

    /// Returns the unparsed ordinal table.
    ///
    /// An ordinal table entry is a 0-based index into the address table.
    /// See [`Self::address_by_index`] and [`Self::target_by_index`].
    pub fn name_ordinals(&self) -> &'data [U16Bytes<LE>] {
        self.name_ordinals
    }

    /// Returns an iterator for the entries in the name pointer table and ordinal table.
    ///
    /// A name pointer table entry can be used with [`Self::name_from_pointer`].
    ///
    /// An ordinal table entry is a 0-based index into the address table.
    /// See [`Self::address_by_index`] and [`Self::target_by_index`].
    pub fn name_iter(&self) -> impl Iterator<Item = (u32, u16)> + 'data {
        self.names
            .iter()
            .map(|x| x.get(LE))
            .zip(self.name_ordinals.iter().map(|x| x.get(LE)))
    }

    /// Returns the export address table entry at the given address index.
    ///
    /// This may be a local address, or the address of a forwarded export entry.
    /// See [`Self::is_forward`] and [`Self::target_from_address`].
    ///
    /// `index` is a 0-based index into the export address table.
    pub fn address_by_index(&self, index: u32) -> Result<u32> {
        Ok(self
            .addresses
            .get(index as usize)
            .read_error("Invalid PE export address index")?
            .get(LE))
    }

    /// Returns the export address table entry at the given ordinal.
    ///
    /// This may be a local address, or the address of a forwarded export entry.
    /// See [`Self::is_forward`] and [`Self::target_from_address`].
    pub fn address_by_ordinal(&self, ordinal: u32) -> Result<u32> {
        self.address_by_index(ordinal.wrapping_sub(self.ordinal_base()))
    }

    /// Returns the target of the export at the given address index.
    ///
    /// `index` is a 0-based index into the export address table.
    pub fn target_by_index(&self, index: u32) -> Result<ExportTarget<'data>> {
        self.target_from_address(self.address_by_index(index)?)
    }

    /// Returns the target of the export at the given ordinal.
    pub fn target_by_ordinal(&self, ordinal: u32) -> Result<ExportTarget<'data>> {
        self.target_from_address(self.address_by_ordinal(ordinal)?)
    }

    /// Convert an export address table entry into a target.
    pub fn target_from_address(&self, address: u32) -> Result<ExportTarget<'data>> {
        Ok(if let Some(forward) = self.forward_string(address)? {
            let i = forward
                .iter()
                .position(|x| *x == b'.')
                .read_error("Missing PE forwarded export separator")?;
            let library = &forward[..i];
            match &forward[i + 1..] {
                [b'#', digits @ ..] => {
                    let ordinal =
                        parse_ordinal(digits).read_error("Invalid PE forwarded export ordinal")?;
                    ExportTarget::ForwardByOrdinal(library, ordinal)
                }
                [] => {
                    return Err(Error("Missing PE forwarded export name"));
                }
                name => ExportTarget::ForwardByName(library, name),
            }
        } else {
            ExportTarget::Address(address)
        })
    }

    fn forward_offset(&self, address: u32) -> Option<usize> {
        let offset = address.wrapping_sub(self.virtual_address) as usize;
        if offset < self.data.len() {
            Some(offset)
        } else {
            None
        }
    }

    /// Return true if the export address table entry is a forward.
    pub fn is_forward(&self, address: u32) -> bool {
        self.forward_offset(address).is_some()
    }

    /// Return the forward string if the export address table entry is a forward.
    pub fn forward_string(&self, address: u32) -> Result<Option<&'data [u8]>> {
        if let Some(offset) = self.forward_offset(address) {
            self.data
                .read_string_at(offset)
                .read_error("Invalid PE forwarded export address")
                .map(Some)
        } else {
            Ok(None)
        }
    }

    /// Convert an export name pointer table entry into a name.
    pub fn name_from_pointer(&self, name_pointer: u32) -> Result<&'data [u8]> {
        let offset = name_pointer.wrapping_sub(self.virtual_address);
        self.data
            .read_string_at(offset as usize)
            .read_error("Invalid PE export name pointer")
    }

    /// Returns the parsed exports in this table.
    pub fn exports(&self) -> Result<Vec<Export<'data>>> {
        // First, let's list all exports.
        let mut exports = Vec::new();
        let ordinal_base = self.ordinal_base();
        for (i, address) in self.addresses.iter().enumerate() {
            // Convert from an array index to an ordinal.
            let ordinal = ordinal_base.wrapping_add(i as u32);
            let target = self.target_from_address(address.get(LE))?;
            exports.push(Export {
                ordinal,
                target,
                // Might be populated later.
                name: None,
            });
        }

        // Now, check whether some (or all) of them have an associated name.
        // `ordinal_index` is a 0-based index into `addresses`.
        for (name_pointer, ordinal_index) in self.name_iter() {
            let name = self.name_from_pointer(name_pointer)?;
            exports
                .get_mut(ordinal_index as usize)
                .read_error("Invalid PE export ordinal")?
                .name = Some(name);
        }

        Ok(exports)
    }
}

fn parse_ordinal(digits: &[u8]) -> Option<u32> {
    if digits.is_empty() {
        return None;
    }
    let mut result: u32 = 0;
    for &c in digits {
        let x = (c as char).to_digit(10)?;
        result = result.checked_mul(10)?.checked_add(x)?;
    }
    Some(result)
}
