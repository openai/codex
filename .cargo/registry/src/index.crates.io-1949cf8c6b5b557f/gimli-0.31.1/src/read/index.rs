use core::slice;

use crate::common::SectionId;
use crate::constants;
use crate::endianity::Endianity;
use crate::read::{EndianSlice, Error, Reader, ReaderOffset, Result, Section};

/// The data in the `.debug_cu_index` section of a `.dwp` file.
///
/// This section contains the compilation unit index.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebugCuIndex<R> {
    section: R,
}

impl<'input, Endian> DebugCuIndex<EndianSlice<'input, Endian>>
where
    Endian: Endianity,
{
    /// Construct a new `DebugCuIndex` instance from the data in the `.debug_cu_index`
    /// section.
    pub fn new(section: &'input [u8], endian: Endian) -> Self {
        Self::from(EndianSlice::new(section, endian))
    }
}

impl<T> DebugCuIndex<T> {
    /// Create a `DebugCuIndex` section that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// Used by `DwarfPackageSections::borrow`.
    pub(crate) fn borrow<'a, F, R>(&'a self, mut borrow: F) -> DebugCuIndex<R>
    where
        F: FnMut(&'a T) -> R,
    {
        borrow(&self.section).into()
    }
}

impl<R> Section<R> for DebugCuIndex<R> {
    fn id() -> SectionId {
        SectionId::DebugCuIndex
    }

    fn reader(&self) -> &R {
        &self.section
    }
}

impl<R> From<R> for DebugCuIndex<R> {
    fn from(section: R) -> Self {
        DebugCuIndex { section }
    }
}

impl<R: Reader> DebugCuIndex<R> {
    /// Parse the index header.
    pub fn index(self) -> Result<UnitIndex<R>> {
        UnitIndex::parse(self.section)
    }
}

/// The data in the `.debug_tu_index` section of a `.dwp` file.
///
/// This section contains the type unit index.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebugTuIndex<R> {
    section: R,
}

impl<'input, Endian> DebugTuIndex<EndianSlice<'input, Endian>>
where
    Endian: Endianity,
{
    /// Construct a new `DebugTuIndex` instance from the data in the `.debug_tu_index`
    /// section.
    pub fn new(section: &'input [u8], endian: Endian) -> Self {
        Self::from(EndianSlice::new(section, endian))
    }
}

impl<T> DebugTuIndex<T> {
    /// Create a `DebugTuIndex` section that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// Used by `DwarfPackageSections::borrow`.
    pub(crate) fn borrow<'a, F, R>(&'a self, mut borrow: F) -> DebugTuIndex<R>
    where
        F: FnMut(&'a T) -> R,
    {
        borrow(&self.section).into()
    }
}

impl<R> Section<R> for DebugTuIndex<R> {
    fn id() -> SectionId {
        SectionId::DebugTuIndex
    }

    fn reader(&self) -> &R {
        &self.section
    }
}

impl<R> From<R> for DebugTuIndex<R> {
    fn from(section: R) -> Self {
        DebugTuIndex { section }
    }
}

impl<R: Reader> DebugTuIndex<R> {
    /// Parse the index header.
    pub fn index(self) -> Result<UnitIndex<R>> {
        UnitIndex::parse(self.section)
    }
}

const SECTION_COUNT_MAX: u8 = 8;

/// The partially parsed index from a `DebugCuIndex` or `DebugTuIndex`.
#[derive(Debug, Clone)]
pub struct UnitIndex<R: Reader> {
    version: u16,
    section_count: u32,
    unit_count: u32,
    slot_count: u32,
    hash_ids: R,
    hash_rows: R,
    // Only `section_count` values are valid.
    sections: [IndexSectionId; SECTION_COUNT_MAX as usize],
    offsets: R,
    sizes: R,
}

