use alloc::vec::Vec;

use crate::read::{Bytes, ReadError, ReadRef, Result, StringTable, SymbolIndex};
use crate::{elf, endian};

use super::FileHeader;

/// A version index.
#[derive(Debug, Default, Clone, Copy)]
pub struct VersionIndex(pub u16);

impl VersionIndex {
    /// Return the version index.
    pub fn index(&self) -> u16 {
        self.0 & elf::VERSYM_VERSION
    }

    /// Return true if it is the local index.
    pub fn is_local(&self) -> bool {
        self.index() == elf::VER_NDX_LOCAL
    }

    /// Return true if it is the global index.
    pub fn is_global(&self) -> bool {
        self.index() == elf::VER_NDX_GLOBAL
    }

    /// Return the hidden flag.
    pub fn is_hidden(&self) -> bool {
        self.0 & elf::VERSYM_HIDDEN != 0
    }
}

/// A version definition or requirement.
///
/// This is derived from entries in the [`elf::SHT_GNU_VERDEF`] and [`elf::SHT_GNU_VERNEED`] sections.
#[derive(Debug, Default, Clone, Copy)]
pub struct Version<'data> {
    name: &'data [u8],
    hash: u32,
    // Used to keep track of valid indices in `VersionTable`.
    valid: bool,
    file: Option<&'data [u8]>,
}

impl<'data> Version<'data> {
    /// Return the version name.
    pub fn name(&self) -> &'data [u8] {
        self.name
    }

    /// Return hash of the version name.
    pub fn hash(&self) -> u32 {
        self.hash
    }

    /// Return the filename of the library containing this version.
    ///
    /// This is the `vn_file` field of the associated entry in [`elf::SHT_GNU_VERNEED`].
    /// or `None` if the version info was parsed from a [`elf::SHT_GNU_VERDEF`] section.
    pub fn file(&self) -> Option<&'data [u8]> {
        self.file
    }
}

/// A table of version definitions and requirements.
///
/// It allows looking up the version information for a given symbol index.
///
/// This is derived from entries in the [`elf::SHT_GNU_VERSYM`], [`elf::SHT_GNU_VERDEF`]
/// and [`elf::SHT_GNU_VERNEED`] sections.
///
/// Returned by [`SectionTable::versions`](super::SectionTable::versions).
#[derive(Debug, Clone)]
pub struct VersionTable<'data, Elf: FileHeader> {
    symbols: &'data [elf::Versym<Elf::Endian>],
    versions: Vec<Version<'data>>,
}

impl<'data, Elf: FileHeader> Default for VersionTable<'data, Elf> {
    fn default() -> Self {
        VersionTable {
            symbols: &[],
            versions: Vec::new(),
        }
    }
}

