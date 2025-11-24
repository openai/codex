use alloc::vec::Vec;
use indexmap::IndexSet;
use std::ops::{Deref, DerefMut};

use crate::common::{Encoding, LocationListsOffset, SectionId};
use crate::write::{
    Address, BaseId, DebugInfoReference, Error, Expression, Result, Section, Sections, UnitOffsets,
    Writer,
};

define_section!(
    DebugLoc,
    LocationListsOffset,
    "A writable `.debug_loc` section."
);
define_section!(
    DebugLocLists,
    LocationListsOffset,
    "A writable `.debug_loclists` section."
);

define_offsets!(
    LocationListOffsets: LocationListId => LocationListsOffset,
    "The section offsets of a series of location lists within the `.debug_loc` or `.debug_loclists` sections."
);

define_id!(
    LocationListId,
    "An identifier for a location list in a `LocationListTable`."
);

/// A table of location lists that will be stored in a `.debug_loc` or `.debug_loclists` section.
#[derive(Debug, Default)]
pub struct LocationListTable {
    base_id: BaseId,
    locations: IndexSet<LocationList>,
}

impl LocationListTable {
    /// Add a location list to the table.
    pub fn add(&mut self, loc_list: LocationList) -> LocationListId {
        let (index, _) = self.locations.insert_full(loc_list);
        LocationListId::new(self.base_id, index)
    }

    /// Get a reference to a location list.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    #[inline]
    pub fn get(&self, id: LocationListId) -> &LocationList {
        debug_assert_eq!(self.base_id, id.base_id);
        &self.locations[id.index]
    }

    /// Write the location list table to the appropriate section for the given DWARF version.
    pub(crate) fn write<W: Writer>(
        &self,
        sections: &mut Sections<W>,
        encoding: Encoding,
        unit_offsets: Option<&UnitOffsets>,
    ) -> Result<LocationListOffsets> {
        if self.locations.is_empty() {
            return Ok(LocationListOffsets::none());
        }

        match encoding.version {
            2..=4 => self.write_loc(
                &mut sections.debug_loc,
                &mut sections.debug_loc_refs,
                encoding,
                unit_offsets,
            ),
            5 => self.write_loclists(
                &mut sections.debug_loclists,
                &mut sections.debug_loclists_refs,
                encoding,
                unit_offsets,
            ),
            _ => Err(Error::UnsupportedVersion(encoding.version)),
        }
    }

    /// Write the location list table to the `.debug_loc` section.
    fn write_loc<W: Writer>(
        &self,
        w: &mut DebugLoc<W>,
        refs: &mut Vec<DebugInfoReference>,
        encoding: Encoding,
        unit_offsets: Option<&UnitOffsets>,
    ) -> Result<LocationListOffsets> {
        let address_size = encoding.address_size;
        let mut offsets = Vec::new();
        for loc_list in self.locations.iter() {
            offsets.push(w.offset());
            for loc in &loc_list.0 {
                // Note that we must ensure none of the ranges have both begin == 0 and end == 0.
                // We do this by ensuring that begin != end, which is a bit more restrictive
                // than required, but still seems reasonable.
                match *loc {
                    Location::BaseAddress { address } => {
                        let marker = !0 >> (64 - address_size * 8);
                        w.write_udata(marker, address_size)?;
                        w.write_address(address, address_size)?;
                    }
                    Location::OffsetPair {
                        begin,
                        end,
                        ref data,
                    } => {
                        if begin == end {
                            return Err(Error::InvalidRange);
                        }
                        w.write_udata(begin, address_size)?;
                        w.write_udata(end, address_size)?;
                        write_expression(&mut w.0, refs, encoding, unit_offsets, data)?;
                    }
                    Location::StartEnd {
                        begin,
                        end,
                        ref data,
                    } => {
                        if begin == end {
                            return Err(Error::InvalidRange);
                        }
                        w.write_address(begin, address_size)?;
                        w.write_address(end, address_size)?;
                        write_expression(&mut w.0, refs, encoding, unit_offsets, data)?;
                    }
                    Location::StartLength {
                        begin,
                        length,
                        ref data,
                    } => {
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
                        write_expression(&mut w.0, refs, encoding, unit_offsets, data)?;
                    }
                    Location::DefaultLocation { .. } => {
                        return Err(Error::InvalidRange);
                    }
                }
            }
            w.write_udata(0, address_size)?;
            w.write_udata(0, address_size)?;
        }
        Ok(LocationListOffsets {
            base_id: self.base_id,
            offsets,
        })
    }

