use alloc::vec::Vec;
use indexmap::IndexSet;
use std::ops::{Deref, DerefMut};

use crate::common::{Encoding, RangeListsOffset, SectionId};
use crate::write::{Address, BaseId, Error, Result, Section, Sections, Writer};

define_section!(
    DebugRanges,
    RangeListsOffset,
    "A writable `.debug_ranges` section."
);
define_section!(
    DebugRngLists,
    RangeListsOffset,
    "A writable `.debug_rnglists` section."
);

define_offsets!(
    RangeListOffsets: RangeListId => RangeListsOffset,
    "The section offsets of a series of range lists within the `.debug_ranges` or `.debug_rnglists` sections."
);

define_id!(
    RangeListId,
    "An identifier for a range list in a `RangeListTable`."
);

/// A table of range lists that will be stored in a `.debug_ranges` or `.debug_rnglists` section.
#[derive(Debug, Default)]
pub struct RangeListTable {
    base_id: BaseId,
    ranges: IndexSet<RangeList>,
}

impl RangeListTable {
    /// Add a range list to the table.
    pub fn add(&mut self, range_list: RangeList) -> RangeListId {
        let (index, _) = self.ranges.insert_full(range_list);
        RangeListId::new(self.base_id, index)
    }

    /// Get a reference to a location list.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    #[inline]
    pub fn get(&self, id: RangeListId) -> &RangeList {
        debug_assert_eq!(self.base_id, id.base_id);
        &self.ranges[id.index]
    }

    /// Write the range list table to the appropriate section for the given DWARF version.
    pub(crate) fn write<W: Writer>(
        &self,
        sections: &mut Sections<W>,
        encoding: Encoding,
    ) -> Result<RangeListOffsets> {
        if self.ranges.is_empty() {
            return Ok(RangeListOffsets::none());
        }

        match encoding.version {
            2..=4 => self.write_ranges(&mut sections.debug_ranges, encoding.address_size),
            5 => self.write_rnglists(&mut sections.debug_rnglists, encoding),
            _ => Err(Error::UnsupportedVersion(encoding.version)),
        }
    }

    /// Write the range list table to the `.debug_ranges` section.
    fn write_ranges<W: Writer>(
        &self,
        w: &mut DebugRanges<W>,
        address_size: u8,
    ) -> Result<RangeListOffsets> {
        let mut offsets = Vec::new();
        for range_list in self.ranges.iter() {
            offsets.push(w.offset());
            for range in &range_list.0 {
                // Note that we must ensure none of the ranges have both begin == 0 and end == 0.
                // We do this by ensuring that begin != end, which is a bit more restrictive
                // than required, but still seems reasonable.
                match *range {
                    Range::BaseAddress { address } => {
                        let marker = !0 >> (64 - address_size * 8);
                        w.write_udata(marker, address_size)?;
                        w.write_address(address, address_size)?;
                    }
                    Range::OffsetPair { begin, end } => {
                        if begin == end {
                            return Err(Error::InvalidRange);
                        }
                        w.write_udata(begin, address_size)?;
                        w.write_udata(end, address_size)?;
                    }
                    Range::StartEnd { begin, end } => {
                        if begin == end {
                            return Err(Error::InvalidRange);
                        }
                        w.write_address(begin, address_size)?;
                        w.write_address(end, address_size)?;
                    }
                    Range::StartLength { begin, length } => {
                        let end = match begin {
                            Address::Constant(begin) => Address::Constant(begin + length),
                            Address::Symbol { symbol, addend } => Address::Symbol {
                                symbol,
                                addend: addend + length as i64,
                            },
                        };
                        if begin == end {
                            return Err(Error::InvalidRange);
                        }
                        w.write_address(begin, address_size)?;
                        w.write_address(end, address_size)?;
                    }
                }
            }
            w.write_udata(0, address_size)?;
            w.write_udata(0, address_size)?;
        }
        Ok(RangeListOffsets {
            base_id: self.base_id,
            offsets,
        })
    }