impl<R: Reader> UnitIndex<R> {
    fn parse(mut input: R) -> Result<UnitIndex<R>> {
        if input.is_empty() {
            return Ok(UnitIndex {
                version: 0,
                section_count: 0,
                unit_count: 0,
                slot_count: 0,
                hash_ids: input.clone(),
                hash_rows: input.clone(),
                sections: [IndexSectionId::DebugAbbrev; SECTION_COUNT_MAX as usize],
                offsets: input.clone(),
                sizes: input.clone(),
            });
        }

        // GNU split-dwarf extension to DWARF 4 uses a 32-bit version,
        // but DWARF 5 uses a 16-bit version followed by 16-bit padding.
        let mut original_input = input.clone();
        let version;
        if input.read_u32()? == 2 {
            version = 2
        } else {
            version = original_input.read_u16()?;
            if version != 5 {
                return Err(Error::UnknownVersion(version.into()));
            }
        }

        let section_count = input.read_u32()?;
        let unit_count = input.read_u32()?;
        let slot_count = input.read_u32()?;
        if slot_count != 0 && (slot_count & (slot_count - 1) != 0 || slot_count <= unit_count) {
            return Err(Error::InvalidIndexSlotCount);
        }

        let hash_ids = input.split(R::Offset::from_u64(u64::from(slot_count) * 8)?)?;
        let hash_rows = input.split(R::Offset::from_u64(u64::from(slot_count) * 4)?)?;

        let mut sections = [IndexSectionId::DebugAbbrev; SECTION_COUNT_MAX as usize];
        if section_count > SECTION_COUNT_MAX.into() {
            return Err(Error::InvalidIndexSectionCount);
        }
        for i in 0..section_count {
            let section = input.read_u32()?;
            sections[i as usize] = if version == 2 {
                match constants::DwSectV2(section) {
                    constants::DW_SECT_V2_INFO => IndexSectionId::DebugInfo,
                    constants::DW_SECT_V2_TYPES => IndexSectionId::DebugTypes,
                    constants::DW_SECT_V2_ABBREV => IndexSectionId::DebugAbbrev,
                    constants::DW_SECT_V2_LINE => IndexSectionId::DebugLine,
                    constants::DW_SECT_V2_LOC => IndexSectionId::DebugLoc,
                    constants::DW_SECT_V2_STR_OFFSETS => IndexSectionId::DebugStrOffsets,
                    constants::DW_SECT_V2_MACINFO => IndexSectionId::DebugMacinfo,
                    constants::DW_SECT_V2_MACRO => IndexSectionId::DebugMacro,
                    section => return Err(Error::UnknownIndexSectionV2(section)),
                }
            } else {
                match constants::DwSect(section) {
                    constants::DW_SECT_INFO => IndexSectionId::DebugInfo,
                    constants::DW_SECT_ABBREV => IndexSectionId::DebugAbbrev,
                    constants::DW_SECT_LINE => IndexSectionId::DebugLine,
                    constants::DW_SECT_LOCLISTS => IndexSectionId::DebugLocLists,
                    constants::DW_SECT_STR_OFFSETS => IndexSectionId::DebugStrOffsets,
                    constants::DW_SECT_MACRO => IndexSectionId::DebugMacro,
                    constants::DW_SECT_RNGLISTS => IndexSectionId::DebugRngLists,
                    section => return Err(Error::UnknownIndexSection(section)),
                }
            };
        }

        let offsets = input.split(R::Offset::from_u64(
            u64::from(unit_count) * u64::from(section_count) * 4,
        )?)?;
        let sizes = input.split(R::Offset::from_u64(
            u64::from(unit_count) * u64::from(section_count) * 4,
        )?)?;

        Ok(UnitIndex {
            version,
            section_count,
            unit_count,
            slot_count,
            hash_ids,
            hash_rows,
            sections,
            offsets,
            sizes,
        })
    }

    /// Find `id` in the index hash table, and return the row index.
    ///
    /// `id` may be a compilation unit ID if this index is from `.debug_cu_index`,
    /// or a type signature if this index is from `.debug_tu_index`.
    pub fn find(&self, id: u64) -> Option<u32> {
        if self.slot_count == 0 {
            return None;
        }
        let mask = u64::from(self.slot_count - 1);
        let mut hash1 = id & mask;
        let hash2 = ((id >> 32) & mask) | 1;
        for _ in 0..self.slot_count {
            // The length of these arrays was validated in `UnitIndex::parse`.
            let mut hash_ids = self.hash_ids.clone();
            hash_ids.skip(R::Offset::from_u64(hash1 * 8).ok()?).ok()?;
            let hash_id = hash_ids.read_u64().ok()?;
            if hash_id == id {
                let mut hash_rows = self.hash_rows.clone();
                hash_rows.skip(R::Offset::from_u64(hash1 * 4).ok()?).ok()?;
                let hash_row = hash_rows.read_u32().ok()?;
                return Some(hash_row);
            }
            if hash_id == 0 {
                return None;
            }
            hash1 = (hash1 + hash2) & mask;
        }
        None
    }