    /// Write the location list table to the `.debug_loclists` section.
    fn write_loclists<W: Writer>(
        &self,
        w: &mut DebugLocLists<W>,
        refs: &mut Vec<DebugInfoReference>,
        encoding: Encoding,
        unit_offsets: Option<&UnitOffsets>,
    ) -> Result<LocationListOffsets> {
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

        for loc_list in self.locations.iter() {
            offsets.push(w.offset());
            for loc in &loc_list.0 {
                match *loc {
                    Location::BaseAddress { address } => {
                        w.write_u8(crate::constants::DW_LLE_base_address.0)?;
                        w.write_address(address, encoding.address_size)?;
                    }
                    Location::OffsetPair {
                        begin,
                        end,
                        ref data,
                    } => {
                        w.write_u8(crate::constants::DW_LLE_offset_pair.0)?;
                        w.write_uleb128(begin)?;
                        w.write_uleb128(end)?;
                        write_expression(&mut w.0, refs, encoding, unit_offsets, data)?;
                    }
                    Location::StartEnd {
                        begin,
                        end,
                        ref data,
                    } => {
                        w.write_u8(crate::constants::DW_LLE_start_end.0)?;
                        w.write_address(begin, encoding.address_size)?;
                        w.write_address(end, encoding.address_size)?;
                        write_expression(&mut w.0, refs, encoding, unit_offsets, data)?;
                    }
                    Location::StartLength {
                        begin,
                        length,
                        ref data,
                    } => {
                        w.write_u8(crate::constants::DW_LLE_start_length.0)?;
                        w.write_address(begin, encoding.address_size)?;
                        w.write_uleb128(length)?;
                        write_expression(&mut w.0, refs, encoding, unit_offsets, data)?;
                    }
                    Location::DefaultLocation { ref data } => {
                        w.write_u8(crate::constants::DW_LLE_default_location.0)?;
                        write_expression(&mut w.0, refs, encoding, unit_offsets, data)?;
                    }
                }
            }

            w.write_u8(crate::constants::DW_LLE_end_of_list.0)?;
        }

        let length = (w.len() - length_base) as u64;
        w.write_initial_length_at(length_offset, length, encoding.format)?;

        Ok(LocationListOffsets {
            base_id: self.base_id,
            offsets,
        })
    }
}

/// A locations list that will be stored in a `.debug_loc` or `.debug_loclists` section.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LocationList(pub Vec<Location>);

/// A single location.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Location {
    /// DW_LLE_base_address
    BaseAddress {
        /// Base address.
        address: Address,
    },
    /// DW_LLE_offset_pair
    OffsetPair {
        /// Start of range relative to base address.
        begin: u64,
        /// End of range relative to base address.
        end: u64,
        /// Location description.
        data: Expression,
    },
    /// DW_LLE_start_end
    StartEnd {
        /// Start of range.
        begin: Address,
        /// End of range.
        end: Address,
        /// Location description.
        data: Expression,
    },
    /// DW_LLE_start_length
    StartLength {
        /// Start of range.
        begin: Address,
        /// Length of range.
        length: u64,
        /// Location description.
        data: Expression,
    },
    /// DW_LLE_default_location
    DefaultLocation {
        /// Location description.
        data: Expression,
    },
}

fn write_expression<W: Writer>(
    w: &mut W,
    refs: &mut Vec<DebugInfoReference>,
    encoding: Encoding,
    unit_offsets: Option<&UnitOffsets>,
    val: &Expression,
) -> Result<()> {
    let size = val.size(encoding, unit_offsets) as u64;
    if encoding.version <= 4 {
        w.write_udata(size, 2)?;
    } else {
        w.write_uleb128(size)?;
    }
    val.write(w, Some(refs), encoding, unit_offsets)?;
    Ok(())
}

#[cfg(feature = "read")]
mod convert {
    use super::*;

    use crate::read::{self, Reader};
    use crate::write::{ConvertError, ConvertResult, ConvertUnitContext};

