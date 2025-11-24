use core::fmt::Debug;
use core::mem;

use crate::endian::{LittleEndian as LE, U16Bytes};
use crate::pe;
use crate::pod::Pod;
use crate::read::{Bytes, ReadError, Result};

use super::ImageNtHeaders;

/// Information for parsing a PE import table.
///
/// Returned by [`DataDirectories::import_table`](super::DataDirectories::import_table).
#[derive(Debug, Clone)]
pub struct ImportTable<'data> {
    section_data: Bytes<'data>,
    section_address: u32,
    import_address: u32,
}

impl<'data> ImportTable<'data> {
    /// Create a new import table parser.
    ///
    /// The import descriptors start at `import_address`.
    /// The size declared in the `IMAGE_DIRECTORY_ENTRY_IMPORT` data directory is
    /// ignored by the Windows loader, and so descriptors will be parsed until a null entry.
    ///
    /// `section_data` should be from the section containing `import_address`, and
    /// `section_address` should be the address of that section. Pointers within the
    /// descriptors and thunks may point to anywhere within the section data.
    pub fn new(section_data: &'data [u8], section_address: u32, import_address: u32) -> Self {
        ImportTable {
            section_data: Bytes(section_data),
            section_address,
            import_address,
        }
    }

    /// Return an iterator for the import descriptors.
    pub fn descriptors(&self) -> Result<ImportDescriptorIterator<'data>> {
        let offset = self.import_address.wrapping_sub(self.section_address);
        let mut data = self.section_data;
        data.skip(offset as usize)
            .read_error("Invalid PE import descriptor address")?;
        Ok(ImportDescriptorIterator { data, null: false })
    }

    /// Return a library name given its address.
    ///
    /// This address may be from [`pe::ImageImportDescriptor::name`].
    pub fn name(&self, address: u32) -> Result<&'data [u8]> {
        self.section_data
            .read_string_at(address.wrapping_sub(self.section_address) as usize)
            .read_error("Invalid PE import descriptor name")
    }

    /// Return a list of thunks given its address.
    ///
    /// This address may be from [`pe::ImageImportDescriptor::original_first_thunk`]
    /// or [`pe::ImageImportDescriptor::first_thunk`].
    pub fn thunks(&self, address: u32) -> Result<ImportThunkList<'data>> {
        let offset = address.wrapping_sub(self.section_address);
        let mut data = self.section_data;
        data.skip(offset as usize)
            .read_error("Invalid PE import thunk table address")?;
        Ok(ImportThunkList { data })
    }

    /// Parse a thunk.
    pub fn import<Pe: ImageNtHeaders>(&self, thunk: Pe::ImageThunkData) -> Result<Import<'data>> {
        if thunk.is_ordinal() {
            Ok(Import::Ordinal(thunk.ordinal()))
        } else {
            let (hint, name) = self.hint_name(thunk.address())?;
            Ok(Import::Name(hint, name))
        }
    }

    /// Return the hint and name at the given address.
    ///
    /// This address may be from [`pe::ImageThunkData32`] or [`pe::ImageThunkData64`].
    ///
    /// The hint is an index into the export name pointer table in the target library.
    pub fn hint_name(&self, address: u32) -> Result<(u16, &'data [u8])> {
        let offset = address.wrapping_sub(self.section_address);
        let mut data = self.section_data;
        data.skip(offset as usize)
            .read_error("Invalid PE import thunk address")?;
        let hint = data
            .read::<U16Bytes<LE>>()
            .read_error("Missing PE import thunk hint")?
            .get(LE);
        let name = data
            .read_string()
            .read_error("Missing PE import thunk name")?;
        Ok((hint, name))
    }
}

/// A fallible iterator for the descriptors in the import data directory.
#[derive(Debug, Clone)]
pub struct ImportDescriptorIterator<'data> {
    data: Bytes<'data>,
    null: bool,
}