impl<'data, Elf: FileHeader> VersionTable<'data, Elf> {
    /// Parse the version sections.
    pub fn parse<R: ReadRef<'data>>(
        endian: Elf::Endian,
        versyms: &'data [elf::Versym<Elf::Endian>],
        verdefs: Option<VerdefIterator<'data, Elf>>,
        verneeds: Option<VerneedIterator<'data, Elf>>,
        strings: StringTable<'data, R>,
    ) -> Result<Self> {
        let mut max_index = 0;
        if let Some(mut verdefs) = verdefs.clone() {
            while let Some((verdef, _)) = verdefs.next()? {
                if verdef.vd_flags.get(endian) & elf::VER_FLG_BASE != 0 {
                    continue;
                }
                let index = verdef.vd_ndx.get(endian) & elf::VERSYM_VERSION;
                if max_index < index {
                    max_index = index;
                }
            }
        }
        if let Some(mut verneeds) = verneeds.clone() {
            while let Some((_, mut vernauxs)) = verneeds.next()? {
                while let Some(vernaux) = vernauxs.next()? {
                    let index = vernaux.vna_other.get(endian) & elf::VERSYM_VERSION;
                    if max_index < index {
                        max_index = index;
                    }
                }
            }
        }

        // Indices should be sequential, but this could be up to
        // 32k * size_of::<Version>() if max_index is bad.
        let mut versions = vec![Version::default(); max_index as usize + 1];

        if let Some(mut verdefs) = verdefs {
            while let Some((verdef, mut verdauxs)) = verdefs.next()? {
                if verdef.vd_flags.get(endian) & elf::VER_FLG_BASE != 0 {
                    continue;
                }
                let index = verdef.vd_ndx.get(endian) & elf::VERSYM_VERSION;
                if index <= elf::VER_NDX_GLOBAL {
                    // TODO: return error?
                    continue;
                }
                if let Some(verdaux) = verdauxs.next()? {
                    versions[usize::from(index)] = Version {
                        name: verdaux.name(endian, strings)?,
                        hash: verdef.vd_hash.get(endian),
                        valid: true,
                        file: None,
                    };
                }
            }
        }
        if let Some(mut verneeds) = verneeds {
            while let Some((verneed, mut vernauxs)) = verneeds.next()? {
                while let Some(vernaux) = vernauxs.next()? {
                    let index = vernaux.vna_other.get(endian) & elf::VERSYM_VERSION;
                    if index <= elf::VER_NDX_GLOBAL {
                        // TODO: return error?
                        continue;
                    }
                    versions[usize::from(index)] = Version {
                        name: vernaux.name(endian, strings)?,
                        hash: vernaux.vna_hash.get(endian),
                        valid: true,
                        file: Some(verneed.file(endian, strings)?),
                    };
                }
            }
        }

        Ok(VersionTable {
            symbols: versyms,
            versions,
        })
    }

    /// Return true if the version table is empty.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Return version index for a given symbol index.
    pub fn version_index(&self, endian: Elf::Endian, index: SymbolIndex) -> VersionIndex {
        let version_index = match self.symbols.get(index.0) {
            Some(x) => x.0.get(endian),
            // Ideally this would be VER_NDX_LOCAL for undefined symbols,
            // but currently there are no checks that need this distinction.
            None => elf::VER_NDX_GLOBAL,
        };
        VersionIndex(version_index)
    }

    /// Return version information for a given symbol version index.
    ///
    /// Returns `Ok(None)` for local and global versions.
    /// Returns `Err(_)` if index is invalid.
    pub fn version(&self, index: VersionIndex) -> Result<Option<&Version<'data>>> {
        if index.index() <= elf::VER_NDX_GLOBAL {
            return Ok(None);
        }
        self.versions
            .get(usize::from(index.index()))
            .filter(|version| version.valid)
            .read_error("Invalid ELF symbol version index")
            .map(Some)
    }

    /// Return true if the given symbol index satisfies the requirements of `need`.
    ///
    /// Returns false for any error.
    ///
    /// Note: this function hasn't been fully tested and is likely to be incomplete.
    pub fn matches(
        &self,
        endian: Elf::Endian,
        index: SymbolIndex,
        need: Option<&Version<'_>>,
    ) -> bool {
        let version_index = self.version_index(endian, index);
        let def = match self.version(version_index) {
            Ok(def) => def,
            Err(_) => return false,
        };
        match (def, need) {
            (Some(def), Some(need)) => need.hash == def.hash && need.name == def.name,
            (None, Some(_need)) => {
                // Version must be present if needed.
                false
            }
            (Some(_def), None) => {
                // For a dlsym call, use the newest version.
                // TODO: if not a dlsym call, then use the oldest version.
                !version_index.is_hidden()
            }
            (None, None) => true,
        }
    }
}

/// An iterator for the entries in an ELF [`elf::SHT_GNU_VERDEF`] section.
#[derive(Debug, Clone)]
pub struct VerdefIterator<'data, Elf: FileHeader> {
    endian: Elf::Endian,
    data: Bytes<'data>,
}

