use alloc::vec::Vec;
use std::ops::{Deref, DerefMut};
use std::slice;

use crate::common::{
    DebugAbbrevOffset, DebugInfoOffset, DebugLineOffset, DebugMacinfoOffset, DebugMacroOffset,
    DebugStrOffset, DebugTypeSignature, Encoding, Format, SectionId,
};
use crate::constants;
use crate::leb128::write::{sleb128_size, uleb128_size};
use crate::write::{
    Abbreviation, AbbreviationTable, Address, AttributeSpecification, BaseId, DebugLineStrOffsets,
    DebugStrOffsets, Error, Expression, FileId, LineProgram, LineStringId, LocationListId,
    LocationListOffsets, LocationListTable, RangeListId, RangeListOffsets, RangeListTable,
    Reference, Result, Section, Sections, StringId, Writer,
};

define_id!(UnitId, "An identifier for a unit in a `UnitTable`.");

define_id!(UnitEntryId, "An identifier for an entry in a `Unit`.");

/// A table of units that will be stored in the `.debug_info` section.
#[derive(Debug, Default)]
pub struct UnitTable {
    base_id: BaseId,
    units: Vec<Unit>,
}

impl UnitTable {
    /// Create a new unit and add it to the table.
    ///
    /// `address_size` must be in bytes.
    ///
    /// Returns the `UnitId` of the new unit.
    #[inline]
    pub fn add(&mut self, unit: Unit) -> UnitId {
        let id = UnitId::new(self.base_id, self.units.len());
        self.units.push(unit);
        id
    }

    /// Return the number of units.
    #[inline]
    pub fn count(&self) -> usize {
        self.units.len()
    }

    /// Return the id of a unit.
    ///
    /// # Panics
    ///
    /// Panics if `index >= self.count()`.
    #[inline]
    pub fn id(&self, index: usize) -> UnitId {
        assert!(index < self.count());
        UnitId::new(self.base_id, index)
    }

    /// Get a reference to a unit.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    #[inline]
    pub fn get(&self, id: UnitId) -> &Unit {
        debug_assert_eq!(self.base_id, id.base_id);
        &self.units[id.index]
    }

    /// Get a mutable reference to a unit.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    #[inline]
    pub fn get_mut(&mut self, id: UnitId) -> &mut Unit {
        debug_assert_eq!(self.base_id, id.base_id);
        &mut self.units[id.index]
    }

    /// Get an iterator for the units.
    pub fn iter(&self) -> impl Iterator<Item = (UnitId, &Unit)> {
        self.units
            .iter()
            .enumerate()
            .map(move |(index, unit)| (UnitId::new(self.base_id, index), unit))
    }

    /// Get a mutable iterator for the units.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (UnitId, &mut Unit)> {
        let base_id = self.base_id;
        self.units
            .iter_mut()
            .enumerate()
            .map(move |(index, unit)| (UnitId::new(base_id, index), unit))
    }

    /// Write the units to the given sections.
    ///
    /// `strings` must contain the `.debug_str` offsets of the corresponding
    /// `StringTable`.
    pub fn write<W: Writer>(
        &mut self,
        sections: &mut Sections<W>,
        line_strings: &DebugLineStrOffsets,
        strings: &DebugStrOffsets,
    ) -> Result<DebugInfoOffsets> {
        let mut offsets = DebugInfoOffsets {
            base_id: self.base_id,
            units: Vec::new(),
        };
        for unit in &mut self.units {
            // TODO: maybe share abbreviation tables
            let abbrev_offset = sections.debug_abbrev.offset();
            let mut abbrevs = AbbreviationTable::default();

            offsets.units.push(unit.write(
                sections,
                abbrev_offset,
                &mut abbrevs,
                line_strings,
                strings,
            )?);

            abbrevs.write(&mut sections.debug_abbrev)?;
        }

        write_section_refs(
            &mut sections.debug_info_refs,
            &mut sections.debug_info.0,
            &offsets,
        )?;
        write_section_refs(
            &mut sections.debug_loc_refs,
            &mut sections.debug_loc.0,
            &offsets,
        )?;
        write_section_refs(
            &mut sections.debug_loclists_refs,
            &mut sections.debug_loclists.0,
            &offsets,
        )?;

        Ok(offsets)
    }
}

fn write_section_refs<W: Writer>(
    references: &mut Vec<DebugInfoReference>,
    w: &mut W,
    offsets: &DebugInfoOffsets,
) -> Result<()> {
    for r in references.drain(..) {
        let entry_offset = offsets.entry(r.unit, r.entry).0;
        debug_assert_ne!(entry_offset, 0);
        w.write_offset_at(r.offset, entry_offset, SectionId::DebugInfo, r.size)?;
    }
    Ok(())
}

/// A unit's debugging information.
#[derive(Debug)]
pub struct Unit {
    base_id: BaseId,
    /// The encoding parameters for this unit.
    encoding: Encoding,
    /// The line number program for this unit.
    pub line_program: LineProgram,
    /// A table of range lists used by this unit.
    pub ranges: RangeListTable,
    /// A table of location lists used by this unit.
    pub locations: LocationListTable,
    /// All entries in this unit. The order is unrelated to the tree order.
    // Requirements:
    // - entries form a tree
    // - entries can be added in any order
    // - entries have a fixed id
    // - able to quickly lookup an entry from its id
    // Limitations of current implementation:
    // - mutable iteration of children is messy due to borrow checker
    entries: Vec<DebuggingInformationEntry>,
    /// The index of the root entry in entries.
    root: UnitEntryId,
}

impl Unit {
    /// Create a new `Unit`.
    pub fn new(encoding: Encoding, line_program: LineProgram) -> Self {
        let base_id = BaseId::default();
        let ranges = RangeListTable::default();
        let locations = LocationListTable::default();
        let mut entries = Vec::new();
        let root = DebuggingInformationEntry::new(
            base_id,
            &mut entries,
            None,
            constants::DW_TAG_compile_unit,
        );
        Unit {
            base_id,
            encoding,
            line_program,
            ranges,
            locations,
            entries,
            root,
        }
    }

    /// Return the encoding parameters for this unit.
    #[inline]
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Return the DWARF version for this unit.
    #[inline]
    pub fn version(&self) -> u16 {
        self.encoding.version
    }

    /// Return the address size in bytes for this unit.
    #[inline]
    pub fn address_size(&self) -> u8 {
        self.encoding.address_size
    }

    /// Return the DWARF format for this unit.
    #[inline]
    pub fn format(&self) -> Format {
        self.encoding.format
    }

    /// Return the number of `DebuggingInformationEntry`s created for this unit.
    ///
    /// This includes entries that no longer have a parent.
    #[inline]
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Return the id of the root entry.
    #[inline]
    pub fn root(&self) -> UnitEntryId {
        self.root
    }

    /// Add a new `DebuggingInformationEntry` to this unit and return its id.
    ///
    /// The `parent` must be within the same unit.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is invalid.
    #[inline]
    pub fn add(&mut self, parent: UnitEntryId, tag: constants::DwTag) -> UnitEntryId {
        debug_assert_eq!(self.base_id, parent.base_id);
        DebuggingInformationEntry::new(self.base_id, &mut self.entries, Some(parent), tag)
    }

    /// Get a reference to an entry.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    #[inline]
    pub fn get(&self, id: UnitEntryId) -> &DebuggingInformationEntry {
        debug_assert_eq!(self.base_id, id.base_id);
        &self.entries[id.index]
    }

    /// Get a mutable reference to an entry.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    #[inline]
    pub fn get_mut(&mut self, id: UnitEntryId) -> &mut DebuggingInformationEntry {
        debug_assert_eq!(self.base_id, id.base_id);
        &mut self.entries[id.index]
    }

    /// Return true if `self.line_program` is used by a DIE.
    fn line_program_in_use(&self) -> bool {
        if self.line_program.is_none() {
            return false;
        }
        if !self.line_program.is_empty() {
            return true;
        }

        for entry in &self.entries {
            for attr in &entry.attrs {
                if let AttributeValue::FileIndex(Some(_)) = attr.value {
                    return true;
                }
            }
        }

        false
    }

    /// Write the unit to the given sections.
    pub(crate) fn write<W: Writer>(
        &mut self,
        sections: &mut Sections<W>,
        abbrev_offset: DebugAbbrevOffset,
        abbrevs: &mut AbbreviationTable,
        line_strings: &DebugLineStrOffsets,
        strings: &DebugStrOffsets,
    ) -> Result<UnitOffsets> {
        let line_program = if self.line_program_in_use() {
            self.entries[self.root.index]
                .set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);
            Some(self.line_program.write(
                &mut sections.debug_line,
                self.encoding,
                line_strings,
                strings,
            )?)
        } else {
            self.entries[self.root.index].delete(constants::DW_AT_stmt_list);
            None
        };

        // TODO: use .debug_types for type units in DWARF v4.
        let w = &mut sections.debug_info;

        let mut offsets = UnitOffsets {
            base_id: self.base_id,
            unit: w.offset(),
            // Entries can be written in any order, so create the complete vec now.
            entries: vec![EntryOffset::none(); self.entries.len()],
        };

        let length_offset = w.write_initial_length(self.format())?;
        let length_base = w.len();

        w.write_u16(self.version())?;
        if 2 <= self.version() && self.version() <= 4 {
            w.write_offset(
                abbrev_offset.0,
                SectionId::DebugAbbrev,
                self.format().word_size(),
            )?;
            w.write_u8(self.address_size())?;
        } else if self.version() == 5 {
            w.write_u8(constants::DW_UT_compile.0)?;
            w.write_u8(self.address_size())?;
            w.write_offset(
                abbrev_offset.0,
                SectionId::DebugAbbrev,
                self.format().word_size(),
            )?;
        } else {
            return Err(Error::UnsupportedVersion(self.version()));
        }

        // Calculate all DIE offsets, so that we are able to output references to them.
        // However, references to base types in expressions use ULEB128, so base types
        // must be moved to the front before we can calculate offsets.
        self.reorder_base_types();
        let mut offset = w.len();
        self.entries[self.root.index].calculate_offsets(
            self,
            &mut offset,
            &mut offsets,
            abbrevs,
        )?;

        let range_lists = self.ranges.write(sections, self.encoding)?;
        // Location lists can't be written until we have DIE offsets.
        let loc_lists = self
            .locations
            .write(sections, self.encoding, Some(&offsets))?;

        let w = &mut sections.debug_info;
        let mut unit_refs = Vec::new();
        self.entries[self.root.index].write(
            w,
            &mut sections.debug_info_refs,
            &mut unit_refs,
            self,
            &mut offsets,
            line_program,
            line_strings,
            strings,
            &range_lists,
            &loc_lists,
        )?;

        let length = (w.len() - length_base) as u64;
        w.write_initial_length_at(length_offset, length, self.format())?;

        for (offset, entry) in unit_refs {
            // This does not need relocation.
            w.write_udata_at(
                offset.0,
                offsets.unit_offset(entry),
                self.format().word_size(),
            )?;
        }

        Ok(offsets)
    }

    /// Reorder base types to come first so that typed stack operations
    /// can get their offset.
    fn reorder_base_types(&mut self) {
        let root = &self.entries[self.root.index];
        let mut root_children = Vec::with_capacity(root.children.len());
        for entry in &root.children {
            if self.entries[entry.index].tag == constants::DW_TAG_base_type {
                root_children.push(*entry);
            }
        }
        for entry in &root.children {
            if self.entries[entry.index].tag != constants::DW_TAG_base_type {
                root_children.push(*entry);
            }
        }
        self.entries[self.root.index].children = root_children;
    }
}

/// A Debugging Information Entry (DIE).
///
/// DIEs have a set of attributes and optionally have children DIEs as well.
///
/// DIEs form a tree without any cycles. This is enforced by specifying the
/// parent when creating a DIE, and disallowing changes of parent.
#[derive(Debug)]
pub struct DebuggingInformationEntry {
    id: UnitEntryId,
    parent: Option<UnitEntryId>,
    tag: constants::DwTag,
    /// Whether to emit `DW_AT_sibling`.
    sibling: bool,
    attrs: Vec<Attribute>,
    children: Vec<UnitEntryId>,
}