impl<'data> ImportDescriptorIterator<'data> {
    /// Return the next descriptor.
    ///
    /// Returns `Ok(None)` when a null descriptor is found.
    pub fn next(&mut self) -> Result<Option<&'data pe::ImageImportDescriptor>> {
        if self.null {
            return Ok(None);
        }
        let result = self
            .data
            .read::<pe::ImageImportDescriptor>()
            .read_error("Missing PE null import descriptor");
        match result {
            Ok(import_desc) => {
                if import_desc.is_null() {
                    self.null = true;
                    Ok(None)
                } else {
                    Ok(Some(import_desc))
                }
            }
            Err(e) => {
                self.null = true;
                Err(e)
            }
        }
    }
}

impl<'data> Iterator for ImportDescriptorIterator<'data> {
    type Item = Result<&'data pe::ImageImportDescriptor>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// A list of import thunks.
///
/// These may be in the import lookup table, or the import address table.
#[derive(Debug, Clone)]
pub struct ImportThunkList<'data> {
    data: Bytes<'data>,
}

impl<'data> ImportThunkList<'data> {
    /// Get the thunk at the given index.
    pub fn get<Pe: ImageNtHeaders>(&self, index: usize) -> Result<Pe::ImageThunkData> {
        let thunk = self
            .data
            .read_at(index * mem::size_of::<Pe::ImageThunkData>())
            .read_error("Invalid PE import thunk index")?;
        Ok(*thunk)
    }

    /// Return the first thunk in the list, and update `self` to point after it.
    ///
    /// Returns `Ok(None)` when a null thunk is found.
    pub fn next<Pe: ImageNtHeaders>(&mut self) -> Result<Option<Pe::ImageThunkData>> {
        let thunk = self
            .data
            .read::<Pe::ImageThunkData>()
            .read_error("Missing PE null import thunk")?;
        if thunk.address() == 0 {
            Ok(None)
        } else {
            Ok(Some(*thunk))
        }
    }
}

/// A parsed import thunk.
#[derive(Debug, Clone, Copy)]
pub enum Import<'data> {
    /// Import by ordinal.
    Ordinal(u16),
    /// Import by name.
    ///
    /// Includes a hint for the index into the export name pointer table in the target library.
    Name(u16, &'data [u8]),
}

/// A trait for generic access to [`pe::ImageThunkData32`] and [`pe::ImageThunkData64`].
#[allow(missing_docs)]
pub trait ImageThunkData: Debug + Pod {
    /// Return the raw thunk value.
    fn raw(self) -> u64;

    /// Returns true if the ordinal flag is set.
    fn is_ordinal(self) -> bool;

    /// Return the ordinal portion of the thunk.
    ///
    /// Does not check the ordinal flag.
    fn ordinal(self) -> u16;

    /// Return the RVA portion of the thunk.
    ///
    /// Does not check the ordinal flag.
    fn address(self) -> u32;
}

impl ImageThunkData for pe::ImageThunkData64 {
    fn raw(self) -> u64 {
        self.0.get(LE)
    }

    fn is_ordinal(self) -> bool {
        self.0.get(LE) & pe::IMAGE_ORDINAL_FLAG64 != 0
    }

    fn ordinal(self) -> u16 {
        self.0.get(LE) as u16
    }

    fn address(self) -> u32 {
        self.0.get(LE) as u32 & 0x7fff_ffff
    }
}

impl ImageThunkData for pe::ImageThunkData32 {
    fn raw(self) -> u64 {
        self.0.get(LE).into()
    }

    fn is_ordinal(self) -> bool {
        self.0.get(LE) & pe::IMAGE_ORDINAL_FLAG32 != 0
    }

    fn ordinal(self) -> u16 {
        self.0.get(LE) as u16
    }

    fn address(self) -> u32 {
        self.0.get(LE) & 0x7fff_ffff
    }
}

/// Information for parsing a PE delay-load import table.
///
/// Returned by
/// [`DataDirectories::delay_load_import_table`](super::DataDirectories::delay_load_import_table).
#[derive(Debug, Clone)]
pub struct DelayLoadImportTable<'data> {
    section_data: Bytes<'data>,
    section_address: u32,
    import_address: u32,
}

