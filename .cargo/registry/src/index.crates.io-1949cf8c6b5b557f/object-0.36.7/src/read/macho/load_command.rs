use core::marker::PhantomData;
use core::mem;

use crate::endian::Endian;
use crate::macho;
use crate::pod::Pod;
use crate::read::macho::{MachHeader, SymbolTable};
use crate::read::{Bytes, Error, ReadError, ReadRef, Result, StringTable};

/// An iterator for the load commands from a [`MachHeader`].
#[derive(Debug, Default, Clone, Copy)]
pub struct LoadCommandIterator<'data, E: Endian> {
    endian: E,
    data: Bytes<'data>,
    ncmds: u32,
}

impl<'data, E: Endian> LoadCommandIterator<'data, E> {
    pub(super) fn new(endian: E, data: &'data [u8], ncmds: u32) -> Self {
        LoadCommandIterator {
            endian,
            data: Bytes(data),
            ncmds,
        }
    }

    /// Return the next load command.
    pub fn next(&mut self) -> Result<Option<LoadCommandData<'data, E>>> {
        if self.ncmds == 0 {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.ncmds = 0;
        } else {
            self.ncmds -= 1;
        }
        result
    }

    fn parse(&mut self) -> Result<LoadCommandData<'data, E>> {
        let header = self
            .data
            .read_at::<macho::LoadCommand<E>>(0)
            .read_error("Invalid Mach-O load command header")?;
        let cmd = header.cmd.get(self.endian);
        let cmdsize = header.cmdsize.get(self.endian) as usize;
        if cmdsize < mem::size_of::<macho::LoadCommand<E>>() {
            return Err(Error("Invalid Mach-O load command size"));
        }
        let data = self
            .data
            .read_bytes(cmdsize)
            .read_error("Invalid Mach-O load command size")?;
        Ok(LoadCommandData {
            cmd,
            data,
            marker: Default::default(),
        })
    }
}

impl<'data, E: Endian> Iterator for LoadCommandIterator<'data, E> {
    type Item = Result<LoadCommandData<'data, E>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// The data for a [`macho::LoadCommand`].
#[derive(Debug, Clone, Copy)]
pub struct LoadCommandData<'data, E: Endian> {
    cmd: u32,
    // Includes the header.
    data: Bytes<'data>,
    marker: PhantomData<E>,
}

impl<'data, E: Endian> LoadCommandData<'data, E> {
    /// Return the `cmd` field of the [`macho::LoadCommand`].
    ///
    /// This is one of the `LC_` constants.
    pub fn cmd(&self) -> u32 {
        self.cmd
    }

    /// Return the `cmdsize` field of the [`macho::LoadCommand`].
    pub fn cmdsize(&self) -> u32 {
        self.data.len() as u32
    }

