use alloc::string::String;
use alloc::sync::Arc;

use crate::common::{
    DebugAddrBase, DebugAddrIndex, DebugInfoOffset, DebugLineStrOffset, DebugLocListsBase,
    DebugLocListsIndex, DebugRngListsBase, DebugRngListsIndex, DebugStrOffset, DebugStrOffsetsBase,
    DebugStrOffsetsIndex, DebugTypeSignature, DebugTypesOffset, DwarfFileType, DwoId, Encoding,
    LocationListsOffset, RangeListsOffset, RawRangeListsOffset, SectionId, UnitSectionOffset,
};
use crate::constants;
use crate::read::{
    Abbreviations, AbbreviationsCache, AbbreviationsCacheStrategy, AttributeValue, DebugAbbrev,
    DebugAddr, DebugAranges, DebugCuIndex, DebugInfo, DebugInfoUnitHeadersIter, DebugLine,
    DebugLineStr, DebugLoc, DebugLocLists, DebugRanges, DebugRngLists, DebugStr, DebugStrOffsets,
    DebugTuIndex, DebugTypes, DebugTypesUnitHeadersIter, DebuggingInformationEntry, EntriesCursor,
    EntriesRaw, EntriesTree, Error, IncompleteLineProgram, IndexSectionId, LocListIter,
    LocationLists, Range, RangeLists, RawLocListIter, RawRngListIter, Reader, ReaderOffset,
    ReaderOffsetId, Result, RngListIter, Section, UnitHeader, UnitIndex, UnitIndexSectionIterator,
    UnitOffset, UnitType,
};

/// All of the commonly used DWARF sections.
///
/// This is useful for storing sections when `T` does not implement `Reader`.
/// It can be used to create a `Dwarf` that references the data in `self`.
/// If `T` does implement `Reader`, then use `Dwarf` directly.
///
/// ## Example Usage
///
/// It can be useful to load DWARF sections into owned data structures,
/// such as `Vec`. However, we do not implement the `Reader` trait
/// for `Vec`, because it would be very inefficient, but this trait
/// is required for all of the methods that parse the DWARF data.
/// So we first load the DWARF sections into `Vec`s, and then use
/// `borrow` to create `Reader`s that reference the data.
///
/// ```rust,no_run
/// # fn example() -> Result<(), gimli::Error> {
/// # let loader = |name| -> Result<_, gimli::Error> { unimplemented!() };
/// // Read the DWARF sections into `Vec`s with whatever object loader you're using.
/// let dwarf_sections: gimli::DwarfSections<Vec<u8>> = gimli::DwarfSections::load(loader)?;
/// // Create references to the DWARF sections.
/// let dwarf: gimli::Dwarf<_> = dwarf_sections.borrow(|section| {
///     gimli::EndianSlice::new(&section, gimli::LittleEndian)
/// });
/// # unreachable!()
/// # }
/// ```
#[derive(Debug, Default)]
pub struct DwarfSections<T> {
    /// The `.debug_abbrev` section.
    pub debug_abbrev: DebugAbbrev<T>,
    /// The `.debug_addr` section.
    pub debug_addr: DebugAddr<T>,
    /// The `.debug_aranges` section.
    pub debug_aranges: DebugAranges<T>,
    /// The `.debug_info` section.
    pub debug_info: DebugInfo<T>,
    /// The `.debug_line` section.
    pub debug_line: DebugLine<T>,
    /// The `.debug_line_str` section.
    pub debug_line_str: DebugLineStr<T>,
    /// The `.debug_str` section.
    pub debug_str: DebugStr<T>,
    /// The `.debug_str_offsets` section.
    pub debug_str_offsets: DebugStrOffsets<T>,
    /// The `.debug_types` section.
    pub debug_types: DebugTypes<T>,
    /// The `.debug_loc` section.
    pub debug_loc: DebugLoc<T>,
    /// The `.debug_loclists` section.
    pub debug_loclists: DebugLocLists<T>,
    /// The `.debug_ranges` section.
    pub debug_ranges: DebugRanges<T>,
    /// The `.debug_rnglists` section.
    pub debug_rnglists: DebugRngLists<T>,
}

impl<T> DwarfSections<T> {
    /// Try to load the DWARF sections using the given loader function.
    ///
    /// `section` loads a DWARF section from the object file.
    /// It should return an empty section if the section does not exist.
    pub fn load<F, E>(mut section: F) -> core::result::Result<Self, E>
    where
        F: FnMut(SectionId) -> core::result::Result<T, E>,
    {
        Ok(DwarfSections {
            // Section types are inferred.
            debug_abbrev: Section::load(&mut section)?,
            debug_addr: Section::load(&mut section)?,
            debug_aranges: Section::load(&mut section)?,
            debug_info: Section::load(&mut section)?,
            debug_line: Section::load(&mut section)?,
            debug_line_str: Section::load(&mut section)?,
            debug_str: Section::load(&mut section)?,
            debug_str_offsets: Section::load(&mut section)?,
            debug_types: Section::load(&mut section)?,
            debug_loc: Section::load(&mut section)?,
            debug_loclists: Section::load(&mut section)?,
            debug_ranges: Section::load(&mut section)?,
            debug_rnglists: Section::load(&mut section)?,
        })
    }