impl<'data, Elf: FileHeader> VerdefIterator<'data, Elf> {
    pub(super) fn new(endian: Elf::Endian, data: &'data [u8]) -> Self {
        VerdefIterator {
            endian,
            data: Bytes(data),
        }
    }

    /// Return the next `Verdef` entry.
    pub fn next(
        &mut self,
    ) -> Result<Option<(&'data elf::Verdef<Elf::Endian>, VerdauxIterator<'data, Elf>)>> {
        if self.data.is_empty() {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.data = Bytes(&[]);
        }
        result
    }

    fn parse(&mut self) -> Result<(&'data elf::Verdef<Elf::Endian>, VerdauxIterator<'data, Elf>)> {
        let verdef = self
            .data
            .read_at::<elf::Verdef<_>>(0)
            .read_error("ELF verdef is too short")?;

        let mut verdaux_data = self.data;
        verdaux_data
            .skip(verdef.vd_aux.get(self.endian) as usize)
            .read_error("Invalid ELF vd_aux")?;
        let verdaux =
            VerdauxIterator::new(self.endian, verdaux_data.0, verdef.vd_cnt.get(self.endian));

        let next = verdef.vd_next.get(self.endian);
        if next != 0 {
            self.data
                .skip(next as usize)
                .read_error("Invalid ELF vd_next")?;
        } else {
            self.data = Bytes(&[]);
        }
        Ok((verdef, verdaux))
    }
}

impl<'data, Elf: FileHeader> Iterator for VerdefIterator<'data, Elf> {
    type Item = Result<(&'data elf::Verdef<Elf::Endian>, VerdauxIterator<'data, Elf>)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// An iterator for the auxiliary records for an entry in an ELF [`elf::SHT_GNU_VERDEF`] section.
#[derive(Debug, Clone)]
pub struct VerdauxIterator<'data, Elf: FileHeader> {
    endian: Elf::Endian,
    data: Bytes<'data>,
    count: u16,
}

impl<'data, Elf: FileHeader> VerdauxIterator<'data, Elf> {
    pub(super) fn new(endian: Elf::Endian, data: &'data [u8], count: u16) -> Self {
        VerdauxIterator {
            endian,
            data: Bytes(data),
            count,
        }
    }

    /// Return the next `Verdaux` entry.
    pub fn next(&mut self) -> Result<Option<&'data elf::Verdaux<Elf::Endian>>> {
        if self.count == 0 {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.count = 0;
        } else {
            self.count -= 1;
        }
        result
    }

    fn parse(&mut self) -> Result<&'data elf::Verdaux<Elf::Endian>> {
        let verdaux = self
            .data
            .read_at::<elf::Verdaux<_>>(0)
            .read_error("ELF verdaux is too short")?;

        self.data
            .skip(verdaux.vda_next.get(self.endian) as usize)
            .read_error("Invalid ELF vda_next")?;
        Ok(verdaux)
    }
}

impl<'data, Elf: FileHeader> Iterator for VerdauxIterator<'data, Elf> {
    type Item = Result<&'data elf::Verdaux<Elf::Endian>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// An iterator for the entries in an ELF [`elf::SHT_GNU_VERNEED`] section.
#[derive(Debug, Clone)]
pub struct VerneedIterator<'data, Elf: FileHeader> {
    endian: Elf::Endian,
    data: Bytes<'data>,
}

impl<'data, Elf: FileHeader> VerneedIterator<'data, Elf> {
    pub(super) fn new(endian: Elf::Endian, data: &'data [u8]) -> Self {
        VerneedIterator {
            endian,
            data: Bytes(data),
        }
    }

    /// Return the next `Verneed` entry.
    pub fn next(
        &mut self,
    ) -> Result<
        Option<(
            &'data elf::Verneed<Elf::Endian>,
            VernauxIterator<'data, Elf>,
        )>,
    > {
        if self.data.is_empty() {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.data = Bytes(&[]);
        }
        result
    }