impl<'data> DelayLoadImportTable<'data> {
    /// Create a new delay load import table parser.
    ///
    /// The import descriptors start at `import_address`.
    /// This table works in the same way the import table does: descriptors will be
    /// parsed until a null entry.
    ///
    /// `section_data` should be from the section containing `import_address`, and
    /// `section_address` should be the address of that section. Pointers within the
    /// descriptors and thunks may point to anywhere within the section data.
    pub fn new(section_data: &'data [u8], section_address: u32, import_address: u32) -> Self {
        DelayLoadImportTable {
            section_data: Bytes(section_data),
            section_address,
            import_address,
        }
    }

    /// Return an iterator for the import descriptors.
    pub fn descriptors(&self) -> Result<DelayLoadDescriptorIterator<'data>> {
        let offset = self.import_address.wrapping_sub(self.section_address);
        let mut data = self.section_data;
        data.skip(offset as usize)
            .read_error("Invalid PE delay-load import descriptor address")?;
        Ok(DelayLoadDescriptorIterator { data, null: false })
    }

    /// Return a library name given its address.
    ///
    /// This address may be from [`pe::ImageDelayloadDescriptor::dll_name_rva`].
    pub fn name(&self, address: u32) -> Result<&'data [u8]> {
        self.section_data
            .read_string_at(address.wrapping_sub(self.section_address) as usize)
            .read_error("Invalid PE import descriptor name")
    }

    /// Return a list of thunks given its address.
    ///
    /// This address may be from the INT, i.e. from
    /// [`pe::ImageDelayloadDescriptor::import_name_table_rva`].
    ///
    /// Please note that others RVA values from [`pe::ImageDelayloadDescriptor`] are used
    /// by the delay loader at runtime to store values, and thus do not point inside the same
    /// section as the INT. Calling this function on those addresses will fail.
    pub fn thunks(&self, address: u32) -> Result<ImportThunkList<'data>> {
        let offset = address.wrapping_sub(self.section_address);
        let mut data = self.section_data;
        data.skip(offset as usize)
            .read_error("Invalid PE delay load import thunk table address")?;
        Ok(ImportThunkList { data })
    }

    /// Parse a thunk.
    pub fn import<Pe: ImageNtHeaders>(&self, thunk: Pe::ImageThunkData) -> Result<Import<'data>> {
        if thunk.is_ordinal() {
            Ok(Import::Ordinal(thunk.ordinal()))
        } else {
            let (hint, name) = self.hint_name(thunk.address())?;
            Ok(Import::Name(hint, name))
        }
    }

    /// Return the hint and name at the given address.
    ///
    /// This address may be from [`pe::ImageThunkData32`] or [`pe::ImageThunkData64`].
    ///
    /// The hint is an index into the export name pointer table in the target library.
    pub fn hint_name(&self, address: u32) -> Result<(u16, &'data [u8])> {
        let offset = address.wrapping_sub(self.section_address);
        let mut data = self.section_data;
        data.skip(offset as usize)
            .read_error("Invalid PE delay load import thunk address")?;
        let hint = data
            .read::<U16Bytes<LE>>()
            .read_error("Missing PE delay load import thunk hint")?
            .get(LE);
        let name = data
            .read_string()
            .read_error("Missing PE delay load import thunk name")?;
        Ok((hint, name))
    }
}

/// A fallible iterator for the descriptors in the delay-load data directory.
#[derive(Debug, Clone)]
pub struct DelayLoadDescriptorIterator<'data> {
    data: Bytes<'data>,
    null: bool,
}

impl<'data> DelayLoadDescriptorIterator<'data> {
    /// Return the next descriptor.
    ///
    /// Returns `Ok(None)` when a null descriptor is found.
    pub fn next(&mut self) -> Result<Option<&'data pe::ImageDelayloadDescriptor>> {
        if self.null {
            return Ok(None);
        }
        let result = self
            .data
            .read::<pe::ImageDelayloadDescriptor>()
            .read_error("Missing PE null delay-load import descriptor");
        match result {
            Ok(import_desc) => {
                if import_desc.is_null() {
                    self.null = true;
                    Ok(None)
                } else {
                    Ok(Some(import_desc))
                }
            }
            Err(e) => {
                self.null = true;
                Err(e)
            }
        }
    }
}

impl<'data> Iterator for DelayLoadDescriptorIterator<'data> {
    type Item = Result<&'data pe::ImageDelayloadDescriptor>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}