impl DebuggingInformationEntry {
    /// Create a new `DebuggingInformationEntry`.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is invalid.
    #[allow(clippy::new_ret_no_self)]
    fn new(
        base_id: BaseId,
        entries: &mut Vec<DebuggingInformationEntry>,
        parent: Option<UnitEntryId>,
        tag: constants::DwTag,
    ) -> UnitEntryId {
        let id = UnitEntryId::new(base_id, entries.len());
        entries.push(DebuggingInformationEntry {
            id,
            parent,
            tag,
            sibling: false,
            attrs: Vec::new(),
            children: Vec::new(),
        });
        if let Some(parent) = parent {
            debug_assert_eq!(base_id, parent.base_id);
            assert_ne!(parent, id);
            entries[parent.index].children.push(id);
        }
        id
    }

    /// Return the id of this entry.
    #[inline]
    pub fn id(&self) -> UnitEntryId {
        self.id
    }

    /// Return the parent of this entry.
    #[inline]
    pub fn parent(&self) -> Option<UnitEntryId> {
        self.parent
    }

    /// Return the tag of this entry.
    #[inline]
    pub fn tag(&self) -> constants::DwTag {
        self.tag
    }

    /// Return `true` if a `DW_AT_sibling` attribute will be emitted.
    #[inline]
    pub fn sibling(&self) -> bool {
        self.sibling
    }

    /// Set whether a `DW_AT_sibling` attribute will be emitted.
    ///
    /// The attribute will only be emitted if the DIE has children.
    #[inline]
    pub fn set_sibling(&mut self, sibling: bool) {
        self.sibling = sibling;
    }

    /// Iterate over the attributes of this entry.
    #[inline]
    pub fn attrs(&self) -> slice::Iter<'_, Attribute> {
        self.attrs.iter()
    }

    /// Iterate over the attributes of this entry for modification.
    #[inline]
    pub fn attrs_mut(&mut self) -> slice::IterMut<'_, Attribute> {
        self.attrs.iter_mut()
    }

    /// Get an attribute.
    pub fn get(&self, name: constants::DwAt) -> Option<&AttributeValue> {
        self.attrs
            .iter()
            .find(|attr| attr.name == name)
            .map(|attr| &attr.value)
    }

    /// Get an attribute for modification.
    pub fn get_mut(&mut self, name: constants::DwAt) -> Option<&mut AttributeValue> {
        self.attrs
            .iter_mut()
            .find(|attr| attr.name == name)
            .map(|attr| &mut attr.value)
    }

    /// Set an attribute.
    ///
    /// Replaces any existing attribute with the same name.
    ///
    /// # Panics
    ///
    /// Panics if `name` is `DW_AT_sibling`. Use `set_sibling` instead.
    pub fn set(&mut self, name: constants::DwAt, value: AttributeValue) {
        assert_ne!(name, constants::DW_AT_sibling);
        if let Some(attr) = self.attrs.iter_mut().find(|attr| attr.name == name) {
            attr.value = value;
            return;
        }
        self.attrs.push(Attribute { name, value });
    }

    /// Delete an attribute.
    ///
    /// Replaces any existing attribute with the same name.
    pub fn delete(&mut self, name: constants::DwAt) {
        self.attrs.retain(|x| x.name != name);
    }

    /// Iterate over the children of this entry.
    ///
    /// Note: use `Unit::add` to add a new child to this entry.
    #[inline]
    pub fn children(&self) -> slice::Iter<'_, UnitEntryId> {
        self.children.iter()
    }

    /// Delete a child entry and all of its children.
    pub fn delete_child(&mut self, id: UnitEntryId) {
        self.children.retain(|&child| child != id);
    }

    /// Return the type abbreviation for this DIE.
    fn abbreviation(&self, encoding: Encoding) -> Result<Abbreviation> {
        let mut attrs = Vec::new();

        if self.sibling && !self.children.is_empty() {
            let form = match encoding.format {
                Format::Dwarf32 => constants::DW_FORM_ref4,
                Format::Dwarf64 => constants::DW_FORM_ref8,
            };
            attrs.push(AttributeSpecification::new(constants::DW_AT_sibling, form));
        }

        for attr in &self.attrs {
            attrs.push(attr.specification(encoding)?);
        }

        Ok(Abbreviation::new(
            self.tag,
            !self.children.is_empty(),
            attrs,
        ))
    }

    fn calculate_offsets(
        &self,
        unit: &Unit,
        offset: &mut usize,
        offsets: &mut UnitOffsets,
        abbrevs: &mut AbbreviationTable,
    ) -> Result<()> {
        offsets.entries[self.id.index].offset = DebugInfoOffset(*offset);
        offsets.entries[self.id.index].abbrev = abbrevs.add(self.abbreviation(unit.encoding())?);
        *offset += self.size(unit, offsets);
        if !self.children.is_empty() {
            for child in &self.children {
                unit.entries[child.index].calculate_offsets(unit, offset, offsets, abbrevs)?;
            }
            // Null child
            *offset += 1;
        }
        Ok(())
    }

    fn size(&self, unit: &Unit, offsets: &UnitOffsets) -> usize {
        let mut size = uleb128_size(offsets.abbrev(self.id));
        if self.sibling && !self.children.is_empty() {
            size += unit.format().word_size() as usize;
        }
        for attr in &self.attrs {
            size += attr.value.size(unit, offsets);
        }
        size
    }

    /// Write the entry to the given sections.
    fn write<W: Writer>(
        &self,
        w: &mut DebugInfo<W>,
        debug_info_refs: &mut Vec<DebugInfoReference>,
        unit_refs: &mut Vec<(DebugInfoOffset, UnitEntryId)>,
        unit: &Unit,
        offsets: &mut UnitOffsets,
        line_program: Option<DebugLineOffset>,
        line_strings: &DebugLineStrOffsets,
        strings: &DebugStrOffsets,
        range_lists: &RangeListOffsets,
        loc_lists: &LocationListOffsets,
    ) -> Result<()> {
        debug_assert_eq!(offsets.debug_info_offset(self.id), w.offset());
        w.write_uleb128(offsets.abbrev(self.id))?;

        let sibling_offset = if self.sibling && !self.children.is_empty() {
            let offset = w.offset();
            w.write_udata(0, unit.format().word_size())?;
            Some(offset)
        } else {
            None
        };

        for attr in &self.attrs {
            attr.value.write(
                w,
                debug_info_refs,
                unit_refs,
                unit,
                offsets,
                line_program,
                line_strings,
                strings,
                range_lists,
                loc_lists,
            )?;
        }

        if !self.children.is_empty() {
            for child in &self.children {
                unit.entries[child.index].write(
                    w,
                    debug_info_refs,
                    unit_refs,
                    unit,
                    offsets,
                    line_program,
                    line_strings,
                    strings,
                    range_lists,
                    loc_lists,
                )?;
            }
            // Null child
            w.write_u8(0)?;
        }

        if let Some(offset) = sibling_offset {
            let next_offset = (w.offset().0 - offsets.unit.0) as u64;
            // This does not need relocation.
            w.write_udata_at(offset.0, next_offset, unit.format().word_size())?;
        }
        Ok(())
    }
}

/// An attribute in a `DebuggingInformationEntry`, consisting of a name and
/// associated value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    name: constants::DwAt,
    value: AttributeValue,
}

impl Attribute {
    /// Get the name of this attribute.
    #[inline]
    pub fn name(&self) -> constants::DwAt {
        self.name
    }

    /// Get the value of this attribute.
    #[inline]
    pub fn get(&self) -> &AttributeValue {
        &self.value
    }

    /// Set the value of this attribute.
    #[inline]
    pub fn set(&mut self, value: AttributeValue) {
        self.value = value;
    }

    /// Return the type specification for this attribute.
    fn specification(&self, encoding: Encoding) -> Result<AttributeSpecification> {
        Ok(AttributeSpecification::new(
            self.name,
            self.value.form(encoding)?,
        ))
    }
}

/// The value of an attribute in a `DebuggingInformationEntry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    /// "Refers to some location in the address space of the described program."
    Address(Address),

    /// A slice of an arbitrary number of bytes.
    Block(Vec<u8>),

    /// A one byte constant data value. How to interpret the byte depends on context.
    ///
    /// From section 7 of the standard: "Depending on context, it may be a
    /// signed integer, an unsigned integer, a floating-point constant, or
    /// anything else."
    Data1(u8),

    /// A two byte constant data value. How to interpret the bytes depends on context.
    ///
    /// This value will be converted to the target endian before writing.
    ///
    /// From section 7 of the standard: "Depending on context, it may be a
    /// signed integer, an unsigned integer, a floating-point constant, or
    /// anything else."
    Data2(u16),

    /// A four byte constant data value. How to interpret the bytes depends on context.
    ///
    /// This value will be converted to the target endian before writing.
    ///
    /// From section 7 of the standard: "Depending on context, it may be a
    /// signed integer, an unsigned integer, a floating-point constant, or
    /// anything else."
    Data4(u32),

    /// An eight byte constant data value. How to interpret the bytes depends on context.
    ///
    /// This value will be converted to the target endian before writing.
    ///
    /// From section 7 of the standard: "Depending on context, it may be a
    /// signed integer, an unsigned integer, a floating-point constant, or
    /// anything else."
    Data8(u64),

    /// A signed integer constant.
    Sdata(i64),

    /// An unsigned integer constant.
    Udata(u64),

    /// "The information bytes contain a DWARF expression (see Section 2.5) or
    /// location description (see Section 2.6)."
    Exprloc(Expression),

    /// A boolean that indicates presence or absence of the attribute.
    Flag(bool),

    /// An attribute that is always present.
    FlagPresent,

    /// A reference to a `DebuggingInformationEntry` in this unit.
    UnitRef(UnitEntryId),

    /// A reference to a `DebuggingInformationEntry` in a potentially different unit.
    DebugInfoRef(Reference),

    /// An offset into the `.debug_info` section of the supplementary object file.
    ///
    /// The API does not currently assist with generating this offset.
    /// This variant will be removed from the API once support for writing
    /// supplementary object files is implemented.
    DebugInfoRefSup(DebugInfoOffset),

    /// A reference to a line number program.
    LineProgramRef,

    /// A reference to a location list.
    LocationListRef(LocationListId),

    /// An offset into the `.debug_macinfo` section.
    ///
    /// The API does not currently assist with generating this offset.
    /// This variant will be removed from the API once support for writing
    /// `.debug_macinfo` sections is implemented.
    DebugMacinfoRef(DebugMacinfoOffset),

    /// An offset into the `.debug_macro` section.
    ///
    /// The API does not currently assist with generating this offset.
    /// This variant will be removed from the API once support for writing
    /// `.debug_macro` sections is implemented.
    DebugMacroRef(DebugMacroOffset),

    /// A reference to a range list.
    RangeListRef(RangeListId),

    /// A type signature.
    ///
    /// The API does not currently assist with generating this signature.
    /// This variant will be removed from the API once support for writing
    /// `.debug_types` sections is implemented.
    DebugTypesRef(DebugTypeSignature),

    /// A reference to a string in the `.debug_str` section.
    StringRef(StringId),

    /// An offset into the `.debug_str` section of the supplementary object file.
    ///
    /// The API does not currently assist with generating this offset.
    /// This variant will be removed from the API once support for writing
    /// supplementary object files is implemented.
    DebugStrRefSup(DebugStrOffset),

    /// A reference to a string in the `.debug_line_str` section.
    LineStringRef(LineStringId),

    /// A slice of bytes representing a string. Must not include null bytes.
    /// Not guaranteed to be UTF-8 or anything like that.
    String(Vec<u8>),

    /// The value of a `DW_AT_encoding` attribute.
    Encoding(constants::DwAte),

    /// The value of a `DW_AT_decimal_sign` attribute.
    DecimalSign(constants::DwDs),

    /// The value of a `DW_AT_endianity` attribute.
    Endianity(constants::DwEnd),

    /// The value of a `DW_AT_accessibility` attribute.
    Accessibility(constants::DwAccess),

    /// The value of a `DW_AT_visibility` attribute.
    Visibility(constants::DwVis),

    /// The value of a `DW_AT_virtuality` attribute.
    Virtuality(constants::DwVirtuality),

    /// The value of a `DW_AT_language` attribute.
    Language(constants::DwLang),

    /// The value of a `DW_AT_address_class` attribute.
    AddressClass(constants::DwAddr),

    /// The value of a `DW_AT_identifier_case` attribute.
    IdentifierCase(constants::DwId),

    /// The value of a `DW_AT_calling_convention` attribute.
    CallingConvention(constants::DwCc),

    /// The value of a `DW_AT_inline` attribute.
    Inline(constants::DwInl),

    /// The value of a `DW_AT_ordering` attribute.
    Ordering(constants::DwOrd),

    /// An index into the filename entries from the line number information
    /// table for the unit containing this value.
    FileIndex(Option<FileId>),
}