    /// Return the section offsets and sizes for the given row index.
    pub fn sections(&self, mut row: u32) -> Result<UnitIndexSectionIterator<'_, R>> {
        if row == 0 {
            return Err(Error::InvalidIndexRow);
        }
        row -= 1;
        if row >= self.unit_count {
            return Err(Error::InvalidIndexRow);
        }
        let mut offsets = self.offsets.clone();
        offsets.skip(R::Offset::from_u64(
            u64::from(row) * u64::from(self.section_count) * 4,
        )?)?;
        let mut sizes = self.sizes.clone();
        sizes.skip(R::Offset::from_u64(
            u64::from(row) * u64::from(self.section_count) * 4,
        )?)?;
        Ok(UnitIndexSectionIterator {
            sections: self.sections[..self.section_count as usize].iter(),
            offsets,
            sizes,
        })
    }

    /// Return the version.
    ///
    /// Defaults to 0 for empty sections.
    pub fn version(&self) -> u16 {
        self.version
    }

    /// Return the number of sections.
    pub fn section_count(&self) -> u32 {
        self.section_count
    }

    /// Return the number of units.
    pub fn unit_count(&self) -> u32 {
        self.unit_count
    }

    /// Return the number of slots.
    pub fn slot_count(&self) -> u32 {
        self.slot_count
    }
}

/// An iterator over the section offsets and sizes for a row in a `UnitIndex`.
#[derive(Debug, Clone)]
pub struct UnitIndexSectionIterator<'index, R: Reader> {
    sections: slice::Iter<'index, IndexSectionId>,
    offsets: R,
    sizes: R,
}

impl<'index, R: Reader> Iterator for UnitIndexSectionIterator<'index, R> {
    type Item = UnitIndexSection;

    fn next(&mut self) -> Option<UnitIndexSection> {
        let section = *self.sections.next()?;
        // The length of these arrays was validated in `UnitIndex::parse`.
        let offset = self.offsets.read_u32().ok()?;
        let size = self.sizes.read_u32().ok()?;
        Some(UnitIndexSection {
            section,
            offset,
            size,
        })
    }
}

/// Information about a unit's contribution to a section in a `.dwp` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitIndexSection {
    /// The section kind.
    pub section: IndexSectionId,
    /// The base offset of the unit's contribution to the section.
    pub offset: u32,
    /// The size of the unit's contribution to the section.
    pub size: u32,
}

/// Section kinds which are permitted in a `.dwp` index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexSectionId {
    /// The `.debug_abbrev.dwo` section.
    DebugAbbrev,
    /// The `.debug_info.dwo` section.
    DebugInfo,
    /// The `.debug_line.dwo` section.
    DebugLine,
    /// The `.debug_loc.dwo` section.
    DebugLoc,
    /// The `.debug_loclists.dwo` section.
    DebugLocLists,
    /// The `.debug_macinfo.dwo` section.
    DebugMacinfo,
    /// The `.debug_macro.dwo` section.
    DebugMacro,
    /// The `.debug_rnglists.dwo` section.
    DebugRngLists,
    /// The `.debug_str_offsets.dwo` section.
    DebugStrOffsets,
    /// The `.debug_types.dwo` section.
    DebugTypes,
}

impl IndexSectionId {
    /// Returns the corresponding `SectionId`.
    pub fn section_id(self) -> SectionId {
        match self {
            IndexSectionId::DebugAbbrev => SectionId::DebugAbbrev,
            IndexSectionId::DebugInfo => SectionId::DebugInfo,
            IndexSectionId::DebugLine => SectionId::DebugLine,
            IndexSectionId::DebugLoc => SectionId::DebugLoc,
            IndexSectionId::DebugLocLists => SectionId::DebugLocLists,
            IndexSectionId::DebugMacro => SectionId::DebugMacro,
            IndexSectionId::DebugMacinfo => SectionId::DebugMacinfo,
            IndexSectionId::DebugRngLists => SectionId::DebugRngLists,
            IndexSectionId::DebugStrOffsets => SectionId::DebugStrOffsets,
            IndexSectionId::DebugTypes => SectionId::DebugTypes,
        }
    }

    /// Returns the ELF section name for this kind, when found in a .dwo or .dwp file.
    pub fn dwo_name(self) -> &'static str {
        self.section_id().dwo_name().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endianity::BigEndian;
    use test_assembler::{Endian, Section};

    #[test]
    fn test_empty() {
        let buf = EndianSlice::new(&[], BigEndian);
        let index = UnitIndex::parse(buf).unwrap();
        assert_eq!(index.version(), 0);
        assert_eq!(index.unit_count(), 0);
        assert_eq!(index.slot_count(), 0);
        assert!(index.find(0).is_none());
    }