    /// Create a `Dwarf` structure that references the data in `self`.
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F) -> Dwarf<R>
    where
        F: FnMut(&'a T) -> R,
    {
        Dwarf::from_sections(DwarfSections {
            debug_abbrev: self.debug_abbrev.borrow(&mut borrow),
            debug_addr: self.debug_addr.borrow(&mut borrow),
            debug_aranges: self.debug_aranges.borrow(&mut borrow),
            debug_info: self.debug_info.borrow(&mut borrow),
            debug_line: self.debug_line.borrow(&mut borrow),
            debug_line_str: self.debug_line_str.borrow(&mut borrow),
            debug_str: self.debug_str.borrow(&mut borrow),
            debug_str_offsets: self.debug_str_offsets.borrow(&mut borrow),
            debug_types: self.debug_types.borrow(&mut borrow),
            debug_loc: self.debug_loc.borrow(&mut borrow),
            debug_loclists: self.debug_loclists.borrow(&mut borrow),
            debug_ranges: self.debug_ranges.borrow(&mut borrow),
            debug_rnglists: self.debug_rnglists.borrow(&mut borrow),
        })
    }

    /// Create a `Dwarf` structure that references the data in `self` and `sup`.
    ///
    /// This is like `borrow`, but also includes the supplementary object file.
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// ## Example Usage
    ///
    /// ```rust,no_run
    /// # fn example() -> Result<(), gimli::Error> {
    /// # let loader = |name| -> Result<_, gimli::Error> { unimplemented!() };
    /// # let sup_loader = |name| -> Result<_, gimli::Error> { unimplemented!() };
    /// // Read the DWARF sections into `Vec`s with whatever object loader you're using.
    /// let dwarf_sections: gimli::DwarfSections<Vec<u8>> = gimli::DwarfSections::load(loader)?;
    /// let dwarf_sup_sections: gimli::DwarfSections<Vec<u8>> = gimli::DwarfSections::load(sup_loader)?;
    /// // Create references to the DWARF sections.
    /// let dwarf = dwarf_sections.borrow_with_sup(&dwarf_sup_sections, |section| {
    ///     gimli::EndianSlice::new(&section, gimli::LittleEndian)
    /// });
    /// # unreachable!()
    /// # }
    /// ```
    pub fn borrow_with_sup<'a, F, R>(&'a self, sup: &'a Self, mut borrow: F) -> Dwarf<R>
    where
        F: FnMut(&'a T) -> R,
    {
        let mut dwarf = self.borrow(&mut borrow);
        dwarf.set_sup(sup.borrow(&mut borrow));
        dwarf
    }
}

/// All of the commonly used DWARF sections, and other common information.
#[derive(Debug, Default)]
pub struct Dwarf<R> {
    /// The `.debug_abbrev` section.
    pub debug_abbrev: DebugAbbrev<R>,

    /// The `.debug_addr` section.
    pub debug_addr: DebugAddr<R>,

    /// The `.debug_aranges` section.
    pub debug_aranges: DebugAranges<R>,

    /// The `.debug_info` section.
    pub debug_info: DebugInfo<R>,

    /// The `.debug_line` section.
    pub debug_line: DebugLine<R>,

    /// The `.debug_line_str` section.
    pub debug_line_str: DebugLineStr<R>,

    /// The `.debug_str` section.
    pub debug_str: DebugStr<R>,

    /// The `.debug_str_offsets` section.
    pub debug_str_offsets: DebugStrOffsets<R>,

    /// The `.debug_types` section.
    pub debug_types: DebugTypes<R>,

    /// The location lists in the `.debug_loc` and `.debug_loclists` sections.
    pub locations: LocationLists<R>,

    /// The range lists in the `.debug_ranges` and `.debug_rnglists` sections.
    pub ranges: RangeLists<R>,

    /// The type of this file.
    pub file_type: DwarfFileType,

    /// The DWARF sections for a supplementary object file.
    pub sup: Option<Arc<Dwarf<R>>>,

    /// A cache of previously parsed abbreviations for units in this file.
    pub abbreviations_cache: AbbreviationsCache,
}

impl<T> Dwarf<T> {
    /// Try to load the DWARF sections using the given loader function.
    ///
    /// `section` loads a DWARF section from the object file.
    /// It should return an empty section if the section does not exist.
    ///
    /// After loading, the user should set the `file_type` field and
    /// call `load_sup` if required.
    pub fn load<F, E>(section: F) -> core::result::Result<Self, E>
    where
        F: FnMut(SectionId) -> core::result::Result<T, E>,
    {
        let sections = DwarfSections::load(section)?;
        Ok(Self::from_sections(sections))
    }

    /// Load the DWARF sections from the supplementary object file.
    ///
    /// `section` operates the same as for `load`.
    ///
    /// Sets `self.sup`, replacing any previous value.
    pub fn load_sup<F, E>(&mut self, section: F) -> core::result::Result<(), E>
    where
        F: FnMut(SectionId) -> core::result::Result<T, E>,
    {
        self.set_sup(Self::load(section)?);
        Ok(())
    }

    /// Create a `Dwarf` structure from the given sections.
    ///
    /// The caller should set the `file_type` and `sup` fields if required.
    fn from_sections(sections: DwarfSections<T>) -> Self {
        Dwarf {
            debug_abbrev: sections.debug_abbrev,
            debug_addr: sections.debug_addr,
            debug_aranges: sections.debug_aranges,
            debug_info: sections.debug_info,
            debug_line: sections.debug_line,
            debug_line_str: sections.debug_line_str,
            debug_str: sections.debug_str,
            debug_str_offsets: sections.debug_str_offsets,
            debug_types: sections.debug_types,
            locations: LocationLists::new(sections.debug_loc, sections.debug_loclists),
            ranges: RangeLists::new(sections.debug_ranges, sections.debug_rnglists),
            file_type: DwarfFileType::Main,
            sup: None,
            abbreviations_cache: AbbreviationsCache::new(),
        }
    }

    /// Create a `Dwarf` structure that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// ## Example Usage
    ///
    /// It can be useful to load DWARF sections into owned data structures,
    /// such as `Vec`. However, we do not implement the `Reader` trait
    /// for `Vec`, because it would be very inefficient, but this trait
    /// is required for all of the methods that parse the DWARF data.
    /// So we first load the DWARF sections into `Vec`s, and then use
    /// `borrow` to create `Reader`s that reference the data.
    ///
    /// ```rust,no_run
    /// # fn example() -> Result<(), gimli::Error> {
    /// # let loader = |name| -> Result<_, gimli::Error> { unimplemented!() };
    /// # let sup_loader = |name| -> Result<_, gimli::Error> { unimplemented!() };
    /// // Read the DWARF sections into `Vec`s with whatever object loader you're using.
    /// let mut owned_dwarf: gimli::Dwarf<Vec<u8>> = gimli::Dwarf::load(loader)?;
    /// owned_dwarf.load_sup(sup_loader)?;
    /// // Create references to the DWARF sections.
    /// let dwarf = owned_dwarf.borrow(|section| {
    ///     gimli::EndianSlice::new(&section, gimli::LittleEndian)
    /// });
    /// # unreachable!()
    /// # }
    /// ```
    #[deprecated(note = "use `DwarfSections::borrow` instead")]
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F) -> Dwarf<R>
    where
        F: FnMut(&'a T) -> R,
    {
        Dwarf {
            debug_abbrev: self.debug_abbrev.borrow(&mut borrow),
            debug_addr: self.debug_addr.borrow(&mut borrow),
            debug_aranges: self.debug_aranges.borrow(&mut borrow),
            debug_info: self.debug_info.borrow(&mut borrow),
            debug_line: self.debug_line.borrow(&mut borrow),
            debug_line_str: self.debug_line_str.borrow(&mut borrow),
            debug_str: self.debug_str.borrow(&mut borrow),
            debug_str_offsets: self.debug_str_offsets.borrow(&mut borrow),
            debug_types: self.debug_types.borrow(&mut borrow),
            locations: self.locations.borrow(&mut borrow),
            ranges: self.ranges.borrow(&mut borrow),
            file_type: self.file_type,
            sup: self.sup().map(|sup| Arc::new(sup.borrow(borrow))),
            abbreviations_cache: AbbreviationsCache::new(),
        }
    }

    /// Store the DWARF sections for the supplementary object file.
    pub fn set_sup(&mut self, sup: Dwarf<T>) {
        self.sup = Some(Arc::new(sup));
    }

    /// Return a reference to the DWARF sections for the supplementary object file.
    pub fn sup(&self) -> Option<&Dwarf<T>> {
        self.sup.as_ref().map(Arc::as_ref)
    }
}

impl<R: Reader> Dwarf<R> {
    /// Parse abbreviations and store them in the cache.
    ///
    /// This will iterate over the units in `self.debug_info` to determine the
    /// abbreviations offsets.
    ///
    /// Errors during parsing abbreviations are also stored in the cache.
    /// Errors during iterating over the units are ignored.
    pub fn populate_abbreviations_cache(&mut self, strategy: AbbreviationsCacheStrategy) {
        self.abbreviations_cache
            .populate(strategy, &self.debug_abbrev, self.debug_info.units());
    }

    /// Iterate the unit headers in the `.debug_info` section.
    ///
    /// Can be [used with
    /// `FallibleIterator`](./index.html#using-with-fallibleiterator).
    #[inline]
    pub fn units(&self) -> DebugInfoUnitHeadersIter<R> {
        self.debug_info.units()
    }

    /// Construct a new `Unit` from the given unit header.
    #[inline]
    pub fn unit(&self, header: UnitHeader<R>) -> Result<Unit<R>> {
        Unit::new(self, header)
    }

    /// Iterate the type-unit headers in the `.debug_types` section.
    ///
    /// Can be [used with
    /// `FallibleIterator`](./index.html#using-with-fallibleiterator).
    #[inline]
    pub fn type_units(&self) -> DebugTypesUnitHeadersIter<R> {
        self.debug_types.units()
    }

    /// Parse the abbreviations for a compilation unit.
    #[inline]
    pub fn abbreviations(&self, unit: &UnitHeader<R>) -> Result<Arc<Abbreviations>> {
        self.abbreviations_cache
            .get(&self.debug_abbrev, unit.debug_abbrev_offset())
    }

    /// Return the string offset at the given index.
    #[inline]
    pub fn string_offset(
        &self,
        unit: &Unit<R>,
        index: DebugStrOffsetsIndex<R::Offset>,
    ) -> Result<DebugStrOffset<R::Offset>> {
        self.debug_str_offsets
            .get_str_offset(unit.header.format(), unit.str_offsets_base, index)
    }

    /// Return the string at the given offset in `.debug_str`.
    #[inline]
    pub fn string(&self, offset: DebugStrOffset<R::Offset>) -> Result<R> {
        self.debug_str.get_str(offset)
    }

    /// Return the string at the given offset in `.debug_line_str`.
    #[inline]
    pub fn line_string(&self, offset: DebugLineStrOffset<R::Offset>) -> Result<R> {
        self.debug_line_str.get_str(offset)
    }

    /// Return the string at the given offset in the `.debug_str`
    /// in the supplementary object file.
    #[inline]
    pub fn sup_string(&self, offset: DebugStrOffset<R::Offset>) -> Result<R> {
        if let Some(sup) = self.sup() {
            sup.debug_str.get_str(offset)
        } else {
            Err(Error::ExpectedStringAttributeValue)
        }
    }

    /// Return an attribute value as a string slice.
    ///
    /// If the attribute value is one of:
    ///
    /// - an inline `DW_FORM_string` string
    /// - a `DW_FORM_strp` reference to an offset into the `.debug_str` section
    /// - a `DW_FORM_strp_sup` reference to an offset into a supplementary
    ///   object file
    /// - a `DW_FORM_line_strp` reference to an offset into the `.debug_line_str`
    ///   section
    /// - a `DW_FORM_strx` index into the `.debug_str_offsets` entries for the unit
    ///
    /// then return the attribute's string value. Returns an error if the attribute
    /// value does not have a string form, or if a string form has an invalid value.
    pub fn attr_string(&self, unit: &Unit<R>, attr: AttributeValue<R>) -> Result<R> {
        match attr {
            AttributeValue::String(string) => Ok(string),
            AttributeValue::DebugStrRef(offset) => self.string(offset),
            AttributeValue::DebugStrRefSup(offset) => self.sup_string(offset),
            AttributeValue::DebugLineStrRef(offset) => self.line_string(offset),
            AttributeValue::DebugStrOffsetsIndex(index) => {
                let offset = self.string_offset(unit, index)?;
                self.string(offset)
            }
            _ => Err(Error::ExpectedStringAttributeValue),
        }
    }

    /// Return the address at the given index.
    pub fn address(&self, unit: &Unit<R>, index: DebugAddrIndex<R::Offset>) -> Result<u64> {
        self.debug_addr
            .get_address(unit.encoding().address_size, unit.addr_base, index)
    }

    /// Try to return an attribute value as an address.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_addr`
    /// - a `DW_FORM_addrx` index into the `.debug_addr` entries for the unit
    ///
    /// then return the address.
    /// Returns `None` for other forms.
    pub fn attr_address(&self, unit: &Unit<R>, attr: AttributeValue<R>) -> Result<Option<u64>> {
        match attr {
            AttributeValue::Addr(addr) => Ok(Some(addr)),
            AttributeValue::DebugAddrIndex(index) => self.address(unit, index).map(Some),
            _ => Ok(None),
        }
    }

    /// Return the range list offset for the given raw offset.
    ///
    /// This handles adding `DW_AT_GNU_ranges_base` if required.
    pub fn ranges_offset_from_raw(
        &self,
        unit: &Unit<R>,
        offset: RawRangeListsOffset<R::Offset>,
    ) -> RangeListsOffset<R::Offset> {
        if self.file_type == DwarfFileType::Dwo && unit.header.version() < 5 {
            RangeListsOffset(offset.0.wrapping_add(unit.rnglists_base.0))
        } else {
            RangeListsOffset(offset.0)
        }
    }

    /// Return the range list offset at the given index.
    pub fn ranges_offset(
        &self,
        unit: &Unit<R>,
        index: DebugRngListsIndex<R::Offset>,
    ) -> Result<RangeListsOffset<R::Offset>> {
        self.ranges
            .get_offset(unit.encoding(), unit.rnglists_base, index)
    }

    /// Iterate over the `RangeListEntry`s starting at the given offset.
    pub fn ranges(
        &self,
        unit: &Unit<R>,
        offset: RangeListsOffset<R::Offset>,
    ) -> Result<RngListIter<R>> {
        self.ranges.ranges(
            offset,
            unit.encoding(),
            unit.low_pc,
            &self.debug_addr,
            unit.addr_base,
        )
    }

    /// Iterate over the `RawRngListEntry`ies starting at the given offset.
    pub fn raw_ranges(
        &self,
        unit: &Unit<R>,
        offset: RangeListsOffset<R::Offset>,
    ) -> Result<RawRngListIter<R>> {
        self.ranges.raw_ranges(offset, unit.encoding())
    }

    /// Try to return an attribute value as a range list offset.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_sec_offset` reference to the `.debug_ranges` or `.debug_rnglists` sections
    /// - a `DW_FORM_rnglistx` index into the `.debug_rnglists` entries for the unit
    ///
    /// then return the range list offset of the range list.
    /// Returns `None` for other forms.
    pub fn attr_ranges_offset(
        &self,
        unit: &Unit<R>,
        attr: AttributeValue<R>,
    ) -> Result<Option<RangeListsOffset<R::Offset>>> {
        match attr {
            AttributeValue::RangeListsRef(offset) => {
                Ok(Some(self.ranges_offset_from_raw(unit, offset)))
            }
            AttributeValue::DebugRngListsIndex(index) => self.ranges_offset(unit, index).map(Some),
            _ => Ok(None),
        }
    }

    /// Try to return an attribute value as a range list entry iterator.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_sec_offset` reference to the `.debug_ranges` or `.debug_rnglists` sections
    /// - a `DW_FORM_rnglistx` index into the `.debug_rnglists` entries for the unit
    ///
    /// then return an iterator over the entries in the range list.
    /// Returns `None` for other forms.
    pub fn attr_ranges(
        &self,
        unit: &Unit<R>,
        attr: AttributeValue<R>,
    ) -> Result<Option<RngListIter<R>>> {
        match self.attr_ranges_offset(unit, attr)? {
            Some(offset) => Ok(Some(self.ranges(unit, offset)?)),
            None => Ok(None),
        }
    }

    /// Return an iterator for the address ranges of a `DebuggingInformationEntry`.
    ///
    /// This uses `DW_AT_low_pc`, `DW_AT_high_pc` and `DW_AT_ranges`.
    pub fn die_ranges(
        &self,
        unit: &Unit<R>,
        entry: &DebuggingInformationEntry<'_, '_, R>,
    ) -> Result<RangeIter<R>> {
        let mut low_pc = None;
        let mut high_pc = None;
        let mut size = None;
        let mut attrs = entry.attrs();
        while let Some(attr) = attrs.next()? {
            match attr.name() {
                constants::DW_AT_low_pc => {
                    low_pc = Some(
                        self.attr_address(unit, attr.value())?
                            .ok_or(Error::UnsupportedAttributeForm)?,
                    );
                }
                constants::DW_AT_high_pc => match attr.value() {
                    AttributeValue::Udata(val) => size = Some(val),
                    attr => {
                        high_pc = Some(
                            self.attr_address(unit, attr)?
                                .ok_or(Error::UnsupportedAttributeForm)?,
                        );
                    }
                },
                constants::DW_AT_ranges => {
                    if let Some(list) = self.attr_ranges(unit, attr.value())? {
                        return Ok(RangeIter(RangeIterInner::List(list)));
                    }
                }
                _ => {}
            }
        }
        let range = low_pc.and_then(|begin| {
            let end = size.map(|size| begin + size).or(high_pc);
            // TODO: perhaps return an error if `end` is `None`
            end.map(|end| Range { begin, end })
        });
        Ok(RangeIter(RangeIterInner::Single(range)))
    }

    /// Return an iterator for the address ranges of a `Unit`.
    ///
    /// This uses `DW_AT_low_pc`, `DW_AT_high_pc` and `DW_AT_ranges` of the
    /// root `DebuggingInformationEntry`.
    pub fn unit_ranges(&self, unit: &Unit<R>) -> Result<RangeIter<R>> {
        let mut cursor = unit.header.entries(&unit.abbreviations);
        cursor.next_dfs()?;
        let root = cursor.current().ok_or(Error::MissingUnitDie)?;
        self.die_ranges(unit, root)
    }

    /// Return the location list offset at the given index.
    pub fn locations_offset(
        &self,
        unit: &Unit<R>,
        index: DebugLocListsIndex<R::Offset>,
    ) -> Result<LocationListsOffset<R::Offset>> {
        self.locations
            .get_offset(unit.encoding(), unit.loclists_base, index)
    }

    /// Iterate over the `LocationListEntry`s starting at the given offset.
    pub fn locations(
        &self,
        unit: &Unit<R>,
        offset: LocationListsOffset<R::Offset>,
    ) -> Result<LocListIter<R>> {
        match self.file_type {
            DwarfFileType::Main => self.locations.locations(
                offset,
                unit.encoding(),
                unit.low_pc,
                &self.debug_addr,
                unit.addr_base,
            ),
            DwarfFileType::Dwo => self.locations.locations_dwo(
                offset,
                unit.encoding(),
                unit.low_pc,
                &self.debug_addr,
                unit.addr_base,
            ),
        }
    }

    /// Iterate over the raw `LocationListEntry`s starting at the given offset.
    pub fn raw_locations(
        &self,
        unit: &Unit<R>,
        offset: LocationListsOffset<R::Offset>,
    ) -> Result<RawLocListIter<R>> {
        match self.file_type {
            DwarfFileType::Main => self.locations.raw_locations(offset, unit.encoding()),
            DwarfFileType::Dwo => self.locations.raw_locations_dwo(offset, unit.encoding()),
        }
    }

    /// Try to return an attribute value as a location list offset.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_sec_offset` reference to the `.debug_loc` or `.debug_loclists` sections
    /// - a `DW_FORM_loclistx` index into the `.debug_loclists` entries for the unit
    ///
    /// then return the location list offset of the location list.
    /// Returns `None` for other forms.
    pub fn attr_locations_offset(
        &self,
        unit: &Unit<R>,
        attr: AttributeValue<R>,
    ) -> Result<Option<LocationListsOffset<R::Offset>>> {
        match attr {
            AttributeValue::LocationListsRef(offset) => Ok(Some(offset)),
            AttributeValue::DebugLocListsIndex(index) => {
                self.locations_offset(unit, index).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Try to return an attribute value as a location list entry iterator.
    ///
    /// If the attribute value is one of:
    ///
    /// - a `DW_FORM_sec_offset` reference to the `.debug_loc` or `.debug_loclists` sections
    /// - a `DW_FORM_loclistx` index into the `.debug_loclists` entries for the unit
    ///
    /// then return an iterator over the entries in the location list.
    /// Returns `None` for other forms.
    pub fn attr_locations(
        &self,
        unit: &Unit<R>,
        attr: AttributeValue<R>,
    ) -> Result<Option<LocListIter<R>>> {
        match self.attr_locations_offset(unit, attr)? {
            Some(offset) => Ok(Some(self.locations(unit, offset)?)),
            None => Ok(None),
        }
    }

    /// Call `Reader::lookup_offset_id` for each section, and return the first match.
    ///
    /// The first element of the tuple is `true` for supplementary sections.
    pub fn lookup_offset_id(&self, id: ReaderOffsetId) -> Option<(bool, SectionId, R::Offset)> {
        None.or_else(|| self.debug_abbrev.lookup_offset_id(id))
            .or_else(|| self.debug_addr.lookup_offset_id(id))
            .or_else(|| self.debug_aranges.lookup_offset_id(id))
            .or_else(|| self.debug_info.lookup_offset_id(id))
            .or_else(|| self.debug_line.lookup_offset_id(id))
            .or_else(|| self.debug_line_str.lookup_offset_id(id))
            .or_else(|| self.debug_str.lookup_offset_id(id))
            .or_else(|| self.debug_str_offsets.lookup_offset_id(id))
            .or_else(|| self.debug_types.lookup_offset_id(id))
            .or_else(|| self.locations.lookup_offset_id(id))
            .or_else(|| self.ranges.lookup_offset_id(id))
            .map(|(id, offset)| (false, id, offset))
            .or_else(|| {
                self.sup()
                    .and_then(|sup| sup.lookup_offset_id(id))
                    .map(|(_, id, offset)| (true, id, offset))
            })
    }

    /// Returns a string representation of the given error.
    ///
    /// This uses information from the DWARF sections to provide more information in some cases.
    pub fn format_error(&self, err: Error) -> String {
        #[allow(clippy::single_match)]
        match err {
            Error::UnexpectedEof(id) => match self.lookup_offset_id(id) {
                Some((sup, section, offset)) => {
                    return format!(
                        "{} at {}{}+0x{:x}",
                        err,
                        section.name(),
                        if sup { "(sup)" } else { "" },
                        offset.into_u64(),
                    );
                }
                None => {}
            },
            _ => {}
        }
        err.description().into()
    }
}

impl<R: Clone> Dwarf<R> {
    /// Assuming `self` was loaded from a .dwo, take the appropriate
    /// sections from `parent` (which contains the skeleton unit for this
    /// dwo) such as `.debug_addr` and merge them into this `Dwarf`.
    pub fn make_dwo(&mut self, parent: &Dwarf<R>) {
        self.file_type = DwarfFileType::Dwo;
        // These sections are always taken from the parent file and not the dwo.
        self.debug_addr = parent.debug_addr.clone();
        // .debug_rnglists comes from the DWO, .debug_ranges comes from the
        // parent file.
        self.ranges
            .set_debug_ranges(parent.ranges.debug_ranges().clone());
        self.sup.clone_from(&parent.sup);
    }
}

/// The sections from a `.dwp` file.
///
/// This is useful for storing sections when `T` does not implement `Reader`.
/// It can be used to create a `DwarfPackage` that references the data in `self`.
/// If `T` does implement `Reader`, then use `DwarfPackage` directly.
///
/// ## Example Usage
///
/// It can be useful to load DWARF sections into owned data structures,
/// such as `Vec`. However, we do not implement the `Reader` trait
/// for `Vec`, because it would be very inefficient, but this trait
/// is required for all of the methods that parse the DWARF data.
/// So we first load the DWARF sections into `Vec`s, and then use
/// `borrow` to create `Reader`s that reference the data.
///
/// ```rust,no_run
/// # fn example() -> Result<(), gimli::Error> {
/// # let loader = |name| -> Result<_, gimli::Error> { unimplemented!() };
/// // Read the DWARF sections into `Vec`s with whatever object loader you're using.
/// let dwp_sections: gimli::DwarfPackageSections<Vec<u8>> = gimli::DwarfPackageSections::load(loader)?;
/// // Create references to the DWARF sections.
/// let dwp: gimli::DwarfPackage<_> = dwp_sections.borrow(
///     |section| gimli::EndianSlice::new(&section, gimli::LittleEndian),
///     gimli::EndianSlice::new(&[], gimli::LittleEndian),
/// )?;
/// # unreachable!()
/// # }
/// ```
#[derive(Debug, Default)]
pub struct DwarfPackageSections<T> {
    /// The `.debug_cu_index` section.
    pub cu_index: DebugCuIndex<T>,
    /// The `.debug_tu_index` section.
    pub tu_index: DebugTuIndex<T>,
    /// The `.debug_abbrev.dwo` section.
    pub debug_abbrev: DebugAbbrev<T>,
    /// The `.debug_info.dwo` section.
    pub debug_info: DebugInfo<T>,
    /// The `.debug_line.dwo` section.
    pub debug_line: DebugLine<T>,
    /// The `.debug_str.dwo` section.
    pub debug_str: DebugStr<T>,
    /// The `.debug_str_offsets.dwo` section.
    pub debug_str_offsets: DebugStrOffsets<T>,
    /// The `.debug_loc.dwo` section.
    ///
    /// Only present when using GNU split-dwarf extension to DWARF 4.
    pub debug_loc: DebugLoc<T>,
    /// The `.debug_loclists.dwo` section.
    pub debug_loclists: DebugLocLists<T>,
    /// The `.debug_rnglists.dwo` section.
    pub debug_rnglists: DebugRngLists<T>,
    /// The `.debug_types.dwo` section.
    ///
    /// Only present when using GNU split-dwarf extension to DWARF 4.
    pub debug_types: DebugTypes<T>,
}

impl<T> DwarfPackageSections<T> {
    /// Try to load the `.dwp` sections using the given loader function.
    ///
    /// `section` loads a DWARF section from the object file.
    /// It should return an empty section if the section does not exist.
    pub fn load<F, E>(mut section: F) -> core::result::Result<Self, E>
    where
        F: FnMut(SectionId) -> core::result::Result<T, E>,
        E: From<Error>,
    {
        Ok(DwarfPackageSections {
            // Section types are inferred.
            cu_index: Section::load(&mut section)?,
            tu_index: Section::load(&mut section)?,
            debug_abbrev: Section::load(&mut section)?,
            debug_info: Section::load(&mut section)?,
            debug_line: Section::load(&mut section)?,
            debug_str: Section::load(&mut section)?,
            debug_str_offsets: Section::load(&mut section)?,
            debug_loc: Section::load(&mut section)?,
            debug_loclists: Section::load(&mut section)?,
            debug_rnglists: Section::load(&mut section)?,
            debug_types: Section::load(&mut section)?,
        })
    }

    /// Create a `DwarfPackage` structure that references the data in `self`.
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F, empty: R) -> Result<DwarfPackage<R>>
    where
        F: FnMut(&'a T) -> R,
        R: Reader,
    {
        DwarfPackage::from_sections(
            DwarfPackageSections {
                cu_index: self.cu_index.borrow(&mut borrow),
                tu_index: self.tu_index.borrow(&mut borrow),
                debug_abbrev: self.debug_abbrev.borrow(&mut borrow),
                debug_info: self.debug_info.borrow(&mut borrow),
                debug_line: self.debug_line.borrow(&mut borrow),
                debug_str: self.debug_str.borrow(&mut borrow),
                debug_str_offsets: self.debug_str_offsets.borrow(&mut borrow),
                debug_loc: self.debug_loc.borrow(&mut borrow),
                debug_loclists: self.debug_loclists.borrow(&mut borrow),
                debug_rnglists: self.debug_rnglists.borrow(&mut borrow),
                debug_types: self.debug_types.borrow(&mut borrow),
            },
            empty,
        )
    }
}

/// The sections from a `.dwp` file, with parsed indices.
#[derive(Debug)]
pub struct DwarfPackage<R: Reader> {
    /// The compilation unit index in the `.debug_cu_index` section.
    pub cu_index: UnitIndex<R>,

    /// The type unit index in the `.debug_tu_index` section.
    pub tu_index: UnitIndex<R>,

    /// The `.debug_abbrev.dwo` section.
    pub debug_abbrev: DebugAbbrev<R>,

    /// The `.debug_info.dwo` section.
    pub debug_info: DebugInfo<R>,

    /// The `.debug_line.dwo` section.
    pub debug_line: DebugLine<R>,

    /// The `.debug_str.dwo` section.
    pub debug_str: DebugStr<R>,

    /// The `.debug_str_offsets.dwo` section.
    pub debug_str_offsets: DebugStrOffsets<R>,

    /// The `.debug_loc.dwo` section.
    ///
    /// Only present when using GNU split-dwarf extension to DWARF 4.
    pub debug_loc: DebugLoc<R>,

    /// The `.debug_loclists.dwo` section.
    pub debug_loclists: DebugLocLists<R>,

    /// The `.debug_rnglists.dwo` section.
    pub debug_rnglists: DebugRngLists<R>,

    /// The `.debug_types.dwo` section.
    ///
    /// Only present when using GNU split-dwarf extension to DWARF 4.
    pub debug_types: DebugTypes<R>,

    /// An empty section.
    ///
    /// Used when creating `Dwarf<R>`.
    pub empty: R,
}

impl<R: Reader> DwarfPackage<R> {
    /// Try to load the `.dwp` sections using the given loader function.
    ///
    /// `section` loads a DWARF section from the object file.
    /// It should return an empty section if the section does not exist.
    pub fn load<F, E>(section: F, empty: R) -> core::result::Result<Self, E>
    where
        F: FnMut(SectionId) -> core::result::Result<R, E>,
        E: From<Error>,
    {
        let sections = DwarfPackageSections::load(section)?;
        Ok(Self::from_sections(sections, empty)?)
    }

    /// Create a `DwarfPackage` structure from the given sections.
    fn from_sections(sections: DwarfPackageSections<R>, empty: R) -> Result<Self> {
        Ok(DwarfPackage {
            cu_index: sections.cu_index.index()?,
            tu_index: sections.tu_index.index()?,
            debug_abbrev: sections.debug_abbrev,
            debug_info: sections.debug_info,
            debug_line: sections.debug_line,
            debug_str: sections.debug_str,
            debug_str_offsets: sections.debug_str_offsets,
            debug_loc: sections.debug_loc,
            debug_loclists: sections.debug_loclists,
            debug_rnglists: sections.debug_rnglists,
            debug_types: sections.debug_types,
            empty,
        })
    }

    /// Find the compilation unit with the given DWO identifier and return its section
    /// contributions.
    ///
    /// ## Example Usage
    ///
    /// ```rust,no_run
    /// # fn example<R: gimli::Reader>(
    /// #        dwarf: &gimli::Dwarf<R>,
    /// #        dwp: &gimli::DwarfPackage<R>,
    /// #        dwo_id: gimli::DwoId,
    /// # ) -> Result<(), gimli::Error> {
    /// if let Some(dwo) = dwp.find_cu(dwo_id, dwarf)? {
    ///    let dwo_header = dwo.units().next()?.expect("DWO should have one unit");
    ///    let dwo_unit = dwo.unit(dwo_header)?;
    ///    // Do something with `dwo_unit`.
    /// }
    /// # unreachable!()
    /// # }
    pub fn find_cu(&self, id: DwoId, parent: &Dwarf<R>) -> Result<Option<Dwarf<R>>> {
        let row = match self.cu_index.find(id.0) {
            Some(row) => row,
            None => return Ok(None),
        };
        self.cu_sections(row, parent).map(Some)
    }

    /// Find the type unit with the given type signature and return its section
    /// contributions.
    pub fn find_tu(
        &self,
        signature: DebugTypeSignature,
        parent: &Dwarf<R>,
    ) -> Result<Option<Dwarf<R>>> {
        let row = match self.tu_index.find(signature.0) {
            Some(row) => row,
            None => return Ok(None),
        };
        self.tu_sections(row, parent).map(Some)
    }

    /// Return the section contributions of the compilation unit at the given index.
    ///
    /// The index must be in the range `1..cu_index.unit_count`.
    ///
    /// This function should only be needed by low level parsers.
    pub fn cu_sections(&self, index: u32, parent: &Dwarf<R>) -> Result<Dwarf<R>> {
        self.sections(self.cu_index.sections(index)?, parent)
    }

    /// Return the section contributions of the compilation unit at the given index.
    ///
    /// The index must be in the range `1..tu_index.unit_count`.
    ///
    /// This function should only be needed by low level parsers.
    pub fn tu_sections(&self, index: u32, parent: &Dwarf<R>) -> Result<Dwarf<R>> {
        self.sections(self.tu_index.sections(index)?, parent)
    }

    /// Return the section contributions of a unit.
    ///
    /// This function should only be needed by low level parsers.
    pub fn sections(
        &self,
        sections: UnitIndexSectionIterator<'_, R>,
        parent: &Dwarf<R>,
    ) -> Result<Dwarf<R>> {
        let mut abbrev_offset = 0;
        let mut abbrev_size = 0;
        let mut info_offset = 0;
        let mut info_size = 0;
        let mut line_offset = 0;
        let mut line_size = 0;
        let mut loc_offset = 0;
        let mut loc_size = 0;
        let mut loclists_offset = 0;
        let mut loclists_size = 0;
        let mut str_offsets_offset = 0;
        let mut str_offsets_size = 0;
        let mut rnglists_offset = 0;
        let mut rnglists_size = 0;
        let mut types_offset = 0;
        let mut types_size = 0;
        for section in sections {
            match section.section {
                IndexSectionId::DebugAbbrev => {
                    abbrev_offset = section.offset;
                    abbrev_size = section.size;
                }
                IndexSectionId::DebugInfo => {
                    info_offset = section.offset;
                    info_size = section.size;
                }
                IndexSectionId::DebugLine => {
                    line_offset = section.offset;
                    line_size = section.size;
                }
                IndexSectionId::DebugLoc => {
                    loc_offset = section.offset;
                    loc_size = section.size;
                }
                IndexSectionId::DebugLocLists => {
                    loclists_offset = section.offset;
                    loclists_size = section.size;
                }
                IndexSectionId::DebugStrOffsets => {
                    str_offsets_offset = section.offset;
                    str_offsets_size = section.size;
                }
                IndexSectionId::DebugRngLists => {
                    rnglists_offset = section.offset;
                    rnglists_size = section.size;
                }
                IndexSectionId::DebugTypes => {
                    types_offset = section.offset;
                    types_size = section.size;
                }
                IndexSectionId::DebugMacro | IndexSectionId::DebugMacinfo => {
                    // These are valid but we can't parse these yet.
                }
            }
        }

        let debug_abbrev = self.debug_abbrev.dwp_range(abbrev_offset, abbrev_size)?;
        let debug_info = self.debug_info.dwp_range(info_offset, info_size)?;
        let debug_line = self.debug_line.dwp_range(line_offset, line_size)?;
        let debug_loc = self.debug_loc.dwp_range(loc_offset, loc_size)?;
        let debug_loclists = self
            .debug_loclists
            .dwp_range(loclists_offset, loclists_size)?;
        let debug_str_offsets = self
            .debug_str_offsets
            .dwp_range(str_offsets_offset, str_offsets_size)?;
        let debug_rnglists = self
            .debug_rnglists
            .dwp_range(rnglists_offset, rnglists_size)?;
        let debug_types = self.debug_types.dwp_range(types_offset, types_size)?;

        let debug_str = self.debug_str.clone();

        let debug_addr = parent.debug_addr.clone();
        let debug_ranges = parent.ranges.debug_ranges().clone();

        let debug_aranges = self.empty.clone().into();
        let debug_line_str = self.empty.clone().into();

        Ok(Dwarf {
            debug_abbrev,
            debug_addr,
            debug_aranges,
            debug_info,
            debug_line,
            debug_line_str,
            debug_str,
            debug_str_offsets,
            debug_types,
            locations: LocationLists::new(debug_loc, debug_loclists),
            ranges: RangeLists::new(debug_ranges, debug_rnglists),
            file_type: DwarfFileType::Dwo,
            sup: parent.sup.clone(),
            abbreviations_cache: AbbreviationsCache::new(),
        })
    }
}

/// All of the commonly used information for a unit in the `.debug_info` or `.debug_types`
/// sections.
#[derive(Debug)]
pub struct Unit<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    /// The header of the unit.
    pub header: UnitHeader<R, Offset>,

    /// The parsed abbreviations for the unit.
    pub abbreviations: Arc<Abbreviations>,

    /// The `DW_AT_name` attribute of the unit.
    pub name: Option<R>,

    /// The `DW_AT_comp_dir` attribute of the unit.
    pub comp_dir: Option<R>,

    /// The `DW_AT_low_pc` attribute of the unit. Defaults to 0.
    pub low_pc: u64,

    /// The `DW_AT_str_offsets_base` attribute of the unit. Defaults to 0.
    pub str_offsets_base: DebugStrOffsetsBase<Offset>,

    /// The `DW_AT_addr_base` attribute of the unit. Defaults to 0.
    pub addr_base: DebugAddrBase<Offset>,

    /// The `DW_AT_loclists_base` attribute of the unit. Defaults to 0.
    pub loclists_base: DebugLocListsBase<Offset>,

    /// The `DW_AT_rnglists_base` attribute of the unit. Defaults to 0.
    pub rnglists_base: DebugRngListsBase<Offset>,

    /// The line number program of the unit.
    pub line_program: Option<IncompleteLineProgram<R, Offset>>,

    /// The DWO ID of a skeleton unit or split compilation unit.
    pub dwo_id: Option<DwoId>,
}

impl<R: Reader> Unit<R> {
    /// Construct a new `Unit` from the given unit header.
    #[inline]
    pub fn new(dwarf: &Dwarf<R>, header: UnitHeader<R>) -> Result<Self> {
        let abbreviations = dwarf.abbreviations(&header)?;
        Self::new_with_abbreviations(dwarf, header, abbreviations)
    }

    /// Construct a new `Unit` from the given unit header and abbreviations.
    ///
    /// The abbreviations for this call can be obtained using `dwarf.abbreviations(&header)`.
    /// The caller may implement caching to reuse the `Abbreviations` across units with the
    /// same `header.debug_abbrev_offset()` value.
    #[inline]
    pub fn new_with_abbreviations(
        dwarf: &Dwarf<R>,
        header: UnitHeader<R>,
        abbreviations: Arc<Abbreviations>,
    ) -> Result<Self> {
        let mut unit = Unit {
            abbreviations,
            name: None,
            comp_dir: None,
            low_pc: 0,
            str_offsets_base: DebugStrOffsetsBase::default_for_encoding_and_file(
                header.encoding(),
                dwarf.file_type,
            ),
            // NB: Because the .debug_addr section never lives in a .dwo, we can assume its base is always 0 or provided.
            addr_base: DebugAddrBase(R::Offset::from_u8(0)),
            loclists_base: DebugLocListsBase::default_for_encoding_and_file(
                header.encoding(),
                dwarf.file_type,
            ),
            rnglists_base: DebugRngListsBase::default_for_encoding_and_file(
                header.encoding(),
                dwarf.file_type,
            ),
            line_program: None,
            dwo_id: match header.type_() {
                UnitType::Skeleton(dwo_id) | UnitType::SplitCompilation(dwo_id) => Some(dwo_id),
                _ => None,
            },
            header,
        };
        let mut name = None;
        let mut comp_dir = None;
        let mut line_program_offset = None;
        let mut low_pc_attr = None;

        {
            let mut cursor = unit.header.entries(&unit.abbreviations);
            cursor.next_dfs()?;
            let root = cursor.current().ok_or(Error::MissingUnitDie)?;
            let mut attrs = root.attrs();
            while let Some(attr) = attrs.next()? {
                match attr.name() {
                    constants::DW_AT_name => {
                        name = Some(attr.value());
                    }
                    constants::DW_AT_comp_dir => {
                        comp_dir = Some(attr.value());
                    }
                    constants::DW_AT_low_pc => {
                        low_pc_attr = Some(attr.value());
                    }
                    constants::DW_AT_stmt_list => {
                        if let AttributeValue::DebugLineRef(offset) = attr.value() {
                            line_program_offset = Some(offset);
                        }
                    }
                    constants::DW_AT_str_offsets_base => {
                        if let AttributeValue::DebugStrOffsetsBase(base) = attr.value() {
                            unit.str_offsets_base = base;
                        }
                    }
                    constants::DW_AT_addr_base | constants::DW_AT_GNU_addr_base => {
                        if let AttributeValue::DebugAddrBase(base) = attr.value() {
                            unit.addr_base = base;
                        }
                    }
                    constants::DW_AT_loclists_base => {
                        if let AttributeValue::DebugLocListsBase(base) = attr.value() {
                            unit.loclists_base = base;
                        }
                    }
                    constants::DW_AT_rnglists_base | constants::DW_AT_GNU_ranges_base => {
                        if let AttributeValue::DebugRngListsBase(base) = attr.value() {
                            unit.rnglists_base = base;
                        }
                    }
                    constants::DW_AT_GNU_dwo_id => {
                        if unit.dwo_id.is_none() {
                            if let AttributeValue::DwoId(dwo_id) = attr.value() {
                                unit.dwo_id = Some(dwo_id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        unit.name = match name {
            Some(val) => dwarf.attr_string(&unit, val).ok(),
            None => None,
        };
        unit.comp_dir = match comp_dir {
            Some(val) => dwarf.attr_string(&unit, val).ok(),
            None => None,
        };
        unit.line_program = match line_program_offset {
            Some(offset) => Some(dwarf.debug_line.program(
                offset,
                unit.header.address_size(),
                unit.comp_dir.clone(),
                unit.name.clone(),
            )?),
            None => None,
        };
        if let Some(low_pc_attr) = low_pc_attr {
            if let Some(addr) = dwarf.attr_address(&unit, low_pc_attr)? {
                unit.low_pc = addr;
            }
        }
        Ok(unit)
    }

    /// Return a reference to this unit and its associated `Dwarf`.
    pub fn unit_ref<'a>(&'a self, dwarf: &'a Dwarf<R>) -> UnitRef<'a, R> {
        UnitRef::new(dwarf, self)
    }

    /// Return the encoding parameters for this unit.
    #[inline]
    pub fn encoding(&self) -> Encoding {
        self.header.encoding()
    }

    /// Read the `DebuggingInformationEntry` at the given offset.
    pub fn entry(
        &self,
        offset: UnitOffset<R::Offset>,
    ) -> Result<DebuggingInformationEntry<'_, '_, R>> {
        self.header.entry(&self.abbreviations, offset)
    }

    /// Navigate this unit's `DebuggingInformationEntry`s.
    #[inline]
    pub fn entries(&self) -> EntriesCursor<'_, '_, R> {
        self.header.entries(&self.abbreviations)
    }

    /// Navigate this unit's `DebuggingInformationEntry`s
    /// starting at the given offset.
    #[inline]
    pub fn entries_at_offset(
        &self,
        offset: UnitOffset<R::Offset>,
    ) -> Result<EntriesCursor<'_, '_, R>> {
        self.header.entries_at_offset(&self.abbreviations, offset)
    }

    /// Navigate this unit's `DebuggingInformationEntry`s as a tree
    /// starting at the given offset.
    #[inline]
    pub fn entries_tree(
        &self,
        offset: Option<UnitOffset<R::Offset>>,
    ) -> Result<EntriesTree<'_, '_, R>> {
        self.header.entries_tree(&self.abbreviations, offset)
    }

    /// Read the raw data that defines the Debugging Information Entries.
    #[inline]
    pub fn entries_raw(
        &self,
        offset: Option<UnitOffset<R::Offset>>,
    ) -> Result<EntriesRaw<'_, '_, R>> {
        self.header.entries_raw(&self.abbreviations, offset)
    }

    /// Copy attributes that are subject to relocation from another unit. This is intended
    /// to be used to copy attributes from a skeleton compilation unit to the corresponding
    /// split compilation unit.
    pub fn copy_relocated_attributes(&mut self, other: &Unit<R>) {
        self.low_pc = other.low_pc;
        self.addr_base = other.addr_base;
        if self.header.version() < 5 {
            self.rnglists_base = other.rnglists_base;
        }
    }

    /// Find the dwo name (if any) for this unit, automatically handling the differences
    /// between the standardized DWARF 5 split DWARF format and the pre-DWARF 5 GNU
    /// extension.
    ///
    /// The returned value is relative to this unit's `comp_dir`.
    pub fn dwo_name(&self) -> Result<Option<AttributeValue<R>>> {
        let mut entries = self.entries();
        entries.next_entry()?;
        let entry = entries.current().ok_or(Error::MissingUnitDie)?;
        if self.header.version() < 5 {
            entry.attr_value(constants::DW_AT_GNU_dwo_name)
        } else {
            entry.attr_value(constants::DW_AT_dwo_name)
        }
    }
}

/// A reference to a `Unit` and its associated `Dwarf`.
///
/// These often need to be passed around together, so this struct makes that easier.
///
/// It implements `Deref` to `Unit`, so you can use it as if it were a `Unit`.
/// It also implements methods that correspond to methods on `Dwarf` that take a `Unit`.
#[derive(Debug)]
pub struct UnitRef<'a, R: Reader> {
    /// The `Dwarf` that contains the unit.
    pub dwarf: &'a Dwarf<R>,

    /// The `Unit` being referenced.
    pub unit: &'a Unit<R>,
}

impl<'a, R: Reader> Clone for UnitRef<'a, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, R: Reader> Copy for UnitRef<'a, R> {}

impl<'a, R: Reader> core::ops::Deref for UnitRef<'a, R> {
    type Target = Unit<R>;

    fn deref(&self) -> &Self::Target {
        self.unit
    }
}

impl<'a, R: Reader> UnitRef<'a, R> {
    /// Construct a new `UnitRef` from a `Dwarf` and a `Unit`.
    pub fn new(dwarf: &'a Dwarf<R>, unit: &'a Unit<R>) -> Self {
        UnitRef { dwarf, unit }
    }

    /// Return the string offset at the given index.
    #[inline]
    pub fn string_offset(
        &self,
        index: DebugStrOffsetsIndex<R::Offset>,
    ) -> Result<DebugStrOffset<R::Offset>> {
        self.dwarf.string_offset(self.unit, index)
    }

    /// Return the string at the given offset in `.debug_str`.
    #[inline]
    pub fn string(&self, offset: DebugStrOffset<R::Offset>) -> Result<R> {
        self.dwarf.string(offset)
    }

    /// Return the string at the given offset in `.debug_line_str`.
    #[inline]
    pub fn line_string(&self, offset: DebugLineStrOffset<R::Offset>) -> Result<R> {
        self.dwarf.line_string(offset)
    }

    /// Return the string at the given offset in the `.debug_str`
    /// in the supplementary object file.
    #[inline]
    pub fn sup_string(&self, offset: DebugStrOffset<R::Offset>) -> Result<R> {
        self.dwarf.sup_string(offset)
    }

    /// Return an attribute value as a string slice.
    ///
    /// See [`Dwarf::attr_string`] for more information.
    pub fn attr_string(&self, attr: AttributeValue<R>) -> Result<R> {
        self.dwarf.attr_string(self.unit, attr)
    }

    /// Return the address at the given index.
    pub fn address(&self, index: DebugAddrIndex<R::Offset>) -> Result<u64> {
        self.dwarf.address(self.unit, index)
    }

    /// Try to return an attribute value as an address.
    ///
    /// See [`Dwarf::attr_address`] for more information.
    pub fn attr_address(&self, attr: AttributeValue<R>) -> Result<Option<u64>> {
        self.dwarf.attr_address(self.unit, attr)
    }

    /// Return the range list offset for the given raw offset.
    ///
    /// This handles adding `DW_AT_GNU_ranges_base` if required.
    pub fn ranges_offset_from_raw(
        &self,
        offset: RawRangeListsOffset<R::Offset>,
    ) -> RangeListsOffset<R::Offset> {
        self.dwarf.ranges_offset_from_raw(self.unit, offset)
    }

    /// Return the range list offset at the given index.
    pub fn ranges_offset(
        &self,
        index: DebugRngListsIndex<R::Offset>,
    ) -> Result<RangeListsOffset<R::Offset>> {
        self.dwarf.ranges_offset(self.unit, index)
    }

    /// Iterate over the `RangeListEntry`s starting at the given offset.
    pub fn ranges(&self, offset: RangeListsOffset<R::Offset>) -> Result<RngListIter<R>> {
        self.dwarf.ranges(self.unit, offset)
    }

    /// Iterate over the `RawRngListEntry`ies starting at the given offset.
    pub fn raw_ranges(&self, offset: RangeListsOffset<R::Offset>) -> Result<RawRngListIter<R>> {
        self.dwarf.raw_ranges(self.unit, offset)
    }

    /// Try to return an attribute value as a range list offset.
    ///
    /// See [`Dwarf::attr_ranges_offset`] for more information.
    pub fn attr_ranges_offset(
        &self,
        attr: AttributeValue<R>,
    ) -> Result<Option<RangeListsOffset<R::Offset>>> {
        self.dwarf.attr_ranges_offset(self.unit, attr)
    }

    /// Try to return an attribute value as a range list entry iterator.
    ///
    /// See [`Dwarf::attr_ranges`] for more information.
    pub fn attr_ranges(&self, attr: AttributeValue<R>) -> Result<Option<RngListIter<R>>> {
        self.dwarf.attr_ranges(self.unit, attr)
    }

    /// Return an iterator for the address ranges of a `DebuggingInformationEntry`.
    ///
    /// This uses `DW_AT_low_pc`, `DW_AT_high_pc` and `DW_AT_ranges`.
    pub fn die_ranges(&self, entry: &DebuggingInformationEntry<'_, '_, R>) -> Result<RangeIter<R>> {
        self.dwarf.die_ranges(self.unit, entry)
    }

    /// Return an iterator for the address ranges of the `Unit`.
    ///
    /// This uses `DW_AT_low_pc`, `DW_AT_high_pc` and `DW_AT_ranges` of the
    /// root `DebuggingInformationEntry`.
    pub fn unit_ranges(&self) -> Result<RangeIter<R>> {
        self.dwarf.unit_ranges(self.unit)
    }

    /// Return the location list offset at the given index.
    pub fn locations_offset(
        &self,
        index: DebugLocListsIndex<R::Offset>,
    ) -> Result<LocationListsOffset<R::Offset>> {
        self.dwarf.locations_offset(self.unit, index)
    }

    /// Iterate over the `LocationListEntry`s starting at the given offset.
    pub fn locations(&self, offset: LocationListsOffset<R::Offset>) -> Result<LocListIter<R>> {
        self.dwarf.locations(self.unit, offset)
    }

    /// Iterate over the raw `LocationListEntry`s starting at the given offset.
    pub fn raw_locations(
        &self,
        offset: LocationListsOffset<R::Offset>,
    ) -> Result<RawLocListIter<R>> {
        self.dwarf.raw_locations(self.unit, offset)
    }

    /// Try to return an attribute value as a location list offset.
    ///
    /// See [`Dwarf::attr_locations_offset`] for more information.
    pub fn attr_locations_offset(
        &self,
        attr: AttributeValue<R>,
    ) -> Result<Option<LocationListsOffset<R::Offset>>> {
        self.dwarf.attr_locations_offset(self.unit, attr)
    }

    /// Try to return an attribute value as a location list entry iterator.
    ///
    /// See [`Dwarf::attr_locations`] for more information.
    pub fn attr_locations(&self, attr: AttributeValue<R>) -> Result<Option<LocListIter<R>>> {
        self.dwarf.attr_locations(self.unit, attr)
    }
}

impl<T: ReaderOffset> UnitSectionOffset<T> {
    /// Convert an offset to be relative to the start of the given unit,
    /// instead of relative to the start of the section.
    ///
    /// Returns `None` if the offset is not within the unit entries.
    pub fn to_unit_offset<R>(&self, unit: &Unit<R>) -> Option<UnitOffset<T>>
    where
        R: Reader<Offset = T>,
    {
        let (offset, unit_offset) = match (self, unit.header.offset()) {
            (
                UnitSectionOffset::DebugInfoOffset(offset),
                UnitSectionOffset::DebugInfoOffset(unit_offset),
            ) => (offset.0, unit_offset.0),
            (
                UnitSectionOffset::DebugTypesOffset(offset),
                UnitSectionOffset::DebugTypesOffset(unit_offset),
            ) => (offset.0, unit_offset.0),
            _ => return None,
        };
        let offset = match offset.checked_sub(unit_offset) {
            Some(offset) => UnitOffset(offset),
            None => return None,
        };
        if !unit.header.is_valid_offset(offset) {
            return None;
        }
        Some(offset)
    }
}

impl<T: ReaderOffset> UnitOffset<T> {
    /// Convert an offset to be relative to the start of the .debug_info section,
    /// instead of relative to the start of the given compilation unit.
    ///
    /// Does not check that the offset is valid.
    pub fn to_unit_section_offset<R>(&self, unit: &Unit<R>) -> UnitSectionOffset<T>
    where
        R: Reader<Offset = T>,
    {
        match unit.header.offset() {
            UnitSectionOffset::DebugInfoOffset(unit_offset) => {
                DebugInfoOffset(unit_offset.0 + self.0).into()
            }
            UnitSectionOffset::DebugTypesOffset(unit_offset) => {
                DebugTypesOffset(unit_offset.0 + self.0).into()
            }
        }
    }
}

/// An iterator for the address ranges of a `DebuggingInformationEntry`.
///
/// Returned by `Dwarf::die_ranges` and `Dwarf::unit_ranges`.
#[derive(Debug)]
pub struct RangeIter<R: Reader>(RangeIterInner<R>);

#[derive(Debug)]
enum RangeIterInner<R: Reader> {
    Single(Option<Range>),
    List(RngListIter<R>),
}

impl<R: Reader> Default for RangeIter<R> {
    fn default() -> Self {
        RangeIter(RangeIterInner::Single(None))
    }
}

impl<R: Reader> RangeIter<R> {
    /// Advance the iterator to the next range.
    pub fn next(&mut self) -> Result<Option<Range>> {
        match self.0 {
            RangeIterInner::Single(ref mut range) => Ok(range.take()),
            RangeIterInner::List(ref mut list) => list.next(),
        }
    }
}

#[cfg(feature = "fallible-iterator")]
impl<R: Reader> fallible_iterator::FallibleIterator for RangeIter<R> {
    type Item = Range;
    type Error = Error;

    #[inline]
    fn next(&mut self) -> ::core::result::Result<Option<Self::Item>, Self::Error> {
        RangeIter::next(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::EndianSlice;
    use crate::{Endianity, LittleEndian};

    /// Ensure that `Dwarf<R>` is covariant wrt R.
    #[test]
    fn test_dwarf_variance() {
        /// This only needs to compile.
        fn _f<'a: 'b, 'b, E: Endianity>(x: Dwarf<EndianSlice<'a, E>>) -> Dwarf<EndianSlice<'b, E>> {
            x
        }
    }

    /// Ensure that `Unit<R>` is covariant wrt R.
    #[test]
    fn test_dwarf_unit_variance() {
        /// This only needs to compile.
        fn _f<'a: 'b, 'b, E: Endianity>(x: Unit<EndianSlice<'a, E>>) -> Unit<EndianSlice<'b, E>> {
            x
        }
    }

    #[test]
    fn test_send() {
        fn assert_is_send<T: Send>() {}
        assert_is_send::<Dwarf<EndianSlice<'_, LittleEndian>>>();
        assert_is_send::<Unit<EndianSlice<'_, LittleEndian>>>();
    }

    #[test]
    fn test_format_error() {
        let dwarf_sections = DwarfSections::load(|_| -> Result<_> { Ok(vec![1, 2]) }).unwrap();
        let sup_sections = DwarfSections::load(|_| -> Result<_> { Ok(vec![1, 2]) }).unwrap();
        let dwarf = dwarf_sections.borrow_with_sup(&sup_sections, |section| {
            EndianSlice::new(section, LittleEndian)
        });

        match dwarf.debug_str.get_str(DebugStrOffset(1)) {
            Ok(r) => panic!("Unexpected str {:?}", r),
            Err(e) => {
                assert_eq!(
                    dwarf.format_error(e),
                    "Hit the end of input before it was expected at .debug_str+0x1"
                );
            }
        }
        match dwarf.sup().unwrap().debug_str.get_str(DebugStrOffset(1)) {
            Ok(r) => panic!("Unexpected str {:?}", r),
            Err(e) => {
                assert_eq!(
                    dwarf.format_error(e),
                    "Hit the end of input before it was expected at .debug_str(sup)+0x1"
                );
            }
        }
        assert_eq!(dwarf.format_error(Error::Io), Error::Io.description());
    }
}
