//! Support for reading short import files.
//!
//! These are used by some Windows linkers as a more compact way to describe
//! dynamically imported symbols.

use crate::endian::LittleEndian as LE;
use crate::pe;
use crate::read::{
    Architecture, ByteString, Bytes, Error, ReadError, ReadRef, Result, SubArchitecture,
};

/// A Windows short form description of a symbol to import.
///
/// Used in Windows import libraries to provide a mapping from
/// a symbol name to a DLL export. This is not an object file.
///
/// This is a file that starts with [`pe::ImportObjectHeader`], and corresponds
/// to [`crate::FileKind::CoffImport`].
#[derive(Debug, Clone)]
pub struct ImportFile<'data> {
    header: &'data pe::ImportObjectHeader,
    kind: ImportType,
    dll: ByteString<'data>,
    symbol: ByteString<'data>,
    import: Option<ByteString<'data>>,
}

impl<'data> ImportFile<'data> {
    /// Parse it.
    pub fn parse<R: ReadRef<'data>>(data: R) -> Result<Self> {
        let mut offset = 0;
        let header = pe::ImportObjectHeader::parse(data, &mut offset)?;
        let data = header.parse_data(data, &mut offset)?;

        // Unmangles a name by removing a `?`, `@` or `_` prefix.
        fn strip_prefix(s: &[u8]) -> &[u8] {
            match s.split_first() {
                Some((b, rest)) if [b'?', b'@', b'_'].contains(b) => rest,
                _ => s,
            }
        }
        Ok(Self {
            header,
            dll: data.dll,
            symbol: data.symbol,
            kind: match header.import_type() {
                pe::IMPORT_OBJECT_CODE => ImportType::Code,
                pe::IMPORT_OBJECT_DATA => ImportType::Data,
                pe::IMPORT_OBJECT_CONST => ImportType::Const,
                _ => return Err(Error("Invalid COFF import library import type")),
            },
            import: match header.name_type() {
                pe::IMPORT_OBJECT_ORDINAL => None,
                pe::IMPORT_OBJECT_NAME => Some(data.symbol()),
                pe::IMPORT_OBJECT_NAME_NO_PREFIX => Some(strip_prefix(data.symbol())),
                pe::IMPORT_OBJECT_NAME_UNDECORATE => Some(
                    strip_prefix(data.symbol())
                        .split(|&b| b == b'@')
                        .next()
                        .unwrap(),
                ),
                pe::IMPORT_OBJECT_NAME_EXPORTAS => data.export(),
                _ => return Err(Error("Unknown COFF import library name type")),
            }
            .map(ByteString),
        })
    }

    /// Get the machine type.
    pub fn architecture(&self) -> Architecture {
        match self.header.machine.get(LE) {
            pe::IMAGE_FILE_MACHINE_ARMNT => Architecture::Arm,
            pe::IMAGE_FILE_MACHINE_ARM64 | pe::IMAGE_FILE_MACHINE_ARM64EC => Architecture::Aarch64,
            pe::IMAGE_FILE_MACHINE_I386 => Architecture::I386,
            pe::IMAGE_FILE_MACHINE_AMD64 => Architecture::X86_64,
            _ => Architecture::Unknown,
        }
    }

    /// Get the sub machine type, if available.
    pub fn sub_architecture(&self) -> Option<SubArchitecture> {
        match self.header.machine.get(LE) {
            pe::IMAGE_FILE_MACHINE_ARM64EC => Some(SubArchitecture::Arm64EC),
            _ => None,
        }
    }

    /// The public symbol name.
    pub fn symbol(&self) -> &'data [u8] {
        self.symbol.0
    }

    /// The name of the DLL to import the symbol from.
    pub fn dll(&self) -> &'data [u8] {
        self.dll.0
    }

    /// The name exported from the DLL.
    pub fn import(&self) -> ImportName<'data> {
        match self.import {
            Some(name) => ImportName::Name(name.0),
            None => ImportName::Ordinal(self.header.ordinal_or_hint.get(LE)),
        }
    }

    /// The type of import. Usually either a function or data.
    pub fn import_type(&self) -> ImportType {
        self.kind
    }
}

/// The name or ordinal to import from a DLL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportName<'data> {
    /// Import by ordinal. Ordinarily this is a 1-based index.
    Ordinal(u16),
    /// Import by name.
    Name(&'data [u8]),
}

/// The kind of import symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportType {
    /// An executable code symbol.
    Code,
    /// A data symbol.
    Data,
    /// A constant value.
    Const,
}

impl pe::ImportObjectHeader {
    /// Read the short import header.
    ///
    /// Also checks that the signature and version are valid.
    /// Directly following this header will be the string data.
    pub fn parse<'data, R: ReadRef<'data>>(data: R, offset: &mut u64) -> Result<&'data Self> {
        let header = data
            .read::<pe::ImportObjectHeader>(offset)
            .read_error("Invalid COFF import library header size")?;
        if header.sig1.get(LE) != 0 || header.sig2.get(LE) != pe::IMPORT_OBJECT_HDR_SIG2 {
            Err(Error("Invalid COFF import library header"))
        } else if header.version.get(LE) != 0 {
            Err(Error("Unknown COFF import library header version"))
        } else {
            Ok(header)
        }
    }

    /// Parse the data following the header.
    pub fn parse_data<'data, R: ReadRef<'data>>(
        &self,
        data: R,
        offset: &mut u64,
    ) -> Result<ImportObjectData<'data>> {
        let mut data = Bytes(
            data.read_bytes(offset, u64::from(self.size_of_data.get(LE)))
                .read_error("Invalid COFF import library data size")?,
        );
        let symbol = data
            .read_string()
            .map(ByteString)
            .read_error("Could not read COFF import library symbol name")?;
        let dll = data
            .read_string()
            .map(ByteString)
            .read_error("Could not read COFF import library DLL name")?;
        let export = if self.name_type() == pe::IMPORT_OBJECT_NAME_EXPORTAS {
            data.read_string()
                .map(ByteString)
                .map(Some)
                .read_error("Could not read COFF import library export name")?
        } else {
            None
        };
        Ok(ImportObjectData {
            symbol,
            dll,
            export,
        })
    }

    /// The type of import.
    ///
    /// This is one of the `IMPORT_OBJECT_*` constants.
    pub fn import_type(&self) -> u16 {
        self.name_type.get(LE) & pe::IMPORT_OBJECT_TYPE_MASK
    }

    /// The type of import name.
    ///
    /// This is one of the `IMPORT_OBJECT_*` constants.
    pub fn name_type(&self) -> u16 {
        (self.name_type.get(LE) >> pe::IMPORT_OBJECT_NAME_SHIFT) & pe::IMPORT_OBJECT_NAME_MASK
    }
}

/// The data following [`pe::ImportObjectHeader`].
#[derive(Debug, Clone)]
pub struct ImportObjectData<'data> {
    symbol: ByteString<'data>,
    dll: ByteString<'data>,
    export: Option<ByteString<'data>>,
}

impl<'data> ImportObjectData<'data> {
    /// The public symbol name.
    pub fn symbol(&self) -> &'data [u8] {
        self.symbol.0
    }

    /// The name of the DLL to import the symbol from.
    pub fn dll(&self) -> &'data [u8] {
        self.dll.0
    }

    /// The name exported from the DLL.
    ///
    /// This is only set if the name is not derived from the symbol name.
    pub fn export(&self) -> Option<&'data [u8]> {
        self.export.map(|export| export.0)
    }
}