    /// Parse the data as the given type.
    #[inline]
    pub fn data<T: Pod>(&self) -> Result<&'data T> {
        self.data
            .read_at(0)
            .read_error("Invalid Mach-O command size")
    }

    /// Raw bytes of this [`macho::LoadCommand`] structure.
    pub fn raw_data(&self) -> &'data [u8] {
        self.data.0
    }

    /// Parse a load command string value.
    ///
    /// Strings used by load commands are specified by offsets that are
    /// relative to the load command header.
    pub fn string(&self, endian: E, s: macho::LcStr<E>) -> Result<&'data [u8]> {
        self.data
            .read_string_at(s.offset.get(endian) as usize)
            .read_error("Invalid load command string offset")
    }

    /// Parse the command data according to the `cmd` field.
    pub fn variant(&self) -> Result<LoadCommandVariant<'data, E>> {
        Ok(match self.cmd {
            macho::LC_SEGMENT => {
                let mut data = self.data;
                let segment = data.read().read_error("Invalid Mach-O command size")?;
                LoadCommandVariant::Segment32(segment, data.0)
            }
            macho::LC_SYMTAB => LoadCommandVariant::Symtab(self.data()?),
            macho::LC_THREAD | macho::LC_UNIXTHREAD => {
                let mut data = self.data;
                let thread = data.read().read_error("Invalid Mach-O command size")?;
                LoadCommandVariant::Thread(thread, data.0)
            }
            macho::LC_DYSYMTAB => LoadCommandVariant::Dysymtab(self.data()?),
            macho::LC_LOAD_DYLIB
            | macho::LC_LOAD_WEAK_DYLIB
            | macho::LC_REEXPORT_DYLIB
            | macho::LC_LAZY_LOAD_DYLIB
            | macho::LC_LOAD_UPWARD_DYLIB => LoadCommandVariant::Dylib(self.data()?),
            macho::LC_ID_DYLIB => LoadCommandVariant::IdDylib(self.data()?),
            macho::LC_LOAD_DYLINKER => LoadCommandVariant::LoadDylinker(self.data()?),
            macho::LC_ID_DYLINKER => LoadCommandVariant::IdDylinker(self.data()?),
            macho::LC_PREBOUND_DYLIB => LoadCommandVariant::PreboundDylib(self.data()?),
            macho::LC_ROUTINES => LoadCommandVariant::Routines32(self.data()?),
            macho::LC_SUB_FRAMEWORK => LoadCommandVariant::SubFramework(self.data()?),
            macho::LC_SUB_UMBRELLA => LoadCommandVariant::SubUmbrella(self.data()?),
            macho::LC_SUB_CLIENT => LoadCommandVariant::SubClient(self.data()?),
            macho::LC_SUB_LIBRARY => LoadCommandVariant::SubLibrary(self.data()?),
            macho::LC_TWOLEVEL_HINTS => LoadCommandVariant::TwolevelHints(self.data()?),
            macho::LC_PREBIND_CKSUM => LoadCommandVariant::PrebindCksum(self.data()?),
            macho::LC_SEGMENT_64 => {
                let mut data = self.data;
                let segment = data.read().read_error("Invalid Mach-O command size")?;
                LoadCommandVariant::Segment64(segment, data.0)
            }
            macho::LC_ROUTINES_64 => LoadCommandVariant::Routines64(self.data()?),
            macho::LC_UUID => LoadCommandVariant::Uuid(self.data()?),
            macho::LC_RPATH => LoadCommandVariant::Rpath(self.data()?),
            macho::LC_CODE_SIGNATURE
            | macho::LC_SEGMENT_SPLIT_INFO
            | macho::LC_FUNCTION_STARTS
            | macho::LC_DATA_IN_CODE
            | macho::LC_DYLIB_CODE_SIGN_DRS
            | macho::LC_LINKER_OPTIMIZATION_HINT
            | macho::LC_DYLD_EXPORTS_TRIE
            | macho::LC_DYLD_CHAINED_FIXUPS => LoadCommandVariant::LinkeditData(self.data()?),
            macho::LC_ENCRYPTION_INFO => LoadCommandVariant::EncryptionInfo32(self.data()?),
            macho::LC_DYLD_INFO | macho::LC_DYLD_INFO_ONLY => {
                LoadCommandVariant::DyldInfo(self.data()?)
            }
            macho::LC_VERSION_MIN_MACOSX
            | macho::LC_VERSION_MIN_IPHONEOS
            | macho::LC_VERSION_MIN_TVOS
            | macho::LC_VERSION_MIN_WATCHOS => LoadCommandVariant::VersionMin(self.data()?),
            macho::LC_DYLD_ENVIRONMENT => LoadCommandVariant::DyldEnvironment(self.data()?),
            macho::LC_MAIN => LoadCommandVariant::EntryPoint(self.data()?),
            macho::LC_SOURCE_VERSION => LoadCommandVariant::SourceVersion(self.data()?),
            macho::LC_ENCRYPTION_INFO_64 => LoadCommandVariant::EncryptionInfo64(self.data()?),
            macho::LC_LINKER_OPTION => LoadCommandVariant::LinkerOption(self.data()?),
            macho::LC_NOTE => LoadCommandVariant::Note(self.data()?),
            macho::LC_BUILD_VERSION => LoadCommandVariant::BuildVersion(self.data()?),
            macho::LC_FILESET_ENTRY => LoadCommandVariant::FilesetEntry(self.data()?),
            _ => LoadCommandVariant::Other,
        })
    }

    /// Try to parse this command as a [`macho::SegmentCommand32`].
    ///
    /// Returns the segment command and the data containing the sections.
    pub fn segment_32(self) -> Result<Option<(&'data macho::SegmentCommand32<E>, &'data [u8])>> {
        if self.cmd == macho::LC_SEGMENT {
            let mut data = self.data;
            let segment = data.read().read_error("Invalid Mach-O command size")?;
            Ok(Some((segment, data.0)))
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a [`macho::SymtabCommand`].
    ///
    /// Returns the segment command and the data containing the sections.
    pub fn symtab(self) -> Result<Option<&'data macho::SymtabCommand<E>>> {
        if self.cmd == macho::LC_SYMTAB {
            Some(self.data()).transpose()
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a [`macho::DysymtabCommand`].
    pub fn dysymtab(self) -> Result<Option<&'data macho::DysymtabCommand<E>>> {
        if self.cmd == macho::LC_DYSYMTAB {
            Some(self.data()).transpose()
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a [`macho::DylibCommand`].
    pub fn dylib(self) -> Result<Option<&'data macho::DylibCommand<E>>> {
        if self.cmd == macho::LC_LOAD_DYLIB
            || self.cmd == macho::LC_LOAD_WEAK_DYLIB
            || self.cmd == macho::LC_REEXPORT_DYLIB
            || self.cmd == macho::LC_LAZY_LOAD_DYLIB
            || self.cmd == macho::LC_LOAD_UPWARD_DYLIB
        {
            Some(self.data()).transpose()
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a [`macho::UuidCommand`].
    pub fn uuid(self) -> Result<Option<&'data macho::UuidCommand<E>>> {
        if self.cmd == macho::LC_UUID {
            Some(self.data()).transpose()
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a [`macho::SegmentCommand64`].
    pub fn segment_64(self) -> Result<Option<(&'data macho::SegmentCommand64<E>, &'data [u8])>> {
        if self.cmd == macho::LC_SEGMENT_64 {
            let mut data = self.data;
            let command = data.read().read_error("Invalid Mach-O command size")?;
            Ok(Some((command, data.0)))
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a [`macho::DyldInfoCommand`].
    pub fn dyld_info(self) -> Result<Option<&'data macho::DyldInfoCommand<E>>> {
        if self.cmd == macho::LC_DYLD_INFO || self.cmd == macho::LC_DYLD_INFO_ONLY {
            Some(self.data()).transpose()
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as an [`macho::EntryPointCommand`].
    pub fn entry_point(self) -> Result<Option<&'data macho::EntryPointCommand<E>>> {
        if self.cmd == macho::LC_MAIN {
            Some(self.data()).transpose()
        } else {
            Ok(None)
        }
    }

    /// Try to parse this command as a [`macho::BuildVersionCommand`].
    pub fn build_version(self) -> Result<Option<&'data macho::BuildVersionCommand<E>>> {
        if self.cmd == macho::LC_BUILD_VERSION {
            Some(self.data()).transpose()
        } else {
            Ok(None)
        }
    }
}

/// A [`macho::LoadCommand`] that has been interpreted according to its `cmd` field.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum LoadCommandVariant<'data, E: Endian> {
    /// `LC_SEGMENT`
    Segment32(&'data macho::SegmentCommand32<E>, &'data [u8]),
    /// `LC_SYMTAB`
    Symtab(&'data macho::SymtabCommand<E>),
    // obsolete: `LC_SYMSEG`
    //Symseg(&'data macho::SymsegCommand<E>),
    /// `LC_THREAD` or `LC_UNIXTHREAD`
    Thread(&'data macho::ThreadCommand<E>, &'data [u8]),
    // obsolete: `LC_IDFVMLIB` or `LC_LOADFVMLIB`
    //Fvmlib(&'data macho::FvmlibCommand<E>),
    // obsolete: `LC_IDENT`
    //Ident(&'data macho::IdentCommand<E>),
    // internal: `LC_FVMFILE`
    //Fvmfile(&'data macho::FvmfileCommand<E>),
    // internal: `LC_PREPAGE`
    /// `LC_DYSYMTAB`
    Dysymtab(&'data macho::DysymtabCommand<E>),
    /// `LC_LOAD_DYLIB`, `LC_LOAD_WEAK_DYLIB`, `LC_REEXPORT_DYLIB`,
    /// `LC_LAZY_LOAD_DYLIB`, or `LC_LOAD_UPWARD_DYLIB`
    Dylib(&'data macho::DylibCommand<E>),
    /// `LC_ID_DYLIB`
    IdDylib(&'data macho::DylibCommand<E>),
    /// `LC_LOAD_DYLINKER`
    LoadDylinker(&'data macho::DylinkerCommand<E>),
    /// `LC_ID_DYLINKER`
    IdDylinker(&'data macho::DylinkerCommand<E>),
    /// `LC_PREBOUND_DYLIB`
    PreboundDylib(&'data macho::PreboundDylibCommand<E>),
    /// `LC_ROUTINES`
    Routines32(&'data macho::RoutinesCommand32<E>),
    /// `LC_SUB_FRAMEWORK`
    SubFramework(&'data macho::SubFrameworkCommand<E>),
    /// `LC_SUB_UMBRELLA`
    SubUmbrella(&'data macho::SubUmbrellaCommand<E>),
    /// `LC_SUB_CLIENT`
    SubClient(&'data macho::SubClientCommand<E>),
    /// `LC_SUB_LIBRARY`
    SubLibrary(&'data macho::SubLibraryCommand<E>),
    /// `LC_TWOLEVEL_HINTS`
    TwolevelHints(&'data macho::TwolevelHintsCommand<E>),
    /// `LC_PREBIND_CKSUM`
    PrebindCksum(&'data macho::PrebindCksumCommand<E>),
    /// `LC_SEGMENT_64`
    Segment64(&'data macho::SegmentCommand64<E>, &'data [u8]),
    /// `LC_ROUTINES_64`
    Routines64(&'data macho::RoutinesCommand64<E>),
    /// `LC_UUID`
    Uuid(&'data macho::UuidCommand<E>),
    /// `LC_RPATH`
    Rpath(&'data macho::RpathCommand<E>),
    /// `LC_CODE_SIGNATURE`, `LC_SEGMENT_SPLIT_INFO`, `LC_FUNCTION_STARTS`,
    /// `LC_DATA_IN_CODE`, `LC_DYLIB_CODE_SIGN_DRS`, `LC_LINKER_OPTIMIZATION_HINT`,
    /// `LC_DYLD_EXPORTS_TRIE`, or `LC_DYLD_CHAINED_FIXUPS`.
    LinkeditData(&'data macho::LinkeditDataCommand<E>),
    /// `LC_ENCRYPTION_INFO`
    EncryptionInfo32(&'data macho::EncryptionInfoCommand32<E>),
    /// `LC_DYLD_INFO` or `LC_DYLD_INFO_ONLY`
    DyldInfo(&'data macho::DyldInfoCommand<E>),
    /// `LC_VERSION_MIN_MACOSX`, `LC_VERSION_MIN_IPHONEOS`, `LC_VERSION_MIN_WATCHOS`,
    /// or `LC_VERSION_MIN_TVOS`
    VersionMin(&'data macho::VersionMinCommand<E>),
    /// `LC_DYLD_ENVIRONMENT`
    DyldEnvironment(&'data macho::DylinkerCommand<E>),
    /// `LC_MAIN`
    EntryPoint(&'data macho::EntryPointCommand<E>),
    /// `LC_SOURCE_VERSION`
    SourceVersion(&'data macho::SourceVersionCommand<E>),
    /// `LC_ENCRYPTION_INFO_64`
    EncryptionInfo64(&'data macho::EncryptionInfoCommand64<E>),
    /// `LC_LINKER_OPTION`
    LinkerOption(&'data macho::LinkerOptionCommand<E>),
    /// `LC_NOTE`
    Note(&'data macho::NoteCommand<E>),
    /// `LC_BUILD_VERSION`
    BuildVersion(&'data macho::BuildVersionCommand<E>),
    /// `LC_FILESET_ENTRY`
    FilesetEntry(&'data macho::FilesetEntryCommand<E>),
    /// An unrecognized or obsolete load command.
    Other,
}

impl<E: Endian> macho::SymtabCommand<E> {
    /// Return the symbol table that this command references.
    pub fn symbols<'data, Mach: MachHeader<Endian = E>, R: ReadRef<'data>>(
        &self,
        endian: E,
        data: R,
    ) -> Result<SymbolTable<'data, Mach, R>> {
        let symbols = data
            .read_slice_at(
                self.symoff.get(endian).into(),
                self.nsyms.get(endian) as usize,
            )
            .read_error("Invalid Mach-O symbol table offset or size")?;
        let str_start: u64 = self.stroff.get(endian).into();
        let str_end = str_start
            .checked_add(self.strsize.get(endian).into())
            .read_error("Invalid Mach-O string table length")?;
        let strings = StringTable::new(data, str_start, str_end);
        Ok(SymbolTable::new(symbols, strings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LittleEndian;

    #[test]
    fn cmd_size_invalid() {
        #[repr(align(16))]
        struct Align<const N: usize>([u8; N]);
        let mut commands = LoadCommandIterator::new(LittleEndian, &Align([0; 8]).0, 10);
        assert!(commands.next().is_err());
        let mut commands =
            LoadCommandIterator::new(LittleEndian, &Align([0, 0, 0, 0, 7, 0, 0, 0, 0]).0, 10);
        assert!(commands.next().is_err());
        let mut commands =
            LoadCommandIterator::new(LittleEndian, &Align([0, 0, 0, 0, 8, 0, 0, 0, 0]).0, 10);
        assert!(commands.next().is_ok());
    }
}