impl AttributeValue {
    /// Return the form that will be used to encode this value.
    pub fn form(&self, encoding: Encoding) -> Result<constants::DwForm> {
        // TODO: missing forms:
        // - DW_FORM_indirect
        // - DW_FORM_implicit_const
        // - FW_FORM_block1/block2/block4
        // - DW_FORM_str/strx1/strx2/strx3/strx4
        // - DW_FORM_addrx/addrx1/addrx2/addrx3/addrx4
        // - DW_FORM_data16
        // - DW_FORM_line_strp
        // - DW_FORM_loclistx
        // - DW_FORM_rnglistx
        let form = match *self {
            AttributeValue::Address(_) => constants::DW_FORM_addr,
            AttributeValue::Block(_) => constants::DW_FORM_block,
            AttributeValue::Data1(_) => constants::DW_FORM_data1,
            AttributeValue::Data2(_) => constants::DW_FORM_data2,
            AttributeValue::Data4(_) => constants::DW_FORM_data4,
            AttributeValue::Data8(_) => constants::DW_FORM_data8,
            AttributeValue::Exprloc(_) => constants::DW_FORM_exprloc,
            AttributeValue::Flag(_) => constants::DW_FORM_flag,
            AttributeValue::FlagPresent => constants::DW_FORM_flag_present,
            AttributeValue::UnitRef(_) => {
                // Using a fixed size format lets us write a placeholder before we know
                // the value.
                match encoding.format {
                    Format::Dwarf32 => constants::DW_FORM_ref4,
                    Format::Dwarf64 => constants::DW_FORM_ref8,
                }
            }
            AttributeValue::DebugInfoRef(_) => constants::DW_FORM_ref_addr,
            AttributeValue::DebugInfoRefSup(_) => {
                // TODO: should this depend on the size of supplementary section?
                match encoding.format {
                    Format::Dwarf32 => constants::DW_FORM_ref_sup4,
                    Format::Dwarf64 => constants::DW_FORM_ref_sup8,
                }
            }
            AttributeValue::LineProgramRef
            | AttributeValue::LocationListRef(_)
            | AttributeValue::DebugMacinfoRef(_)
            | AttributeValue::DebugMacroRef(_)
            | AttributeValue::RangeListRef(_) => {
                if encoding.version == 2 || encoding.version == 3 {
                    match encoding.format {
                        Format::Dwarf32 => constants::DW_FORM_data4,
                        Format::Dwarf64 => constants::DW_FORM_data8,
                    }
                } else {
                    constants::DW_FORM_sec_offset
                }
            }
            AttributeValue::DebugTypesRef(_) => constants::DW_FORM_ref_sig8,
            AttributeValue::StringRef(_) => constants::DW_FORM_strp,
            AttributeValue::DebugStrRefSup(_) => constants::DW_FORM_strp_sup,
            AttributeValue::LineStringRef(_) => constants::DW_FORM_line_strp,
            AttributeValue::String(_) => constants::DW_FORM_string,
            AttributeValue::Encoding(_)
            | AttributeValue::DecimalSign(_)
            | AttributeValue::Endianity(_)
            | AttributeValue::Accessibility(_)
            | AttributeValue::Visibility(_)
            | AttributeValue::Virtuality(_)
            | AttributeValue::Language(_)
            | AttributeValue::AddressClass(_)
            | AttributeValue::IdentifierCase(_)
            | AttributeValue::CallingConvention(_)
            | AttributeValue::Inline(_)
            | AttributeValue::Ordering(_)
            | AttributeValue::FileIndex(_)
            | AttributeValue::Udata(_) => constants::DW_FORM_udata,
            AttributeValue::Sdata(_) => constants::DW_FORM_sdata,
        };
        Ok(form)
    }