    /// Write the range list table to the `.debug_rnglists` section.
    fn write_rnglists<W: Writer>(
        &self,
        w: &mut DebugRngLists<W>,
        encoding: Encoding,
    ) -> Result<RangeListOffsets> {
        let mut offsets = Vec::new();

        if encoding.version != 5 {
            return Err(Error::NeedVersion(5));
        }

        let length_offset = w.write_initial_length(encoding.format)?;
        let length_base = w.len();

        w.write_u16(encoding.version)?;
        w.write_u8(encoding.address_size)?;
        w.write_u8(0)?; // segment_selector_size
        w.write_u32(0)?; // offset_entry_count (when set to zero DW_FORM_rnglistx can't be used, see section 7.28)
                         // FIXME implement DW_FORM_rnglistx writing and implement the offset entry list

        for range_list in self.ranges.iter() {
            offsets.push(w.offset());
            for range in &range_list.0 {
                match *range {
                    Range::BaseAddress { address } => {
                        w.write_u8(crate::constants::DW_RLE_base_address.0)?;
                        w.write_address(address, encoding.address_size)?;
                    }
                    Range::OffsetPair { begin, end } => {
                        w.write_u8(crate::constants::DW_RLE_offset_pair.0)?;
                        w.write_uleb128(begin)?;
                        w.write_uleb128(end)?;
                    }
                    Range::StartEnd { begin, end } => {
                        w.write_u8(crate::constants::DW_RLE_start_end.0)?;
                        w.write_address(begin, encoding.address_size)?;
                        w.write_address(end, encoding.address_size)?;
                    }
                    Range::StartLength { begin, length } => {
                        w.write_u8(crate::constants::DW_RLE_start_length.0)?;
                        w.write_address(begin, encoding.address_size)?;
                        w.write_uleb128(length)?;
                    }
                }
            }

            w.write_u8(crate::constants::DW_RLE_end_of_list.0)?;
        }

        let length = (w.len() - length_base) as u64;
        w.write_initial_length_at(length_offset, length, encoding.format)?;

        Ok(RangeListOffsets {
            base_id: self.base_id,
            offsets,
        })
    }
}

/// A range list that will be stored in a `.debug_ranges` or `.debug_rnglists` section.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RangeList(pub Vec<Range>);

/// A single range.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Range {
    /// DW_RLE_base_address
    BaseAddress {
        /// Base address.
        address: Address,
    },
    /// DW_RLE_offset_pair
    OffsetPair {
        /// Start of range relative to base address.
        begin: u64,
        /// End of range relative to base address.
        end: u64,
    },
    /// DW_RLE_start_end
    StartEnd {
        /// Start of range.
        begin: Address,
        /// End of range.
        end: Address,
    },
    /// DW_RLE_start_length
    StartLength {
        /// Start of range.
        begin: Address,
        /// Length of range.
        length: u64,
    },
}

#[cfg(feature = "read")]
mod convert {
    use super::*;

    use crate::read::{self, Reader};
    use crate::write::{ConvertError, ConvertResult, ConvertUnitContext};

    impl RangeList {
        /// Create a range list by reading the data from the give range list iter.
        pub(crate) fn from<R: Reader<Offset = usize>>(
            mut from: read::RawRngListIter<R>,
            context: &ConvertUnitContext<'_, R>,
        ) -> ConvertResult<Self> {
            let mut have_base_address = context.base_address != Address::Constant(0);
            let convert_address =
                |x| (context.convert_address)(x).ok_or(ConvertError::InvalidAddress);
            let mut ranges = Vec::new();
            while let Some(from_range) = from.next()? {
                let range = match from_range {
                    read::RawRngListEntry::AddressOrOffsetPair { begin, end } => {
                        // These were parsed as addresses, even if they are offsets.
                        let begin = convert_address(begin)?;
                        let end = convert_address(end)?;
                        match (begin, end) {
                            (Address::Constant(begin_offset), Address::Constant(end_offset)) => {
                                if have_base_address {
                                    Range::OffsetPair {
                                        begin: begin_offset,
                                        end: end_offset,
                                    }
                                } else {
                                    Range::StartEnd { begin, end }
                                }
                            }
                            _ => {
                                if have_base_address {
                                    // At least one of begin/end is an address, but we also have
                                    // a base address. Adding addresses is undefined.
                                    return Err(ConvertError::InvalidRangeRelativeAddress);
                                }
                                Range::StartEnd { begin, end }
                            }
                        }
                    }
                    read::RawRngListEntry::BaseAddress { addr } => {
                        have_base_address = true;
                        let address = convert_address(addr)?;
                        Range::BaseAddress { address }
                    }
                    read::RawRngListEntry::BaseAddressx { addr } => {
                        have_base_address = true;
                        let address = convert_address(context.dwarf.address(context.unit, addr)?)?;
                        Range::BaseAddress { address }
                    }
                    read::RawRngListEntry::StartxEndx { begin, end } => {
                        let begin = convert_address(context.dwarf.address(context.unit, begin)?)?;
                        let end = convert_address(context.dwarf.address(context.unit, end)?)?;
                        Range::StartEnd { begin, end }
                    }
                    read::RawRngListEntry::StartxLength { begin, length } => {
                        let begin = convert_address(context.dwarf.address(context.unit, begin)?)?;
                        Range::StartLength { begin, length }
                    }
                    read::RawRngListEntry::OffsetPair { begin, end } => {
                        Range::OffsetPair { begin, end }
                    }
                    read::RawRngListEntry::StartEnd { begin, end } => {
                        let begin = convert_address(begin)?;
                        let end = convert_address(end)?;
                        Range::StartEnd { begin, end }
                    }
                    read::RawRngListEntry::StartLength { begin, length } => {
                        let begin = convert_address(begin)?;
                        Range::StartLength { begin, length }
                    }
                };
                // Filtering empty ranges out.
                match range {
                    Range::StartLength { length: 0, .. } => continue,
                    Range::StartEnd { begin, end, .. } if begin == end => continue,
                    Range::OffsetPair { begin, end, .. } if begin == end => continue,
                    _ => (),
                }
                ranges.push(range);
            }
            Ok(RangeList(ranges))
        }
    }
}