    fn parse(
        &mut self,
    ) -> Result<(
        &'data elf::Verneed<Elf::Endian>,
        VernauxIterator<'data, Elf>,
    )> {
        let verneed = self
            .data
            .read_at::<elf::Verneed<_>>(0)
            .read_error("ELF verneed is too short")?;

        let mut vernaux_data = self.data;
        vernaux_data
            .skip(verneed.vn_aux.get(self.endian) as usize)
            .read_error("Invalid ELF vn_aux")?;
        let vernaux =
            VernauxIterator::new(self.endian, vernaux_data.0, verneed.vn_cnt.get(self.endian));

        let next = verneed.vn_next.get(self.endian);
        if next != 0 {
            self.data
                .skip(next as usize)
                .read_error("Invalid ELF vn_next")?;
        } else {
            self.data = Bytes(&[]);
        }
        Ok((verneed, vernaux))
    }
}

impl<'data, Elf: FileHeader> Iterator for VerneedIterator<'data, Elf> {
    type Item = Result<(
        &'data elf::Verneed<Elf::Endian>,
        VernauxIterator<'data, Elf>,
    )>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

/// An iterator for the auxiliary records for an entry in an ELF [`elf::SHT_GNU_VERNEED`] section.
#[derive(Debug, Clone)]
pub struct VernauxIterator<'data, Elf: FileHeader> {
    endian: Elf::Endian,
    data: Bytes<'data>,
    count: u16,
}

impl<'data, Elf: FileHeader> VernauxIterator<'data, Elf> {
    pub(super) fn new(endian: Elf::Endian, data: &'data [u8], count: u16) -> Self {
        VernauxIterator {
            endian,
            data: Bytes(data),
            count,
        }
    }

    /// Return the next `Vernaux` entry.
    pub fn next(&mut self) -> Result<Option<&'data elf::Vernaux<Elf::Endian>>> {
        if self.count == 0 {
            return Ok(None);
        }

        let result = self.parse().map(Some);
        if result.is_err() {
            self.count = 0;
        } else {
            self.count -= 1;
        }
        result
    }

    fn parse(&mut self) -> Result<&'data elf::Vernaux<Elf::Endian>> {
        let vernaux = self
            .data
            .read_at::<elf::Vernaux<_>>(0)
            .read_error("ELF vernaux is too short")?;
        self.data
            .skip(vernaux.vna_next.get(self.endian) as usize)
            .read_error("Invalid ELF vna_next")?;
        Ok(vernaux)
    }
}

impl<'data, Elf: FileHeader> Iterator for VernauxIterator<'data, Elf> {
    type Item = Result<&'data elf::Vernaux<Elf::Endian>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next().transpose()
    }
}

impl<Endian: endian::Endian> elf::Verdaux<Endian> {
    /// Parse the version name from the string table.
    pub fn name<'data, R: ReadRef<'data>>(
        &self,
        endian: Endian,
        strings: StringTable<'data, R>,
    ) -> Result<&'data [u8]> {
        strings
            .get(self.vda_name.get(endian))
            .read_error("Invalid ELF vda_name")
    }
}

impl<Endian: endian::Endian> elf::Verneed<Endian> {
    /// Parse the file from the string table.
    pub fn file<'data, R: ReadRef<'data>>(
        &self,
        endian: Endian,
        strings: StringTable<'data, R>,
    ) -> Result<&'data [u8]> {
        strings
            .get(self.vn_file.get(endian))
            .read_error("Invalid ELF vn_file")
    }
}

impl<Endian: endian::Endian> elf::Vernaux<Endian> {
    /// Parse the version name from the string table.
    pub fn name<'data, R: ReadRef<'data>>(
        &self,
        endian: Endian,
        strings: StringTable<'data, R>,
    ) -> Result<&'data [u8]> {
        strings
            .get(self.vna_name.get(endian))
            .read_error("Invalid ELF vna_name")
    }
}