    fn size(&self, unit: &Unit, offsets: &UnitOffsets) -> usize {
        macro_rules! debug_assert_form {
            ($form:expr) => {
                debug_assert_eq!(self.form(unit.encoding()).unwrap(), $form)
            };
        }
        match *self {
            AttributeValue::Address(_) => {
                debug_assert_form!(constants::DW_FORM_addr);
                unit.address_size() as usize
            }
            AttributeValue::Block(ref val) => {
                debug_assert_form!(constants::DW_FORM_block);
                uleb128_size(val.len() as u64) + val.len()
            }
            AttributeValue::Data1(_) => {
                debug_assert_form!(constants::DW_FORM_data1);
                1
            }
            AttributeValue::Data2(_) => {
                debug_assert_form!(constants::DW_FORM_data2);
                2
            }
            AttributeValue::Data4(_) => {
                debug_assert_form!(constants::DW_FORM_data4);
                4
            }
            AttributeValue::Data8(_) => {
                debug_assert_form!(constants::DW_FORM_data8);
                8
            }
            AttributeValue::Sdata(val) => {
                debug_assert_form!(constants::DW_FORM_sdata);
                sleb128_size(val)
            }
            AttributeValue::Udata(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val)
            }
            AttributeValue::Exprloc(ref val) => {
                debug_assert_form!(constants::DW_FORM_exprloc);
                let size = val.size(unit.encoding(), Some(offsets));
                uleb128_size(size as u64) + size
            }
            AttributeValue::Flag(_) => {
                debug_assert_form!(constants::DW_FORM_flag);
                1
            }
            AttributeValue::FlagPresent => {
                debug_assert_form!(constants::DW_FORM_flag_present);
                0
            }
            AttributeValue::UnitRef(_) => {
                match unit.format() {
                    Format::Dwarf32 => debug_assert_form!(constants::DW_FORM_ref4),
                    Format::Dwarf64 => debug_assert_form!(constants::DW_FORM_ref8),
                }
                unit.format().word_size() as usize
            }
            AttributeValue::DebugInfoRef(_) => {
                debug_assert_form!(constants::DW_FORM_ref_addr);
                if unit.version() == 2 {
                    unit.address_size() as usize
                } else {
                    unit.format().word_size() as usize
                }
            }
            AttributeValue::DebugInfoRefSup(_) => {
                match unit.format() {
                    Format::Dwarf32 => debug_assert_form!(constants::DW_FORM_ref_sup4),
                    Format::Dwarf64 => debug_assert_form!(constants::DW_FORM_ref_sup8),
                }
                unit.format().word_size() as usize
            }
            AttributeValue::LineProgramRef => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                unit.format().word_size() as usize
            }
            AttributeValue::LocationListRef(_) => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                unit.format().word_size() as usize
            }
            AttributeValue::DebugMacinfoRef(_) => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                unit.format().word_size() as usize
            }
            AttributeValue::DebugMacroRef(_) => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                unit.format().word_size() as usize
            }
            AttributeValue::RangeListRef(_) => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                unit.format().word_size() as usize
            }
            AttributeValue::DebugTypesRef(_) => {
                debug_assert_form!(constants::DW_FORM_ref_sig8);
                8
            }
            AttributeValue::StringRef(_) => {
                debug_assert_form!(constants::DW_FORM_strp);
                unit.format().word_size() as usize
            }
            AttributeValue::DebugStrRefSup(_) => {
                debug_assert_form!(constants::DW_FORM_strp_sup);
                unit.format().word_size() as usize
            }
            AttributeValue::LineStringRef(_) => {
                debug_assert_form!(constants::DW_FORM_line_strp);
                unit.format().word_size() as usize
            }
            AttributeValue::String(ref val) => {
                debug_assert_form!(constants::DW_FORM_string);
                val.len() + 1
            }
            AttributeValue::Encoding(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::DecimalSign(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::Endianity(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::Accessibility(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::Visibility(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::Virtuality(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::Language(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::AddressClass(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0)
            }
            AttributeValue::IdentifierCase(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::CallingConvention(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::Inline(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::Ordering(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.0 as u64)
            }
            AttributeValue::FileIndex(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                uleb128_size(val.map(|id| id.raw(unit.version())).unwrap_or(0))
            }
        }
    }

    /// Write the attribute value to the given sections.
    fn write<W: Writer>(
        &self,
        w: &mut DebugInfo<W>,
        debug_info_refs: &mut Vec<DebugInfoReference>,
        unit_refs: &mut Vec<(DebugInfoOffset, UnitEntryId)>,
        unit: &Unit,
        offsets: &UnitOffsets,
        line_program: Option<DebugLineOffset>,
        line_strings: &DebugLineStrOffsets,
        strings: &DebugStrOffsets,
        range_lists: &RangeListOffsets,
        loc_lists: &LocationListOffsets,
    ) -> Result<()> {
        macro_rules! debug_assert_form {
            ($form:expr) => {
                debug_assert_eq!(self.form(unit.encoding()).unwrap(), $form)
            };
        }
        match *self {
            AttributeValue::Address(val) => {
                debug_assert_form!(constants::DW_FORM_addr);
                w.write_address(val, unit.address_size())?;
            }
            AttributeValue::Block(ref val) => {
                debug_assert_form!(constants::DW_FORM_block);
                w.write_uleb128(val.len() as u64)?;
                w.write(val)?;
            }
            AttributeValue::Data1(val) => {
                debug_assert_form!(constants::DW_FORM_data1);
                w.write_u8(val)?;
            }
            AttributeValue::Data2(val) => {
                debug_assert_form!(constants::DW_FORM_data2);
                w.write_u16(val)?;
            }
            AttributeValue::Data4(val) => {
                debug_assert_form!(constants::DW_FORM_data4);
                w.write_u32(val)?;
            }
            AttributeValue::Data8(val) => {
                debug_assert_form!(constants::DW_FORM_data8);
                w.write_u64(val)?;
            }
            AttributeValue::Sdata(val) => {
                debug_assert_form!(constants::DW_FORM_sdata);
                w.write_sleb128(val)?;
            }
            AttributeValue::Udata(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(val)?;
            }
            AttributeValue::Exprloc(ref val) => {
                debug_assert_form!(constants::DW_FORM_exprloc);
                w.write_uleb128(val.size(unit.encoding(), Some(offsets)) as u64)?;
                val.write(
                    &mut w.0,
                    Some(debug_info_refs),
                    unit.encoding(),
                    Some(offsets),
                )?;
            }
            AttributeValue::Flag(val) => {
                debug_assert_form!(constants::DW_FORM_flag);
                w.write_u8(val as u8)?;
            }
            AttributeValue::FlagPresent => {
                debug_assert_form!(constants::DW_FORM_flag_present);
            }
            AttributeValue::UnitRef(id) => {
                match unit.format() {
                    Format::Dwarf32 => debug_assert_form!(constants::DW_FORM_ref4),
                    Format::Dwarf64 => debug_assert_form!(constants::DW_FORM_ref8),
                }
                unit_refs.push((w.offset(), id));
                w.write_udata(0, unit.format().word_size())?;
            }
            AttributeValue::DebugInfoRef(reference) => {
                debug_assert_form!(constants::DW_FORM_ref_addr);
                let size = if unit.version() == 2 {
                    unit.address_size()
                } else {
                    unit.format().word_size()
                };
                match reference {
                    Reference::Symbol(symbol) => w.write_reference(symbol, size)?,
                    Reference::Entry(unit, entry) => {
                        debug_info_refs.push(DebugInfoReference {
                            offset: w.len(),
                            unit,
                            entry,
                            size,
                        });
                        w.write_udata(0, size)?;
                    }
                }
            }
            AttributeValue::DebugInfoRefSup(val) => {
                match unit.format() {
                    Format::Dwarf32 => debug_assert_form!(constants::DW_FORM_ref_sup4),
                    Format::Dwarf64 => debug_assert_form!(constants::DW_FORM_ref_sup8),
                }
                w.write_udata(val.0 as u64, unit.format().word_size())?;
            }
            AttributeValue::LineProgramRef => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                match line_program {
                    Some(line_program) => {
                        w.write_offset(
                            line_program.0,
                            SectionId::DebugLine,
                            unit.format().word_size(),
                        )?;
                    }
                    None => return Err(Error::InvalidAttributeValue),
                }
            }
            AttributeValue::LocationListRef(val) => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                let section = if unit.version() <= 4 {
                    SectionId::DebugLoc
                } else {
                    SectionId::DebugLocLists
                };
                w.write_offset(loc_lists.get(val).0, section, unit.format().word_size())?;
            }
            AttributeValue::DebugMacinfoRef(val) => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                w.write_offset(val.0, SectionId::DebugMacinfo, unit.format().word_size())?;
            }
            AttributeValue::DebugMacroRef(val) => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                w.write_offset(val.0, SectionId::DebugMacro, unit.format().word_size())?;
            }
            AttributeValue::RangeListRef(val) => {
                if unit.version() >= 4 {
                    debug_assert_form!(constants::DW_FORM_sec_offset);
                }
                let section = if unit.version() <= 4 {
                    SectionId::DebugRanges
                } else {
                    SectionId::DebugRngLists
                };
                w.write_offset(range_lists.get(val).0, section, unit.format().word_size())?;
            }
            AttributeValue::DebugTypesRef(val) => {
                debug_assert_form!(constants::DW_FORM_ref_sig8);
                w.write_u64(val.0)?;
            }
            AttributeValue::StringRef(val) => {
                debug_assert_form!(constants::DW_FORM_strp);
                w.write_offset(
                    strings.get(val).0,
                    SectionId::DebugStr,
                    unit.format().word_size(),
                )?;
            }
            AttributeValue::DebugStrRefSup(val) => {
                debug_assert_form!(constants::DW_FORM_strp_sup);
                w.write_udata(val.0 as u64, unit.format().word_size())?;
            }
            AttributeValue::LineStringRef(val) => {
                debug_assert_form!(constants::DW_FORM_line_strp);
                w.write_offset(
                    line_strings.get(val).0,
                    SectionId::DebugLineStr,
                    unit.format().word_size(),
                )?;
            }
            AttributeValue::String(ref val) => {
                debug_assert_form!(constants::DW_FORM_string);
                w.write(val)?;
                w.write_u8(0)?;
            }
            AttributeValue::Encoding(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::DecimalSign(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::Endianity(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::Accessibility(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::Visibility(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::Virtuality(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::Language(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::AddressClass(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(val.0)?;
            }
            AttributeValue::IdentifierCase(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::CallingConvention(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::Inline(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::Ordering(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(u64::from(val.0))?;
            }
            AttributeValue::FileIndex(val) => {
                debug_assert_form!(constants::DW_FORM_udata);
                w.write_uleb128(val.map(|id| id.raw(unit.version())).unwrap_or(0))?;
            }
        }
        Ok(())
    }
}

define_section!(
    DebugInfo,
    DebugInfoOffset,
    "A writable `.debug_info` section."
);

/// The section offsets of all elements within a `.debug_info` section.
#[derive(Debug, Default)]
pub struct DebugInfoOffsets {
    base_id: BaseId,
    units: Vec<UnitOffsets>,
}

impl DebugInfoOffsets {
    /// Get the `.debug_info` section offset for the given unit.
    #[inline]
    pub fn unit(&self, unit: UnitId) -> DebugInfoOffset {
        debug_assert_eq!(self.base_id, unit.base_id);
        self.units[unit.index].unit
    }

    /// Get the `.debug_info` section offset for the given entry.
    #[inline]
    pub fn entry(&self, unit: UnitId, entry: UnitEntryId) -> DebugInfoOffset {
        debug_assert_eq!(self.base_id, unit.base_id);
        self.units[unit.index].debug_info_offset(entry)
    }
}

/// The section offsets of all elements of a unit within a `.debug_info` section.
#[derive(Debug)]
pub(crate) struct UnitOffsets {
    base_id: BaseId,
    unit: DebugInfoOffset,
    entries: Vec<EntryOffset>,
}

impl UnitOffsets {
    /// Get the .debug_info offset for the given entry.
    #[inline]
    pub(crate) fn debug_info_offset(&self, entry: UnitEntryId) -> DebugInfoOffset {
        debug_assert_eq!(self.base_id, entry.base_id);
        let offset = self.entries[entry.index].offset;
        debug_assert_ne!(offset.0, 0);
        offset
    }

    /// Get the unit offset for the given entry.
    #[inline]
    pub(crate) fn unit_offset(&self, entry: UnitEntryId) -> u64 {
        let offset = self.debug_info_offset(entry);
        (offset.0 - self.unit.0) as u64
    }

    /// Get the abbreviation code for the given entry.
    #[inline]
    pub(crate) fn abbrev(&self, entry: UnitEntryId) -> u64 {
        debug_assert_eq!(self.base_id, entry.base_id);
        self.entries[entry.index].abbrev
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EntryOffset {
    offset: DebugInfoOffset,
    abbrev: u64,
}

impl EntryOffset {
    fn none() -> Self {
        EntryOffset {
            offset: DebugInfoOffset(0),
            abbrev: 0,
        }
    }
}

/// A reference to a `.debug_info` entry that has yet to be resolved.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DebugInfoReference {
    /// The offset within the section of the reference.
    pub offset: usize,
    /// The size of the reference.
    pub size: u8,
    /// The unit containing the entry.
    pub unit: UnitId,
    /// The entry being referenced.
    pub entry: UnitEntryId,
}

#[cfg(feature = "read")]
pub(crate) mod convert {
    use super::*;
    use crate::common::{DwoId, UnitSectionOffset};
    use crate::read::{self, Reader};
    use crate::write::{self, ConvertError, ConvertResult, LocationList, RangeList};
    use std::collections::HashMap;

    pub(crate) struct ConvertUnit<R: Reader<Offset = usize>> {
        from_unit: read::Unit<R>,
        base_id: BaseId,
        encoding: Encoding,
        entries: Vec<DebuggingInformationEntry>,
        entry_offsets: Vec<read::UnitOffset>,
        root: UnitEntryId,
    }

    pub(crate) struct ConvertUnitContext<'a, R: Reader<Offset = usize>> {
        pub dwarf: &'a read::Dwarf<R>,
        pub unit: &'a read::Unit<R>,
        pub line_strings: &'a mut write::LineStringTable,
        pub strings: &'a mut write::StringTable,
        pub ranges: &'a mut write::RangeListTable,
        pub locations: &'a mut write::LocationListTable,
        pub convert_address: &'a dyn Fn(u64) -> Option<Address>,
        pub base_address: Address,
        pub line_program_offset: Option<DebugLineOffset>,
        pub line_program_files: Vec<FileId>,
        pub entry_ids: &'a HashMap<UnitSectionOffset, (UnitId, UnitEntryId)>,
    }

    impl UnitTable {
        /// Create a unit table by reading the data in the given sections.
        ///
        /// This also updates the given tables with the values that are referenced from
        /// attributes in this section.
        ///
        /// `convert_address` is a function to convert read addresses into the `Address`
        /// type. For non-relocatable addresses, this function may simply return
        /// `Address::Constant(address)`. For relocatable addresses, it is the caller's
        /// responsibility to determine the symbol and addend corresponding to the address
        /// and return `Address::Symbol { symbol, addend }`.
        pub fn from<R: Reader<Offset = usize>>(
            dwarf: &read::Dwarf<R>,
            line_strings: &mut write::LineStringTable,
            strings: &mut write::StringTable,
            convert_address: &dyn Fn(u64) -> Option<Address>,
        ) -> ConvertResult<UnitTable> {
            let base_id = BaseId::default();
            let mut unit_entries = Vec::new();
            let mut entry_ids = HashMap::new();

            let mut from_units = dwarf.units();
            while let Some(from_unit) = from_units.next()? {
                let unit_id = UnitId::new(base_id, unit_entries.len());
                unit_entries.push(Unit::convert_entries(
                    from_unit,
                    unit_id,
                    &mut entry_ids,
                    dwarf,
                )?);
            }

            // Attributes must be converted in a separate pass so that we can handle
            // references to other compilation units.
            let mut units = Vec::new();
            for unit_entries in unit_entries.drain(..) {
                units.push(Unit::convert_attributes(
                    unit_entries,
                    &entry_ids,
                    dwarf,
                    line_strings,
                    strings,
                    convert_address,
                )?);
            }

            Ok(UnitTable { base_id, units })
        }
    }

    impl Unit {
        /// Create a unit by reading the data in the input sections.
        ///
        /// Does not add entry attributes.
        pub(crate) fn convert_entries<R: Reader<Offset = usize>>(
            from_header: read::UnitHeader<R>,
            unit_id: UnitId,
            entry_ids: &mut HashMap<UnitSectionOffset, (UnitId, UnitEntryId)>,
            dwarf: &read::Dwarf<R>,
        ) -> ConvertResult<ConvertUnit<R>> {
            match from_header.type_() {
                read::UnitType::Compilation => (),
                _ => return Err(ConvertError::UnsupportedUnitType),
            }
            let base_id = BaseId::default();

            let from_unit = dwarf.unit(from_header)?;
            let encoding = from_unit.encoding();

            let mut entries = Vec::new();
            let mut entry_offsets = Vec::new();

            let mut from_tree = from_unit.entries_tree(None)?;
            let from_root = from_tree.root()?;
            let root = DebuggingInformationEntry::convert_entry(
                from_root,
                &from_unit,
                base_id,
                &mut entries,
                &mut entry_offsets,
                entry_ids,
                None,
                unit_id,
            )?;

            Ok(ConvertUnit {
                from_unit,
                base_id,
                encoding,
                entries,
                entry_offsets,
                root,
            })
        }

        /// Create entry attributes by reading the data in the input sections.
        fn convert_attributes<R: Reader<Offset = usize>>(
            unit: ConvertUnit<R>,
            entry_ids: &HashMap<UnitSectionOffset, (UnitId, UnitEntryId)>,
            dwarf: &read::Dwarf<R>,
            line_strings: &mut write::LineStringTable,
            strings: &mut write::StringTable,
            convert_address: &dyn Fn(u64) -> Option<Address>,
        ) -> ConvertResult<Unit> {
            let from_unit = unit.from_unit;
            let base_address =
                convert_address(from_unit.low_pc).ok_or(ConvertError::InvalidAddress)?;

            let (line_program_offset, line_program, line_program_files) =
                match from_unit.line_program {
                    Some(ref from_program) => {
                        let from_program = from_program.clone();
                        let line_program_offset = from_program.header().offset();
                        let (line_program, line_program_files) = LineProgram::from(
                            from_program,
                            dwarf,
                            line_strings,
                            strings,
                            convert_address,
                        )?;
                        (Some(line_program_offset), line_program, line_program_files)
                    }
                    None => (None, LineProgram::none(), Vec::new()),
                };

            let mut ranges = RangeListTable::default();
            let mut locations = LocationListTable::default();

            let mut context = ConvertUnitContext {
                entry_ids,
                dwarf,
                unit: &from_unit,
                line_strings,
                strings,
                ranges: &mut ranges,
                locations: &mut locations,
                convert_address,
                base_address,
                line_program_offset,
                line_program_files,
            };

            let mut entries = unit.entries;
            for entry in &mut entries {
                entry.convert_attributes(&mut context, &unit.entry_offsets)?;
            }

            Ok(Unit {
                base_id: unit.base_id,
                encoding: unit.encoding,
                line_program,
                ranges,
                locations,
                entries,
                root: unit.root,
            })
        }
    }

    impl DebuggingInformationEntry {
        /// Create an entry by reading the data in the input sections.
        ///
        /// Does not add the entry attributes.
        fn convert_entry<R: Reader<Offset = usize>>(
            from: read::EntriesTreeNode<'_, '_, '_, R>,
            from_unit: &read::Unit<R>,
            base_id: BaseId,
            entries: &mut Vec<DebuggingInformationEntry>,
            entry_offsets: &mut Vec<read::UnitOffset>,
            entry_ids: &mut HashMap<UnitSectionOffset, (UnitId, UnitEntryId)>,
            parent: Option<UnitEntryId>,
            unit_id: UnitId,
        ) -> ConvertResult<UnitEntryId> {
            let from_entry = from.entry();
            let id = DebuggingInformationEntry::new(base_id, entries, parent, from_entry.tag());
            let offset = from_entry.offset();
            entry_offsets.push(offset);
            entry_ids.insert(offset.to_unit_section_offset(from_unit), (unit_id, id));

            let mut from_children = from.children();
            while let Some(from_child) = from_children.next()? {
                DebuggingInformationEntry::convert_entry(
                    from_child,
                    from_unit,
                    base_id,
                    entries,
                    entry_offsets,
                    entry_ids,
                    Some(id),
                    unit_id,
                )?;
            }
            Ok(id)
        }

        /// Create an entry's attributes by reading the data in the input sections.
        fn convert_attributes<R: Reader<Offset = usize>>(
            &mut self,
            context: &mut ConvertUnitContext<'_, R>,
            entry_offsets: &[read::UnitOffset],
        ) -> ConvertResult<()> {
            let offset = entry_offsets[self.id.index];
            let from = context.unit.entry(offset)?;
            let mut from_attrs = from.attrs();
            while let Some(from_attr) = from_attrs.next()? {
                if from_attr.name() == constants::DW_AT_sibling {
                    // This may point to a null entry, so we have to treat it differently.
                    self.set_sibling(true);
                } else if let Some(attr) = Attribute::from(context, &from_attr)? {
                    self.set(attr.name, attr.value);
                }
            }
            Ok(())
        }
    }

    impl Attribute {
        /// Create an attribute by reading the data in the given sections.
        pub(crate) fn from<R: Reader<Offset = usize>>(
            context: &mut ConvertUnitContext<'_, R>,
            from: &read::Attribute<R>,
        ) -> ConvertResult<Option<Attribute>> {
            let value = AttributeValue::from(context, from.value())?;
            Ok(value.map(|value| Attribute {
                name: from.name(),
                value,
            }))
        }
    }

    impl AttributeValue {
        /// Create an attribute value by reading the data in the given sections.
        pub(crate) fn from<R: Reader<Offset = usize>>(
            context: &mut ConvertUnitContext<'_, R>,
            from: read::AttributeValue<R>,
        ) -> ConvertResult<Option<AttributeValue>> {
            let to = match from {
                read::AttributeValue::Addr(val) => match (context.convert_address)(val) {
                    Some(val) => AttributeValue::Address(val),
                    None => return Err(ConvertError::InvalidAddress),
                },
                read::AttributeValue::Block(r) => AttributeValue::Block(r.to_slice()?.into()),
                read::AttributeValue::Data1(val) => AttributeValue::Data1(val),
                read::AttributeValue::Data2(val) => AttributeValue::Data2(val),
                read::AttributeValue::Data4(val) => AttributeValue::Data4(val),
                read::AttributeValue::Data8(val) => AttributeValue::Data8(val),
                read::AttributeValue::Sdata(val) => AttributeValue::Sdata(val),
                read::AttributeValue::Udata(val) => AttributeValue::Udata(val),
                read::AttributeValue::Exprloc(expression) => {
                    let expression = Expression::from(
                        expression,
                        context.unit.encoding(),
                        Some(context.dwarf),
                        Some(context.unit),
                        Some(context.entry_ids),
                        context.convert_address,
                    )?;
                    AttributeValue::Exprloc(expression)
                }
                // TODO: it would be nice to preserve the flag form.
                read::AttributeValue::Flag(val) => AttributeValue::Flag(val),
                read::AttributeValue::DebugAddrBase(_base) => {
                    // We convert all address indices to addresses,
                    // so this is unneeded.
                    return Ok(None);
                }
                read::AttributeValue::DebugAddrIndex(index) => {
                    let val = context.dwarf.address(context.unit, index)?;
                    match (context.convert_address)(val) {
                        Some(val) => AttributeValue::Address(val),
                        None => return Err(ConvertError::InvalidAddress),
                    }
                }
                read::AttributeValue::UnitRef(val) => {
                    if !context.unit.header.is_valid_offset(val) {
                        return Err(ConvertError::InvalidUnitRef);
                    }
                    let id = context
                        .entry_ids
                        .get(&val.to_unit_section_offset(context.unit))
                        .ok_or(ConvertError::InvalidUnitRef)?;
                    AttributeValue::UnitRef(id.1)
                }
                read::AttributeValue::DebugInfoRef(val) => {
                    // TODO: support relocation of this value
                    let id = context
                        .entry_ids
                        .get(&UnitSectionOffset::DebugInfoOffset(val))
                        .ok_or(ConvertError::InvalidDebugInfoRef)?;
                    AttributeValue::DebugInfoRef(Reference::Entry(id.0, id.1))
                }
                read::AttributeValue::DebugInfoRefSup(val) => AttributeValue::DebugInfoRefSup(val),
                read::AttributeValue::DebugLineRef(val) => {
                    // There should only be the line program in the CU DIE which we've already
                    // converted, so check if it matches that.
                    if Some(val) == context.line_program_offset {
                        AttributeValue::LineProgramRef
                    } else {
                        return Err(ConvertError::InvalidLineRef);
                    }
                }
                read::AttributeValue::DebugMacinfoRef(val) => AttributeValue::DebugMacinfoRef(val),
                read::AttributeValue::DebugMacroRef(val) => AttributeValue::DebugMacroRef(val),
                read::AttributeValue::LocationListsRef(val) => {
                    let iter = context
                        .dwarf
                        .locations
                        .raw_locations(val, context.unit.encoding())?;
                    let loc_list = LocationList::from(iter, context)?;
                    let loc_id = context.locations.add(loc_list);
                    AttributeValue::LocationListRef(loc_id)
                }
                read::AttributeValue::DebugLocListsBase(_base) => {
                    // We convert all location list indices to offsets,
                    // so this is unneeded.
                    return Ok(None);
                }
                read::AttributeValue::DebugLocListsIndex(index) => {
                    let offset = context.dwarf.locations_offset(context.unit, index)?;
                    let iter = context
                        .dwarf
                        .locations
                        .raw_locations(offset, context.unit.encoding())?;
                    let loc_list = LocationList::from(iter, context)?;
                    let loc_id = context.locations.add(loc_list);
                    AttributeValue::LocationListRef(loc_id)
                }
                read::AttributeValue::RangeListsRef(offset) => {
                    let offset = context.dwarf.ranges_offset_from_raw(context.unit, offset);
                    let iter = context.dwarf.raw_ranges(context.unit, offset)?;
                    let range_list = RangeList::from(iter, context)?;
                    let range_id = context.ranges.add(range_list);
                    AttributeValue::RangeListRef(range_id)
                }
                read::AttributeValue::DebugRngListsBase(_base) => {
                    // We convert all range list indices to offsets,
                    // so this is unneeded.
                    return Ok(None);
                }
                read::AttributeValue::DebugRngListsIndex(index) => {
                    let offset = context.dwarf.ranges_offset(context.unit, index)?;
                    let iter = context
                        .dwarf
                        .ranges
                        .raw_ranges(offset, context.unit.encoding())?;
                    let range_list = RangeList::from(iter, context)?;
                    let range_id = context.ranges.add(range_list);
                    AttributeValue::RangeListRef(range_id)
                }
                read::AttributeValue::DebugTypesRef(val) => AttributeValue::DebugTypesRef(val),
                read::AttributeValue::DebugStrRef(offset) => {
                    let r = context.dwarf.string(offset)?;
                    let id = context.strings.add(r.to_slice()?);
                    AttributeValue::StringRef(id)
                }
                read::AttributeValue::DebugStrRefSup(val) => AttributeValue::DebugStrRefSup(val),
                read::AttributeValue::DebugStrOffsetsBase(_base) => {
                    // We convert all string offsets to `.debug_str` references,
                    // so this is unneeded.
                    return Ok(None);
                }
                read::AttributeValue::DebugStrOffsetsIndex(index) => {
                    let offset = context.dwarf.string_offset(context.unit, index)?;
                    let r = context.dwarf.string(offset)?;
                    let id = context.strings.add(r.to_slice()?);
                    AttributeValue::StringRef(id)
                }
                read::AttributeValue::DebugLineStrRef(offset) => {
                    let r = context.dwarf.line_string(offset)?;
                    let id = context.line_strings.add(r.to_slice()?);
                    AttributeValue::LineStringRef(id)
                }
                read::AttributeValue::String(r) => AttributeValue::String(r.to_slice()?.into()),
                read::AttributeValue::Encoding(val) => AttributeValue::Encoding(val),
                read::AttributeValue::DecimalSign(val) => AttributeValue::DecimalSign(val),
                read::AttributeValue::Endianity(val) => AttributeValue::Endianity(val),
                read::AttributeValue::Accessibility(val) => AttributeValue::Accessibility(val),
                read::AttributeValue::Visibility(val) => AttributeValue::Visibility(val),
                read::AttributeValue::Virtuality(val) => AttributeValue::Virtuality(val),
                read::AttributeValue::Language(val) => AttributeValue::Language(val),
                read::AttributeValue::AddressClass(val) => AttributeValue::AddressClass(val),
                read::AttributeValue::IdentifierCase(val) => AttributeValue::IdentifierCase(val),
                read::AttributeValue::CallingConvention(val) => {
                    AttributeValue::CallingConvention(val)
                }
                read::AttributeValue::Inline(val) => AttributeValue::Inline(val),
                read::AttributeValue::Ordering(val) => AttributeValue::Ordering(val),
                read::AttributeValue::FileIndex(val) => {
                    if val == 0 && context.unit.encoding().version <= 4 {
                        AttributeValue::FileIndex(None)
                    } else {
                        match context.line_program_files.get(val as usize) {
                            Some(id) => AttributeValue::FileIndex(Some(*id)),
                            None => return Err(ConvertError::InvalidFileIndex),
                        }
                    }
                }
                // Should always be a more specific section reference.
                read::AttributeValue::SecOffset(_) => {
                    return Err(ConvertError::InvalidAttributeValue);
                }
                read::AttributeValue::DwoId(DwoId(val)) => AttributeValue::Udata(val),
            };
            Ok(Some(to))
        }
    }
}

#[cfg(test)]
#[cfg(feature = "read")]
mod tests {
    use super::*;
    use crate::common::LineEncoding;
    use crate::constants;
    use crate::read;
    use crate::write::{
        Dwarf, DwarfUnit, EndianVec, LineString, Location, LocationList, Range, RangeList,
    };
    use crate::LittleEndian;
    use std::mem;

    #[test]
    fn test_unit_table() {
        let mut dwarf = Dwarf::new();
        let unit_id1 = dwarf.units.add(Unit::new(
            Encoding {
                version: 4,
                address_size: 8,
                format: Format::Dwarf32,
            },
            LineProgram::none(),
        ));
        let unit2 = dwarf.units.add(Unit::new(
            Encoding {
                version: 2,
                address_size: 4,
                format: Format::Dwarf64,
            },
            LineProgram::none(),
        ));
        let unit3 = dwarf.units.add(Unit::new(
            Encoding {
                version: 5,
                address_size: 4,
                format: Format::Dwarf32,
            },
            LineProgram::none(),
        ));
        assert_eq!(dwarf.units.count(), 3);
        {
            let unit1 = dwarf.units.get_mut(unit_id1);
            assert_eq!(unit1.version(), 4);
            assert_eq!(unit1.address_size(), 8);
            assert_eq!(unit1.format(), Format::Dwarf32);
            assert_eq!(unit1.count(), 1);

            let root_id = unit1.root();
            assert_eq!(root_id, UnitEntryId::new(unit1.base_id, 0));
            {
                let root = unit1.get_mut(root_id);
                assert_eq!(root.id(), root_id);
                assert!(root.parent().is_none());
                assert_eq!(root.tag(), constants::DW_TAG_compile_unit);

                // Test get/get_mut
                assert!(root.get(constants::DW_AT_producer).is_none());
                assert!(root.get_mut(constants::DW_AT_producer).is_none());
                let mut producer = AttributeValue::String(b"root"[..].into());
                root.set(constants::DW_AT_producer, producer.clone());
                assert_eq!(root.get(constants::DW_AT_producer), Some(&producer));
                assert_eq!(root.get_mut(constants::DW_AT_producer), Some(&mut producer));

                // Test attrs
                let mut attrs = root.attrs();
                let attr = attrs.next().unwrap();
                assert_eq!(attr.name(), constants::DW_AT_producer);
                assert_eq!(attr.get(), &producer);
                assert!(attrs.next().is_none());
            }

            let child1 = unit1.add(root_id, constants::DW_TAG_subprogram);
            assert_eq!(child1, UnitEntryId::new(unit1.base_id, 1));
            {
                let child1 = unit1.get_mut(child1);
                assert_eq!(child1.parent(), Some(root_id));

                let tmp = AttributeValue::String(b"tmp"[..].into());
                child1.set(constants::DW_AT_name, tmp.clone());
                assert_eq!(child1.get(constants::DW_AT_name), Some(&tmp));

                // Test attrs_mut
                let name = AttributeValue::StringRef(dwarf.strings.add(&b"child1"[..]));
                {
                    let attr = child1.attrs_mut().next().unwrap();
                    assert_eq!(attr.name(), constants::DW_AT_name);
                    attr.set(name.clone());
                }
                assert_eq!(child1.get(constants::DW_AT_name), Some(&name));
            }

            let child2 = unit1.add(root_id, constants::DW_TAG_subprogram);
            assert_eq!(child2, UnitEntryId::new(unit1.base_id, 2));
            {
                let child2 = unit1.get_mut(child2);
                assert_eq!(child2.parent(), Some(root_id));

                let tmp = AttributeValue::String(b"tmp"[..].into());
                child2.set(constants::DW_AT_name, tmp.clone());
                assert_eq!(child2.get(constants::DW_AT_name), Some(&tmp));

                // Test replace
                let name = AttributeValue::StringRef(dwarf.strings.add(&b"child2"[..]));
                child2.set(constants::DW_AT_name, name.clone());
                assert_eq!(child2.get(constants::DW_AT_name), Some(&name));
            }

            {
                let root = unit1.get(root_id);
                assert_eq!(
                    root.children().cloned().collect::<Vec<_>>(),
                    vec![child1, child2]
                );
            }
        }
        {
            let unit2 = dwarf.units.get(unit2);
            assert_eq!(unit2.version(), 2);
            assert_eq!(unit2.address_size(), 4);
            assert_eq!(unit2.format(), Format::Dwarf64);
            assert_eq!(unit2.count(), 1);

            let root = unit2.root();
            assert_eq!(root, UnitEntryId::new(unit2.base_id, 0));
            let root = unit2.get(root);
            assert_eq!(root.id(), UnitEntryId::new(unit2.base_id, 0));
            assert!(root.parent().is_none());
            assert_eq!(root.tag(), constants::DW_TAG_compile_unit);
        }

        let mut sections = Sections::new(EndianVec::new(LittleEndian));
        dwarf.write(&mut sections).unwrap();

        println!("{:?}", sections.debug_str);
        println!("{:?}", sections.debug_info);
        println!("{:?}", sections.debug_abbrev);

        let read_dwarf = sections.read(LittleEndian);
        let mut read_units = read_dwarf.units();

        {
            let read_unit1 = read_units.next().unwrap().unwrap();
            let unit1 = dwarf.units.get(unit_id1);
            assert_eq!(unit1.version(), read_unit1.version());
            assert_eq!(unit1.address_size(), read_unit1.address_size());
            assert_eq!(unit1.format(), read_unit1.format());

            let read_unit1 = read_dwarf.unit(read_unit1).unwrap();
            let mut read_entries = read_unit1.entries();

            let root = unit1.get(unit1.root());
            {
                let (depth, read_root) = read_entries.next_dfs().unwrap().unwrap();
                assert_eq!(depth, 0);
                assert_eq!(root.tag(), read_root.tag());
                assert!(read_root.has_children());

                let producer = match root.get(constants::DW_AT_producer).unwrap() {
                    AttributeValue::String(ref producer) => &**producer,
                    otherwise => panic!("unexpected {:?}", otherwise),
                };
                assert_eq!(producer, b"root");
                let read_producer = read_root
                    .attr_value(constants::DW_AT_producer)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    read_dwarf
                        .attr_string(&read_unit1, read_producer)
                        .unwrap()
                        .slice(),
                    producer
                );
            }

            let mut children = root.children().cloned();

            {
                let child = children.next().unwrap();
                assert_eq!(child, UnitEntryId::new(unit1.base_id, 1));
                let child = unit1.get(child);
                let (depth, read_child) = read_entries.next_dfs().unwrap().unwrap();
                assert_eq!(depth, 1);
                assert_eq!(child.tag(), read_child.tag());
                assert!(!read_child.has_children());

                let name = match child.get(constants::DW_AT_name).unwrap() {
                    AttributeValue::StringRef(name) => *name,
                    otherwise => panic!("unexpected {:?}", otherwise),
                };
                let name = dwarf.strings.get(name);
                assert_eq!(name, b"child1");
                let read_name = read_child
                    .attr_value(constants::DW_AT_name)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    read_dwarf
                        .attr_string(&read_unit1, read_name)
                        .unwrap()
                        .slice(),
                    name
                );
            }

            {
                let child = children.next().unwrap();
                assert_eq!(child, UnitEntryId::new(unit1.base_id, 2));
                let child = unit1.get(child);
                let (depth, read_child) = read_entries.next_dfs().unwrap().unwrap();
                assert_eq!(depth, 0);
                assert_eq!(child.tag(), read_child.tag());
                assert!(!read_child.has_children());

                let name = match child.get(constants::DW_AT_name).unwrap() {
                    AttributeValue::StringRef(name) => *name,
                    otherwise => panic!("unexpected {:?}", otherwise),
                };
                let name = dwarf.strings.get(name);
                assert_eq!(name, b"child2");
                let read_name = read_child
                    .attr_value(constants::DW_AT_name)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    read_dwarf
                        .attr_string(&read_unit1, read_name)
                        .unwrap()
                        .slice(),
                    name
                );
            }

            assert!(read_entries.next_dfs().unwrap().is_none());
        }

        {
            let read_unit2 = read_units.next().unwrap().unwrap();
            let unit2 = dwarf.units.get(unit2);
            assert_eq!(unit2.version(), read_unit2.version());
            assert_eq!(unit2.address_size(), read_unit2.address_size());
            assert_eq!(unit2.format(), read_unit2.format());

            let abbrevs = read_dwarf.abbreviations(&read_unit2).unwrap();
            let mut read_entries = read_unit2.entries(&abbrevs);

            {
                let root = unit2.get(unit2.root());
                let (depth, read_root) = read_entries.next_dfs().unwrap().unwrap();
                assert_eq!(depth, 0);
                assert_eq!(root.tag(), read_root.tag());
                assert!(!read_root.has_children());
            }

            assert!(read_entries.next_dfs().unwrap().is_none());
        }

        {
            let read_unit3 = read_units.next().unwrap().unwrap();
            let unit3 = dwarf.units.get(unit3);
            assert_eq!(unit3.version(), read_unit3.version());
            assert_eq!(unit3.address_size(), read_unit3.address_size());
            assert_eq!(unit3.format(), read_unit3.format());

            let abbrevs = read_dwarf.abbreviations(&read_unit3).unwrap();
            let mut read_entries = read_unit3.entries(&abbrevs);

            {
                let root = unit3.get(unit3.root());
                let (depth, read_root) = read_entries.next_dfs().unwrap().unwrap();
                assert_eq!(depth, 0);
                assert_eq!(root.tag(), read_root.tag());
                assert!(!read_root.has_children());
            }

            assert!(read_entries.next_dfs().unwrap().is_none());
        }

        assert!(read_units.next().unwrap().is_none());

        let convert_dwarf =
            Dwarf::from(&read_dwarf, &|address| Some(Address::Constant(address))).unwrap();
        assert_eq!(convert_dwarf.units.count(), dwarf.units.count());

        for i in 0..convert_dwarf.units.count() {
            let unit_id = dwarf.units.id(i);
            let unit = dwarf.units.get(unit_id);
            let convert_unit_id = convert_dwarf.units.id(i);
            let convert_unit = convert_dwarf.units.get(convert_unit_id);
            assert_eq!(convert_unit.version(), unit.version());
            assert_eq!(convert_unit.address_size(), unit.address_size());
            assert_eq!(convert_unit.format(), unit.format());
            assert_eq!(convert_unit.count(), unit.count());

            let root = unit.get(unit.root());
            let convert_root = convert_unit.get(convert_unit.root());
            assert_eq!(convert_root.tag(), root.tag());
            for (convert_attr, attr) in convert_root.attrs().zip(root.attrs()) {
                assert_eq!(convert_attr, attr);
            }
        }
    }

    #[test]
    fn test_attribute_value() {
        let string_data = "string data";
        let line_string_data = "line string data";

        let data = vec![1, 2, 3, 4];
        let read_data = read::EndianSlice::new(&[1, 2, 3, 4], LittleEndian);

        let mut expression = Expression::new();
        expression.op_constu(57);
        let read_expression = read::Expression(read::EndianSlice::new(
            &[constants::DW_OP_constu.0, 57],
            LittleEndian,
        ));

        let range = RangeList(vec![Range::StartEnd {
            begin: Address::Constant(0x1234),
            end: Address::Constant(0x2345),
        }]);

        let location = LocationList(vec![Location::StartEnd {
            begin: Address::Constant(0x1234),
            end: Address::Constant(0x2345),
            data: expression.clone(),
        }]);

        for &version in &[2, 3, 4, 5] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };

                    let mut dwarf = Dwarf::new();
                    let unit = dwarf.units.add(Unit::new(encoding, LineProgram::none()));
                    let unit = dwarf.units.get_mut(unit);
                    let loc_id = unit.locations.add(location.clone());
                    let range_id = unit.ranges.add(range.clone());
                    // Create a string with a non-zero id/offset.
                    dwarf.strings.add("dummy string");
                    let string_id = dwarf.strings.add(string_data);
                    dwarf.line_strings.add("dummy line string");
                    let line_string_id = dwarf.line_strings.add(line_string_data);

                    let attributes = &[
                        (
                            constants::DW_AT_name,
                            AttributeValue::Address(Address::Constant(0x1234)),
                            read::AttributeValue::Addr(0x1234),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Block(data.clone()),
                            read::AttributeValue::Block(read_data),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Data1(0x12),
                            read::AttributeValue::Data1(0x12),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Data2(0x1234),
                            read::AttributeValue::Data2(0x1234),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Data4(0x1234),
                            read::AttributeValue::Data4(0x1234),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Data8(0x1234),
                            read::AttributeValue::Data8(0x1234),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Sdata(0x1234),
                            read::AttributeValue::Sdata(0x1234),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Udata(0x1234),
                            read::AttributeValue::Udata(0x1234),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Exprloc(expression.clone()),
                            read::AttributeValue::Exprloc(read_expression),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::Flag(false),
                            read::AttributeValue::Flag(false),
                        ),
                        /*
                        (
                            constants::DW_AT_name,
                            AttributeValue::FlagPresent,
                            read::AttributeValue::Flag(true),
                        ),
                        */
                        (
                            constants::DW_AT_name,
                            AttributeValue::DebugInfoRefSup(DebugInfoOffset(0x1234)),
                            read::AttributeValue::DebugInfoRefSup(DebugInfoOffset(0x1234)),
                        ),
                        (
                            constants::DW_AT_macro_info,
                            AttributeValue::DebugMacinfoRef(DebugMacinfoOffset(0x1234)),
                            read::AttributeValue::SecOffset(0x1234),
                        ),
                        (
                            constants::DW_AT_macros,
                            AttributeValue::DebugMacroRef(DebugMacroOffset(0x1234)),
                            read::AttributeValue::SecOffset(0x1234),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::DebugTypesRef(DebugTypeSignature(0x1234)),
                            read::AttributeValue::DebugTypesRef(DebugTypeSignature(0x1234)),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::DebugStrRefSup(DebugStrOffset(0x1234)),
                            read::AttributeValue::DebugStrRefSup(DebugStrOffset(0x1234)),
                        ),
                        (
                            constants::DW_AT_name,
                            AttributeValue::String(data.clone()),
                            read::AttributeValue::String(read_data),
                        ),
                        (
                            constants::DW_AT_encoding,
                            AttributeValue::Encoding(constants::DwAte(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_decimal_sign,
                            AttributeValue::DecimalSign(constants::DwDs(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_endianity,
                            AttributeValue::Endianity(constants::DwEnd(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_accessibility,
                            AttributeValue::Accessibility(constants::DwAccess(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_visibility,
                            AttributeValue::Visibility(constants::DwVis(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_virtuality,
                            AttributeValue::Virtuality(constants::DwVirtuality(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_language,
                            AttributeValue::Language(constants::DwLang(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_address_class,
                            AttributeValue::AddressClass(constants::DwAddr(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_identifier_case,
                            AttributeValue::IdentifierCase(constants::DwId(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_calling_convention,
                            AttributeValue::CallingConvention(constants::DwCc(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_ordering,
                            AttributeValue::Ordering(constants::DwOrd(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                        (
                            constants::DW_AT_inline,
                            AttributeValue::Inline(constants::DwInl(0x12)),
                            read::AttributeValue::Udata(0x12),
                        ),
                    ];

                    let mut add_attribute = |name, value| {
                        let entry_id = unit.add(unit.root(), constants::DW_TAG_subprogram);
                        let entry = unit.get_mut(entry_id);
                        entry.set(name, value);
                    };
                    for (name, value, _) in attributes {
                        add_attribute(*name, value.clone());
                    }
                    add_attribute(
                        constants::DW_AT_location,
                        AttributeValue::LocationListRef(loc_id),
                    );
                    add_attribute(
                        constants::DW_AT_ranges,
                        AttributeValue::RangeListRef(range_id),
                    );
                    add_attribute(constants::DW_AT_name, AttributeValue::StringRef(string_id));
                    add_attribute(
                        constants::DW_AT_name,
                        AttributeValue::LineStringRef(line_string_id),
                    );

                    let mut sections = Sections::new(EndianVec::new(LittleEndian));
                    dwarf.write(&mut sections).unwrap();

                    let read_dwarf = sections.read(LittleEndian);
                    let mut read_units = read_dwarf.units();
                    let read_unit = read_units.next().unwrap().unwrap();
                    let read_unit = read_dwarf.unit(read_unit).unwrap();
                    let read_unit = read_unit.unit_ref(&read_dwarf);
                    let mut read_entries = read_unit.entries();
                    let (_, _root) = read_entries.next_dfs().unwrap().unwrap();

                    let mut get_attribute = |name| {
                        let (_, entry) = read_entries.next_dfs().unwrap().unwrap();
                        entry.attr(name).unwrap().unwrap()
                    };
                    for (name, _, expect_value) in attributes {
                        let read_value = &get_attribute(*name).raw_value();
                        // read::AttributeValue is invariant in the lifetime of R.
                        // The lifetimes here are all okay, so transmute it.
                        let read_value = unsafe {
                            mem::transmute::<
                                &read::AttributeValue<read::EndianSlice<'_, LittleEndian>>,
                                &read::AttributeValue<read::EndianSlice<'_, LittleEndian>>,
                            >(read_value)
                        };
                        assert_eq!(read_value, expect_value);
                    }

                    let read_attr = get_attribute(constants::DW_AT_location).value();
                    let read::AttributeValue::LocationListsRef(read_loc_offset) = read_attr else {
                        panic!("unexpected {:?}", read_attr);
                    };
                    let mut read_locations = read_unit.locations(read_loc_offset).unwrap();
                    let read_location = read_locations.next().unwrap().unwrap();
                    assert_eq!(read_location.range.begin, 0x1234);
                    assert_eq!(read_location.range.end, 0x2345);
                    assert_eq!(read_location.data, read_expression);

                    let read_attr = get_attribute(constants::DW_AT_ranges).value();
                    let read::AttributeValue::RangeListsRef(read_range_offset) = read_attr else {
                        panic!("unexpected {:?}", read_attr);
                    };
                    let read_range_offset = read_unit.ranges_offset_from_raw(read_range_offset);
                    let mut read_ranges = read_unit.ranges(read_range_offset).unwrap();
                    let read_range = read_ranges.next().unwrap().unwrap();
                    assert_eq!(read_range.begin, 0x1234);
                    assert_eq!(read_range.end, 0x2345);

                    let read_string = get_attribute(constants::DW_AT_name).raw_value();
                    let read::AttributeValue::DebugStrRef(read_string_offset) = read_string else {
                        panic!("unexpected {:?}", read_string);
                    };
                    assert_eq!(
                        read_dwarf.string(read_string_offset).unwrap().slice(),
                        string_data.as_bytes()
                    );

                    let read_line_string = get_attribute(constants::DW_AT_name).raw_value();
                    let read::AttributeValue::DebugLineStrRef(read_line_string_offset) =
                        read_line_string
                    else {
                        panic!("unexpected {:?}", read_line_string);
                    };
                    assert_eq!(
                        read_dwarf
                            .line_string(read_line_string_offset)
                            .unwrap()
                            .slice(),
                        line_string_data.as_bytes()
                    );

                    let convert_dwarf =
                        Dwarf::from(&read_dwarf, &|address| Some(Address::Constant(address)))
                            .unwrap();
                    let convert_unit = convert_dwarf.units.get(convert_dwarf.units.id(0));
                    let convert_root = convert_unit.get(convert_unit.root());
                    let mut convert_entries = convert_root.children();

                    let mut get_convert_attr = |name| {
                        let convert_entry = convert_unit.get(*convert_entries.next().unwrap());
                        convert_entry.get(name).unwrap()
                    };
                    for (name, attr, _) in attributes {
                        let convert_attr = get_convert_attr(*name);
                        assert_eq!(convert_attr, attr);
                    }

                    let convert_attr = get_convert_attr(constants::DW_AT_location);
                    let AttributeValue::LocationListRef(convert_loc_id) = convert_attr else {
                        panic!("unexpected {:?}", convert_attr);
                    };
                    let convert_location = convert_unit.locations.get(*convert_loc_id);
                    assert_eq!(*convert_location, location);

                    let convert_attr = get_convert_attr(constants::DW_AT_ranges);
                    let AttributeValue::RangeListRef(convert_range_id) = convert_attr else {
                        panic!("unexpected {:?}", convert_attr);
                    };
                    let convert_range = convert_unit.ranges.get(*convert_range_id);
                    assert_eq!(*convert_range, range);

                    let convert_attr = get_convert_attr(constants::DW_AT_name);
                    let AttributeValue::StringRef(convert_string_id) = convert_attr else {
                        panic!("unexpected {:?}", convert_attr);
                    };
                    let convert_string = convert_dwarf.strings.get(*convert_string_id);
                    assert_eq!(convert_string, string_data.as_bytes());

                    let convert_attr = get_convert_attr(constants::DW_AT_name);
                    let AttributeValue::LineStringRef(convert_line_string_id) = convert_attr else {
                        panic!("unexpected {:?}", convert_attr);
                    };
                    let convert_line_string =
                        convert_dwarf.line_strings.get(*convert_line_string_id);
                    assert_eq!(convert_line_string, line_string_data.as_bytes());
                }
            }
        }
    }

    #[test]
    fn test_unit_ref() {
        let mut dwarf = Dwarf::new();
        let unit_id1 = dwarf.units.add(Unit::new(
            Encoding {
                version: 4,
                address_size: 8,
                format: Format::Dwarf32,
            },
            LineProgram::none(),
        ));
        assert_eq!(unit_id1, dwarf.units.id(0));
        let unit_id2 = dwarf.units.add(Unit::new(
            Encoding {
                version: 2,
                address_size: 4,
                format: Format::Dwarf64,
            },
            LineProgram::none(),
        ));
        assert_eq!(unit_id2, dwarf.units.id(1));
        let unit1_child1 = UnitEntryId::new(dwarf.units.get(unit_id1).base_id, 1);
        let unit1_child2 = UnitEntryId::new(dwarf.units.get(unit_id1).base_id, 2);
        let unit2_child1 = UnitEntryId::new(dwarf.units.get(unit_id2).base_id, 1);
        let unit2_child2 = UnitEntryId::new(dwarf.units.get(unit_id2).base_id, 2);
        {
            let unit1 = dwarf.units.get_mut(unit_id1);
            let root = unit1.root();
            let child_id1 = unit1.add(root, constants::DW_TAG_subprogram);
            assert_eq!(child_id1, unit1_child1);
            let child_id2 = unit1.add(root, constants::DW_TAG_subprogram);
            assert_eq!(child_id2, unit1_child2);
            {
                let child1 = unit1.get_mut(child_id1);
                child1.set(constants::DW_AT_type, AttributeValue::UnitRef(child_id2));
            }
            {
                let child2 = unit1.get_mut(child_id2);
                child2.set(
                    constants::DW_AT_type,
                    AttributeValue::DebugInfoRef(Reference::Entry(unit_id2, unit2_child1)),
                );
            }
        }
        {
            let unit2 = dwarf.units.get_mut(unit_id2);
            let root = unit2.root();
            let child_id1 = unit2.add(root, constants::DW_TAG_subprogram);
            assert_eq!(child_id1, unit2_child1);
            let child_id2 = unit2.add(root, constants::DW_TAG_subprogram);
            assert_eq!(child_id2, unit2_child2);
            {
                let child1 = unit2.get_mut(child_id1);
                child1.set(constants::DW_AT_type, AttributeValue::UnitRef(child_id2));
            }
            {
                let child2 = unit2.get_mut(child_id2);
                child2.set(
                    constants::DW_AT_type,
                    AttributeValue::DebugInfoRef(Reference::Entry(unit_id1, unit1_child1)),
                );
            }
        }

        let mut sections = Sections::new(EndianVec::new(LittleEndian));
        dwarf.write(&mut sections).unwrap();

        println!("{:?}", sections.debug_info);
        println!("{:?}", sections.debug_abbrev);

        let read_dwarf = sections.read(LittleEndian);
        let mut read_units = read_dwarf.units();

        let read_unit = read_units.next().unwrap().unwrap();
        let abbrevs = read_dwarf.abbreviations(&read_unit).unwrap();
        let mut read_entries = read_unit.entries(&abbrevs);
        let (_, _root) = read_entries.next_dfs().unwrap().unwrap();
        let (_, entry) = read_entries.next_dfs().unwrap().unwrap();
        let read_unit1_child1_attr = entry.attr_value(constants::DW_AT_type).unwrap();
        let read_unit1_child1_section_offset =
            entry.offset().to_debug_info_offset(&read_unit).unwrap();
        let (_, entry) = read_entries.next_dfs().unwrap().unwrap();
        let read_unit1_child2_attr = entry.attr_value(constants::DW_AT_type).unwrap();
        let read_unit1_child2_offset = entry.offset();

        let read_unit = read_units.next().unwrap().unwrap();
        let abbrevs = read_dwarf.abbreviations(&read_unit).unwrap();
        let mut read_entries = read_unit.entries(&abbrevs);
        let (_, _root) = read_entries.next_dfs().unwrap().unwrap();
        let (_, entry) = read_entries.next_dfs().unwrap().unwrap();
        let read_unit2_child1_attr = entry.attr_value(constants::DW_AT_type).unwrap();
        let read_unit2_child1_section_offset =
            entry.offset().to_debug_info_offset(&read_unit).unwrap();
        let (_, entry) = read_entries.next_dfs().unwrap().unwrap();
        let read_unit2_child2_attr = entry.attr_value(constants::DW_AT_type).unwrap();
        let read_unit2_child2_offset = entry.offset();

        assert_eq!(
            read_unit1_child1_attr,
            Some(read::AttributeValue::UnitRef(read_unit1_child2_offset))
        );
        assert_eq!(
            read_unit1_child2_attr,
            Some(read::AttributeValue::DebugInfoRef(
                read_unit2_child1_section_offset
            ))
        );
        assert_eq!(
            read_unit2_child1_attr,
            Some(read::AttributeValue::UnitRef(read_unit2_child2_offset))
        );
        assert_eq!(
            read_unit2_child2_attr,
            Some(read::AttributeValue::DebugInfoRef(
                read_unit1_child1_section_offset
            ))
        );

        let convert_dwarf =
            Dwarf::from(&read_dwarf, &|address| Some(Address::Constant(address))).unwrap();
        let convert_units = &convert_dwarf.units;
        assert_eq!(convert_units.count(), dwarf.units.count());

        for i in 0..convert_units.count() {
            let unit = dwarf.units.get(dwarf.units.id(i));
            let convert_unit = convert_units.get(convert_units.id(i));
            assert_eq!(convert_unit.version(), unit.version());
            assert_eq!(convert_unit.address_size(), unit.address_size());
            assert_eq!(convert_unit.format(), unit.format());
            assert_eq!(convert_unit.count(), unit.count());

            let root = unit.get(unit.root());
            let convert_root = convert_unit.get(convert_unit.root());
            assert_eq!(convert_root.tag(), root.tag());
            for (convert_attr, attr) in convert_root.attrs().zip(root.attrs()) {
                assert_eq!(convert_attr, attr);
            }

            let child1 = unit.get(UnitEntryId::new(unit.base_id, 1));
            let convert_child1 = convert_unit.get(UnitEntryId::new(convert_unit.base_id, 1));
            assert_eq!(convert_child1.tag(), child1.tag());
            for (convert_attr, attr) in convert_child1.attrs().zip(child1.attrs()) {
                assert_eq!(convert_attr.name, attr.name);
                match (convert_attr.value.clone(), attr.value.clone()) {
                    (
                        AttributeValue::DebugInfoRef(Reference::Entry(convert_unit, convert_entry)),
                        AttributeValue::DebugInfoRef(Reference::Entry(unit, entry)),
                    ) => {
                        assert_eq!(convert_unit.index, unit.index);
                        assert_eq!(convert_entry.index, entry.index);
                    }
                    (AttributeValue::UnitRef(convert_id), AttributeValue::UnitRef(id)) => {
                        assert_eq!(convert_id.index, id.index);
                    }
                    (convert_value, value) => assert_eq!(convert_value, value),
                }
            }

            let child2 = unit.get(UnitEntryId::new(unit.base_id, 2));
            let convert_child2 = convert_unit.get(UnitEntryId::new(convert_unit.base_id, 2));
            assert_eq!(convert_child2.tag(), child2.tag());
            for (convert_attr, attr) in convert_child2.attrs().zip(child2.attrs()) {
                assert_eq!(convert_attr.name, attr.name);
                match (convert_attr.value.clone(), attr.value.clone()) {
                    (
                        AttributeValue::DebugInfoRef(Reference::Entry(convert_unit, convert_entry)),
                        AttributeValue::DebugInfoRef(Reference::Entry(unit, entry)),
                    ) => {
                        assert_eq!(convert_unit.index, unit.index);
                        assert_eq!(convert_entry.index, entry.index);
                    }
                    (AttributeValue::UnitRef(convert_id), AttributeValue::UnitRef(id)) => {
                        assert_eq!(convert_id.index, id.index);
                    }
                    (convert_value, value) => assert_eq!(convert_value, value),
                }
            }
        }
    }

    #[test]
    fn test_sibling() {
        fn add_child(
            unit: &mut Unit,
            parent: UnitEntryId,
            tag: constants::DwTag,
            name: &str,
        ) -> UnitEntryId {
            let id = unit.add(parent, tag);
            let child = unit.get_mut(id);
            child.set(constants::DW_AT_name, AttributeValue::String(name.into()));
            child.set_sibling(true);
            id
        }

        fn add_children(unit: &mut Unit) {
            let root = unit.root();
            let child1 = add_child(unit, root, constants::DW_TAG_subprogram, "child1");
            add_child(unit, child1, constants::DW_TAG_variable, "grandchild1");
            add_child(unit, root, constants::DW_TAG_subprogram, "child2");
            add_child(unit, root, constants::DW_TAG_subprogram, "child3");
        }

        fn next_child<R: read::Reader<Offset = usize>>(
            entries: &mut read::EntriesCursor<'_, '_, R>,
        ) -> (read::UnitOffset, Option<read::UnitOffset>) {
            let (_, entry) = entries.next_dfs().unwrap().unwrap();
            let offset = entry.offset();
            let sibling =
                entry
                    .attr_value(constants::DW_AT_sibling)
                    .unwrap()
                    .map(|attr| match attr {
                        read::AttributeValue::UnitRef(offset) => offset,
                        _ => panic!("bad sibling value"),
                    });
            (offset, sibling)
        }

        fn check_sibling<R: read::Reader<Offset = usize>>(
            unit: read::UnitHeader<R>,
            dwarf: &read::Dwarf<R>,
        ) {
            let unit = dwarf.unit(unit).unwrap();
            let mut entries = unit.entries();
            // root
            entries.next_dfs().unwrap().unwrap();
            // child1
            let (_, sibling1) = next_child(&mut entries);
            // grandchild1
            entries.next_dfs().unwrap().unwrap();
            // child2
            let (offset2, sibling2) = next_child(&mut entries);
            // child3
            let (_, _) = next_child(&mut entries);
            assert_eq!(sibling1, Some(offset2));
            assert_eq!(sibling2, None);
        }

        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 4,
            address_size: 8,
        };
        let mut dwarf = Dwarf::new();
        let unit_id1 = dwarf.units.add(Unit::new(encoding, LineProgram::none()));
        add_children(dwarf.units.get_mut(unit_id1));
        let unit_id2 = dwarf.units.add(Unit::new(encoding, LineProgram::none()));
        add_children(dwarf.units.get_mut(unit_id2));

        let mut sections = Sections::new(EndianVec::new(LittleEndian));
        dwarf.write(&mut sections).unwrap();

        println!("{:?}", sections.debug_info);
        println!("{:?}", sections.debug_abbrev);

        let read_dwarf = sections.read(LittleEndian);
        let mut read_units = read_dwarf.units();
        check_sibling(read_units.next().unwrap().unwrap(), &read_dwarf);
        check_sibling(read_units.next().unwrap().unwrap(), &read_dwarf);
    }

    #[test]
    fn test_line_ref() {
        let dir_bytes = b"dir";
        let file_bytes1 = b"file1";
        let file_bytes2 = b"file2";
        let file_string1 = LineString::String(file_bytes1.to_vec());
        let file_string2 = LineString::String(file_bytes2.to_vec());

        for &version in &[2, 3, 4, 5] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };

                    // The line program we'll be referencing.
                    let mut line_program = LineProgram::new(
                        encoding,
                        LineEncoding::default(),
                        LineString::String(dir_bytes.to_vec()),
                        file_string1.clone(),
                        None,
                    );
                    let dir = line_program.default_directory();
                    // For version >= 5, this will reuse the existing file at index 0.
                    let file1 = line_program.add_file(file_string1.clone(), dir, None);
                    let file2 = line_program.add_file(file_string2.clone(), dir, None);

                    let mut unit = Unit::new(encoding, line_program);
                    let root = unit.get_mut(unit.root());
                    root.set(
                        constants::DW_AT_name,
                        AttributeValue::String(file_bytes1.to_vec()),
                    );
                    root.set(
                        constants::DW_AT_comp_dir,
                        AttributeValue::String(dir_bytes.to_vec()),
                    );
                    root.set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);

                    let child = unit.add(unit.root(), constants::DW_TAG_subprogram);
                    unit.get_mut(child).set(
                        constants::DW_AT_decl_file,
                        AttributeValue::FileIndex(Some(file1)),
                    );

                    let child = unit.add(unit.root(), constants::DW_TAG_subprogram);
                    unit.get_mut(child).set(
                        constants::DW_AT_call_file,
                        AttributeValue::FileIndex(Some(file2)),
                    );

                    let mut dwarf = Dwarf::new();
                    dwarf.units.add(unit);

                    let mut sections = Sections::new(EndianVec::new(LittleEndian));
                    dwarf.write(&mut sections).unwrap();

                    let read_dwarf = sections.read(LittleEndian);
                    let mut read_units = read_dwarf.units();
                    let read_unit = read_units.next().unwrap().unwrap();
                    let read_unit = read_dwarf.unit(read_unit).unwrap();
                    let read_unit = read_unit.unit_ref(&read_dwarf);
                    let read_line_program = read_unit.line_program.as_ref().unwrap().header();
                    let mut read_entries = read_unit.entries();
                    let (_, _root) = read_entries.next_dfs().unwrap().unwrap();

                    let mut get_path = |name| {
                        let (_, entry) = read_entries.next_dfs().unwrap().unwrap();
                        let read_attr = entry.attr(name).unwrap().unwrap();
                        let read::AttributeValue::FileIndex(read_file_index) = read_attr.value()
                        else {
                            panic!("unexpected {:?}", read_attr);
                        };
                        let read_file = read_line_program.file(read_file_index).unwrap();
                        let read_path = read_unit
                            .attr_string(read_file.path_name())
                            .unwrap()
                            .slice();
                        (read_file_index, read_path)
                    };

                    let (read_index, read_path) = get_path(constants::DW_AT_decl_file);
                    assert_eq!(read_index, if version >= 5 { 0 } else { 1 });
                    assert_eq!(read_path, file_bytes1);

                    let (read_index, read_path) = get_path(constants::DW_AT_call_file);
                    assert_eq!(read_index, if version >= 5 { 1 } else { 2 });
                    assert_eq!(read_path, file_bytes2);

                    let convert_dwarf =
                        Dwarf::from(&read_dwarf, &|address| Some(Address::Constant(address)))
                            .unwrap();
                    let convert_unit = convert_dwarf.units.get(convert_dwarf.units.id(0));
                    let convert_root = convert_unit.get(convert_unit.root());
                    let mut convert_entries = convert_root.children();

                    let mut get_convert_path = |name| {
                        let convert_entry = convert_unit.get(*convert_entries.next().unwrap());
                        let convert_attr = convert_entry.get(name).unwrap();
                        let AttributeValue::FileIndex(Some(convert_file_index)) = convert_attr
                        else {
                            panic!("unexpected {:?}", convert_attr);
                        };
                        convert_unit.line_program.get_file(*convert_file_index).0
                    };

                    let convert_path = get_convert_path(constants::DW_AT_decl_file);
                    assert_eq!(convert_path, &file_string1);

                    let convert_path = get_convert_path(constants::DW_AT_call_file);
                    assert_eq!(convert_path, &file_string2);
                }
            }
        }
    }

    #[test]
    fn test_line_program_used() {
        for used in [false, true] {
            let encoding = Encoding {
                format: Format::Dwarf32,
                version: 5,
                address_size: 8,
            };

            let line_program = LineProgram::new(
                encoding,
                LineEncoding::default(),
                LineString::String(b"comp_dir".to_vec()),
                LineString::String(b"comp_name".to_vec()),
                None,
            );

            let mut unit = Unit::new(encoding, line_program);
            let file_id = if used { Some(FileId::new(0)) } else { None };
            let root = unit.root();
            unit.get_mut(root).set(
                constants::DW_AT_decl_file,
                AttributeValue::FileIndex(file_id),
            );

            let mut dwarf = Dwarf::new();
            dwarf.units.add(unit);

            let mut sections = Sections::new(EndianVec::new(LittleEndian));
            dwarf.write(&mut sections).unwrap();
            assert_eq!(!used, sections.debug_line.slice().is_empty());
        }
    }

    #[test]
    fn test_delete_child() {
        fn set_name(unit: &mut Unit, id: UnitEntryId, name: &str) {
            let entry = unit.get_mut(id);
            entry.set(constants::DW_AT_name, AttributeValue::String(name.into()));
        }
        fn check_name<R: read::Reader>(
            entry: &read::DebuggingInformationEntry<'_, '_, R>,
            unit: read::UnitRef<'_, R>,
            name: &str,
        ) {
            let name_attr = entry.attr(constants::DW_AT_name).unwrap().unwrap();
            let entry_name = unit.attr_string(name_attr.value()).unwrap();
            let entry_name_str = entry_name.to_string().unwrap();
            assert_eq!(entry_name_str, name);
        }
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 4,
            address_size: 8,
        };
        let mut dwarf = DwarfUnit::new(encoding);
        let root = dwarf.unit.root();

        // Add and delete entries in the root unit
        let child1 = dwarf.unit.add(root, constants::DW_TAG_subprogram);
        set_name(&mut dwarf.unit, child1, "child1");
        let grandchild1 = dwarf.unit.add(child1, constants::DW_TAG_variable);
        set_name(&mut dwarf.unit, grandchild1, "grandchild1");
        let child2 = dwarf.unit.add(root, constants::DW_TAG_subprogram);
        set_name(&mut dwarf.unit, child2, "child2");
        // This deletes both `child1` and its child `grandchild1`
        dwarf.unit.get_mut(root).delete_child(child1);
        let child3 = dwarf.unit.add(root, constants::DW_TAG_subprogram);
        set_name(&mut dwarf.unit, child3, "child3");
        let child4 = dwarf.unit.add(root, constants::DW_TAG_subprogram);
        set_name(&mut dwarf.unit, child4, "child4");
        let grandchild4 = dwarf.unit.add(child4, constants::DW_TAG_variable);
        set_name(&mut dwarf.unit, grandchild4, "grandchild4");
        dwarf.unit.get_mut(child4).delete_child(grandchild4);

        let mut sections = Sections::new(EndianVec::new(LittleEndian));

        // Write DWARF data which should only include `child2`, `child3` and `child4`
        dwarf.write(&mut sections).unwrap();

        let read_dwarf = sections.read(LittleEndian);
        let read_unit = read_dwarf.units().next().unwrap().unwrap();
        let read_unit = read_dwarf.unit(read_unit).unwrap();
        let read_unit = read_unit.unit_ref(&read_dwarf);
        let mut entries = read_unit.entries();
        // root
        entries.next_dfs().unwrap().unwrap();
        // child2
        let (_, read_child2) = entries.next_dfs().unwrap().unwrap();
        check_name(read_child2, read_unit, "child2");
        // child3
        let (_, read_child3) = entries.next_dfs().unwrap().unwrap();
        check_name(read_child3, read_unit, "child3");
        // child4
        let (_, read_child4) = entries.next_dfs().unwrap().unwrap();
        check_name(read_child4, read_unit, "child4");
        // There should be no more entries
        assert!(entries.next_dfs().unwrap().is_none());
    }
}