#[cfg(test)]
#[cfg(feature = "read")]
mod tests {
    use super::*;
    use crate::common::{
        DebugAbbrevOffset, DebugAddrBase, DebugInfoOffset, DebugLocListsBase, DebugRngListsBase,
        DebugStrOffsetsBase, Format,
    };
    use crate::read;
    use crate::write::{
        ConvertUnitContext, EndianVec, LineStringTable, LocationListTable, Range, RangeListTable,
        StringTable,
    };
    use crate::LittleEndian;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_range() {
        let mut line_strings = LineStringTable::default();
        let mut strings = StringTable::default();

        for &version in &[2, 3, 4, 5] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };

                    let mut range_list = RangeList(vec![
                        Range::StartLength {
                            begin: Address::Constant(6666),
                            length: 7777,
                        },
                        Range::StartEnd {
                            begin: Address::Constant(4444),
                            end: Address::Constant(5555),
                        },
                        Range::BaseAddress {
                            address: Address::Constant(1111),
                        },
                        Range::OffsetPair {
                            begin: 2222,
                            end: 3333,
                        },
                    ]);

                    let mut ranges = RangeListTable::default();
                    let range_list_id = ranges.add(range_list.clone());

                    let mut sections = Sections::new(EndianVec::new(LittleEndian));
                    let range_list_offsets = ranges.write(&mut sections, encoding).unwrap();

                    let read_debug_ranges =
                        read::DebugRanges::new(sections.debug_ranges.slice(), LittleEndian);
                    let read_debug_rnglists =
                        read::DebugRngLists::new(sections.debug_rnglists.slice(), LittleEndian);
                    let read_ranges = read::RangeLists::new(read_debug_ranges, read_debug_rnglists);
                    let offset = range_list_offsets.get(range_list_id);
                    let read_range_list = read_ranges.raw_ranges(offset, encoding).unwrap();

                    let dwarf = read::Dwarf {
                        ranges: read_ranges,
                        ..Default::default()
                    };
                    let unit = read::Unit {
                        header: read::UnitHeader::new(
                            encoding,
                            0,
                            read::UnitType::Compilation,
                            DebugAbbrevOffset(0),
                            DebugInfoOffset(0).into(),
                            read::EndianSlice::default(),
                        ),
                        abbreviations: Arc::new(read::Abbreviations::default()),
                        name: None,
                        comp_dir: None,
                        low_pc: 0,
                        str_offsets_base: DebugStrOffsetsBase(0),
                        addr_base: DebugAddrBase(0),
                        loclists_base: DebugLocListsBase(0),
                        rnglists_base: DebugRngListsBase(0),
                        line_program: None,
                        dwo_id: None,
                    };
                    let context = ConvertUnitContext {
                        dwarf: &dwarf,
                        unit: &unit,
                        line_strings: &mut line_strings,
                        strings: &mut strings,
                        ranges: &mut ranges,
                        locations: &mut LocationListTable::default(),
                        convert_address: &|address| Some(Address::Constant(address)),
                        base_address: Address::Constant(0),
                        line_program_offset: None,
                        line_program_files: Vec::new(),
                        entry_ids: &HashMap::new(),
                    };
                    let convert_range_list = RangeList::from(read_range_list, &context).unwrap();

                    if version <= 4 {
                        range_list.0[0] = Range::StartEnd {
                            begin: Address::Constant(6666),
                            end: Address::Constant(6666 + 7777),
                        };
                    }
                    assert_eq!(range_list, convert_range_list);
                }
            }
        }
    }
}