    #[test]
    fn test_zero_slots() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D32(2).D32(0).D32(0).D32(0);
        let buf = section.get_contents().unwrap();
        let buf = EndianSlice::new(&buf, BigEndian);
        let index = UnitIndex::parse(buf).unwrap();
        assert_eq!(index.version(), 2);
        assert_eq!(index.unit_count(), 0);
        assert_eq!(index.slot_count(), 0);
        assert!(index.find(0).is_none());
    }

    #[test]
    fn test_version_2() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D32(2).D32(0).D32(0).D32(1)
            // Slots.
            .D64(0).D32(0);
        let buf = section.get_contents().unwrap();
        let buf = EndianSlice::new(&buf, BigEndian);
        let index = UnitIndex::parse(buf).unwrap();
        assert_eq!(index.version, 2);
    }

    #[test]
    fn test_version_5() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D16(5).D16(0).D32(0).D32(0).D32(1)
            // Slots.
            .D64(0).D32(0);
        let buf = section.get_contents().unwrap();
        let buf = EndianSlice::new(&buf, BigEndian);
        let index = UnitIndex::parse(buf).unwrap();
        assert_eq!(index.version, 5);
    }

    #[test]
    fn test_version_5_invalid() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D32(5).D32(0).D32(0).D32(1)
            // Slots.
            .D64(0).D32(0);
        let buf = section.get_contents().unwrap();
        let buf = EndianSlice::new(&buf, BigEndian);
        assert!(UnitIndex::parse(buf).is_err());
    }

    #[test]
    fn test_version_2_sections() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D32(2).D32(8).D32(1).D32(2)
            // Slots.
            .D64(0).D64(0).D32(0).D32(0)
            // Sections.
            .D32(constants::DW_SECT_V2_INFO.0)
            .D32(constants::DW_SECT_V2_TYPES.0)
            .D32(constants::DW_SECT_V2_ABBREV.0)
            .D32(constants::DW_SECT_V2_LINE.0)
            .D32(constants::DW_SECT_V2_LOC.0)
            .D32(constants::DW_SECT_V2_STR_OFFSETS.0)
            .D32(constants::DW_SECT_V2_MACINFO.0)
            .D32(constants::DW_SECT_V2_MACRO.0)
            // Offsets.
            .D32(11).D32(12).D32(13).D32(14).D32(15).D32(16).D32(17).D32(18)
            // Sizes.
            .D32(21).D32(22).D32(23).D32(24).D32(25).D32(26).D32(27).D32(28);
        let buf = section.get_contents().unwrap();
        let buf = EndianSlice::new(&buf, BigEndian);
        let index = UnitIndex::parse(buf).unwrap();
        assert_eq!(index.section_count, 8);
        assert_eq!(
            index.sections,
            [
                IndexSectionId::DebugInfo,
                IndexSectionId::DebugTypes,
                IndexSectionId::DebugAbbrev,
                IndexSectionId::DebugLine,
                IndexSectionId::DebugLoc,
                IndexSectionId::DebugStrOffsets,
                IndexSectionId::DebugMacinfo,
                IndexSectionId::DebugMacro,
            ]
        );
        #[rustfmt::skip]
        let expect = [
            UnitIndexSection { section: IndexSectionId::DebugInfo, offset: 11, size: 21 },
            UnitIndexSection { section: IndexSectionId::DebugTypes, offset: 12, size: 22 },
            UnitIndexSection { section: IndexSectionId::DebugAbbrev, offset: 13, size: 23 },
            UnitIndexSection { section: IndexSectionId::DebugLine, offset: 14, size: 24 },
            UnitIndexSection { section: IndexSectionId::DebugLoc, offset: 15, size: 25 },
            UnitIndexSection { section: IndexSectionId::DebugStrOffsets, offset: 16, size: 26 },
            UnitIndexSection { section: IndexSectionId::DebugMacinfo, offset: 17, size: 27 },
            UnitIndexSection { section: IndexSectionId::DebugMacro, offset: 18, size: 28 },
        ];
        let mut sections = index.sections(1).unwrap();
        for section in &expect {
            assert_eq!(*section, sections.next().unwrap());
        }
        assert!(sections.next().is_none());
    }

    #[test]
    fn test_version_5_sections() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D16(5).D16(0).D32(7).D32(1).D32(2)
            // Slots.
            .D64(0).D64(0).D32(0).D32(0)
            // Sections.
            .D32(constants::DW_SECT_INFO.0)
            .D32(constants::DW_SECT_ABBREV.0)
            .D32(constants::DW_SECT_LINE.0)
            .D32(constants::DW_SECT_LOCLISTS.0)
            .D32(constants::DW_SECT_STR_OFFSETS.0)
            .D32(constants::DW_SECT_MACRO.0)
            .D32(constants::DW_SECT_RNGLISTS.0)
            // Offsets.
            .D32(11).D32(12).D32(13).D32(14).D32(15).D32(16).D32(17)
            // Sizes.
            .D32(21).D32(22).D32(23).D32(24).D32(25).D32(26).D32(27);
        let buf = section.get_contents().unwrap();
        let buf = EndianSlice::new(&buf, BigEndian);
        let index = UnitIndex::parse(buf).unwrap();
        assert_eq!(index.section_count, 7);
        assert_eq!(
            index.sections[..7],
            [
                IndexSectionId::DebugInfo,
                IndexSectionId::DebugAbbrev,
                IndexSectionId::DebugLine,
                IndexSectionId::DebugLocLists,
                IndexSectionId::DebugStrOffsets,
                IndexSectionId::DebugMacro,
                IndexSectionId::DebugRngLists,
            ]
        );
        #[rustfmt::skip]
        let expect = [
            UnitIndexSection { section: IndexSectionId::DebugInfo, offset: 11, size: 21 },
            UnitIndexSection { section: IndexSectionId::DebugAbbrev, offset: 12, size: 22 },
            UnitIndexSection { section: IndexSectionId::DebugLine, offset: 13, size: 23 },
            UnitIndexSection { section: IndexSectionId::DebugLocLists, offset: 14, size: 24 },
            UnitIndexSection { section: IndexSectionId::DebugStrOffsets, offset: 15, size: 25 },
            UnitIndexSection { section: IndexSectionId::DebugMacro, offset: 16, size: 26 },
            UnitIndexSection { section: IndexSectionId::DebugRngLists, offset: 17, size: 27 },
        ];
        let mut sections = index.sections(1).unwrap();
        for section in &expect {
            assert_eq!(*section, sections.next().unwrap());
        }
        assert!(sections.next().is_none());

        assert!(index.sections(0).is_err());
        assert!(index.sections(2).is_err());
    }

    #[test]
    fn test_hash() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D16(5).D16(0).D32(2).D32(3).D32(4)
            // Slots.
            .D64(0xffff_fff2_ffff_fff1)
            .D64(0xffff_fff0_ffff_fff1)
            .D64(0xffff_fff1_ffff_fff1)
            .D64(0)
            .D32(3).D32(1).D32(2).D32(0)
            // Sections.
            .D32(constants::DW_SECT_INFO.0)
            .D32(constants::DW_SECT_ABBREV.0)
            // Offsets.
            .D32(0).D32(0).D32(0).D32(0).D32(0).D32(0)
            // Sizes.
            .D32(0).D32(0).D32(0).D32(0).D32(0).D32(0);
        let buf = section.get_contents().unwrap();
        let buf = EndianSlice::new(&buf, BigEndian);
        let index = UnitIndex::parse(buf).unwrap();
        assert_eq!(index.version(), 5);
        assert_eq!(index.slot_count(), 4);
        assert_eq!(index.unit_count(), 3);
        assert_eq!(index.section_count(), 2);
        assert_eq!(index.find(0xffff_fff0_ffff_fff1), Some(1));
        assert_eq!(index.find(0xffff_fff1_ffff_fff1), Some(2));
        assert_eq!(index.find(0xffff_fff2_ffff_fff1), Some(3));
        assert_eq!(index.find(0xffff_fff3_ffff_fff1), None);
    }

    #[test]
    fn test_cu_index() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D16(5).D16(0).D32(0).D32(0).D32(1)
            // Slots.
            .D64(0).D32(0);
        let buf = section.get_contents().unwrap();
        let cu_index = DebugCuIndex::new(&buf, BigEndian);
        let index = cu_index.index().unwrap();
        assert_eq!(index.version, 5);
    }

    #[test]
    fn test_tu_index() {
        #[rustfmt::skip]
        let section = Section::with_endian(Endian::Big)
            // Header.
            .D16(5).D16(0).D32(0).D32(0).D32(1)
            // Slots.
            .D64(0).D32(0);
        let buf = section.get_contents().unwrap();
        let tu_index = DebugTuIndex::new(&buf, BigEndian);
        let index = tu_index.index().unwrap();
        assert_eq!(index.version, 5);
    }
}