    impl LocationList {
        /// Create a location list by reading the data from the give location list iter.
        pub(crate) fn from<R: Reader<Offset = usize>>(
            mut from: read::RawLocListIter<R>,
            context: &ConvertUnitContext<'_, R>,
        ) -> ConvertResult<Self> {
            let mut have_base_address = context.base_address != Address::Constant(0);
            let convert_address =
                |x| (context.convert_address)(x).ok_or(ConvertError::InvalidAddress);
            let convert_expression = |x| {
                Expression::from(
                    x,
                    context.unit.encoding(),
                    Some(context.dwarf),
                    Some(context.unit),
                    Some(context.entry_ids),
                    context.convert_address,
                )
            };
            let mut loc_list = Vec::new();
            while let Some(from_loc) = from.next()? {
                let loc = match from_loc {
                    read::RawLocListEntry::AddressOrOffsetPair { begin, end, data } => {
                        // These were parsed as addresses, even if they are offsets.
                        let begin = convert_address(begin)?;
                        let end = convert_address(end)?;
                        let data = convert_expression(data)?;
                        match (begin, end) {
                            (Address::Constant(begin_offset), Address::Constant(end_offset)) => {
                                if have_base_address {
                                    Location::OffsetPair {
                                        begin: begin_offset,
                                        end: end_offset,
                                        data,
                                    }
                                } else {
                                    Location::StartEnd { begin, end, data }
                                }
                            }
                            _ => {
                                if have_base_address {
                                    // At least one of begin/end is an address, but we also have
                                    // a base address. Adding addresses is undefined.
                                    return Err(ConvertError::InvalidRangeRelativeAddress);
                                }
                                Location::StartEnd { begin, end, data }
                            }
                        }
                    }
                    read::RawLocListEntry::BaseAddress { addr } => {
                        have_base_address = true;
                        let address = convert_address(addr)?;
                        Location::BaseAddress { address }
                    }
                    read::RawLocListEntry::BaseAddressx { addr } => {
                        have_base_address = true;
                        let address = convert_address(context.dwarf.address(context.unit, addr)?)?;
                        Location::BaseAddress { address }
                    }
                    read::RawLocListEntry::StartxEndx { begin, end, data } => {
                        let begin = convert_address(context.dwarf.address(context.unit, begin)?)?;
                        let end = convert_address(context.dwarf.address(context.unit, end)?)?;
                        let data = convert_expression(data)?;
                        Location::StartEnd { begin, end, data }
                    }
                    read::RawLocListEntry::StartxLength {
                        begin,
                        length,
                        data,
                    } => {
                        let begin = convert_address(context.dwarf.address(context.unit, begin)?)?;
                        let data = convert_expression(data)?;
                        Location::StartLength {
                            begin,
                            length,
                            data,
                        }
                    }
                    read::RawLocListEntry::OffsetPair { begin, end, data } => {
                        let data = convert_expression(data)?;
                        Location::OffsetPair { begin, end, data }
                    }
                    read::RawLocListEntry::StartEnd { begin, end, data } => {
                        let begin = convert_address(begin)?;
                        let end = convert_address(end)?;
                        let data = convert_expression(data)?;
                        Location::StartEnd { begin, end, data }
                    }
                    read::RawLocListEntry::StartLength {
                        begin,
                        length,
                        data,
                    } => {
                        let begin = convert_address(begin)?;
                        let data = convert_expression(data)?;
                        Location::StartLength {
                            begin,
                            length,
                            data,
                        }
                    }
                    read::RawLocListEntry::DefaultLocation { data } => {
                        let data = convert_expression(data)?;
                        Location::DefaultLocation { data }
                    }
                };
                // In some cases, existing data may contain begin == end, filtering
                // these out.
                match loc {
                    Location::StartLength { length: 0, .. } => continue,
                    Location::StartEnd { begin, end, .. } if begin == end => continue,
                    Location::OffsetPair { begin, end, .. } if begin == end => continue,
                    _ => (),
                }
                loc_list.push(loc);
            }
            Ok(LocationList(loc_list))
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
        ConvertUnitContext, EndianVec, LineStringTable, RangeListTable, StringTable,
    };
    use crate::LittleEndian;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_loc_list() {
        let mut line_strings = LineStringTable::default();
        let mut strings = StringTable::default();
        let mut expression = Expression::new();
        expression.op_constu(0);

        for &version in &[2, 3, 4, 5] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };

                    let mut loc_list = LocationList(vec![
                        Location::StartLength {
                            begin: Address::Constant(6666),
                            length: 7777,
                            data: expression.clone(),
                        },
                        Location::StartEnd {
                            begin: Address::Constant(4444),
                            end: Address::Constant(5555),
                            data: expression.clone(),
                        },
                        Location::BaseAddress {
                            address: Address::Constant(1111),
                        },
                        Location::OffsetPair {
                            begin: 2222,
                            end: 3333,
                            data: expression.clone(),
                        },
                    ]);
                    if version >= 5 {
                        loc_list.0.push(Location::DefaultLocation {
                            data: expression.clone(),
                        });
                    }

                    let mut locations = LocationListTable::default();
                    let loc_list_id = locations.add(loc_list.clone());

                    let mut sections = Sections::new(EndianVec::new(LittleEndian));
                    let loc_list_offsets = locations.write(&mut sections, encoding, None).unwrap();
                    assert!(sections.debug_loc_refs.is_empty());
                    assert!(sections.debug_loclists_refs.is_empty());

                    let read_debug_loc =
                        read::DebugLoc::new(sections.debug_loc.slice(), LittleEndian);
                    let read_debug_loclists =
                        read::DebugLocLists::new(sections.debug_loclists.slice(), LittleEndian);
                    let read_loc = read::LocationLists::new(read_debug_loc, read_debug_loclists);
                    let offset = loc_list_offsets.get(loc_list_id);
                    let read_loc_list = read_loc.raw_locations(offset, encoding).unwrap();

                    let dwarf = read::Dwarf {
                        locations: read_loc,
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
                        ranges: &mut RangeListTable::default(),
                        locations: &mut locations,
                        convert_address: &|address| Some(Address::Constant(address)),
                        base_address: Address::Constant(0),
                        line_program_offset: None,
                        line_program_files: Vec::new(),
                        entry_ids: &HashMap::new(),
                    };
                    let convert_loc_list = LocationList::from(read_loc_list, &context).unwrap();

                    if version <= 4 {
                        loc_list.0[0] = Location::StartEnd {
                            begin: Address::Constant(6666),
                            end: Address::Constant(6666 + 7777),
                            data: expression.clone(),
                        };
                    }
                    assert_eq!(loc_list, convert_loc_list);
                }
            }
        }
    }
}
