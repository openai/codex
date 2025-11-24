use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp;

use crate::lazy::LazyResult;
use crate::{
    Context, DebugFile, Error, Function, Functions, LazyFunctions, LazyLines,
    LineLocationRangeIter, Lines, Location, LookupContinuation, LookupResult, RangeAttributes,
    SimpleLookup, SplitDwarfLoad,
};

pub(crate) struct UnitRange {
    unit_id: usize,
    min_begin: u64,
    range: gimli::Range,
}

pub(crate) struct ResUnit<R: gimli::Reader> {
    offset: gimli::DebugInfoOffset<R::Offset>,
    dw_unit: gimli::Unit<R>,
    pub(crate) lang: Option<gimli::DwLang>,
    lines: LazyLines,
    functions: LazyFunctions<R>,
    dwo: LazyResult<Option<Box<DwoUnit<R>>>>,
}

type UnitRef<'unit, R> = (DebugFile, gimli::UnitRef<'unit, R>);

impl<R: gimli::Reader> ResUnit<R> {
    pub(crate) fn unit_ref<'a>(&'a self, sections: &'a gimli::Dwarf<R>) -> gimli::UnitRef<'a, R> {
        gimli::UnitRef::new(sections, &self.dw_unit)
    }

    /// Returns the DWARF sections and the unit.
    ///
    /// Loads the DWO unit if necessary.
    pub(crate) fn dwarf_and_unit<'unit, 'ctx: 'unit>(
        &'unit self,
        ctx: &'ctx Context<R>,
    ) -> LookupResult<
        SimpleLookup<
            Result<UnitRef<'unit, R>, Error>,
            R,
            impl FnOnce(Option<Arc<gimli::Dwarf<R>>>) -> Result<UnitRef<'unit, R>, Error>,
        >,
    > {
        let map_dwo = move |dwo: &'unit Result<Option<Box<DwoUnit<R>>>, Error>| match dwo {
            Ok(Some(dwo)) => Ok((DebugFile::Dwo, dwo.unit_ref())),
            Ok(None) => Ok((DebugFile::Primary, self.unit_ref(&*ctx.sections))),
            Err(e) => Err(*e),
        };
        let complete = |dwo| SimpleLookup::new_complete(map_dwo(dwo));

        if let Some(dwo) = self.dwo.borrow() {
            return complete(dwo);
        }

        let dwo_id = match self.dw_unit.dwo_id {
            None => {
                return complete(self.dwo.borrow_with(|| Ok(None)));
            }
            Some(dwo_id) => dwo_id,
        };

        let comp_dir = self.dw_unit.comp_dir.clone();

        let dwo_name = self.dw_unit.dwo_name().and_then(|s| {
            if let Some(s) = s {
                Ok(Some(ctx.sections.attr_string(&self.dw_unit, s)?))
            } else {
                Ok(None)
            }
        });

        let path = match dwo_name {
            Ok(v) => v,
            Err(e) => {
                return complete(self.dwo.borrow_with(|| Err(e)));
            }
        };

        let process_dwo = move |dwo_dwarf: Option<Arc<gimli::Dwarf<R>>>| {
            let dwo_dwarf = match dwo_dwarf {
                None => return Ok(None),
                Some(dwo_dwarf) => dwo_dwarf,
            };
            let mut dwo_units = dwo_dwarf.units();
            let dwo_header = match dwo_units.next()? {
                Some(dwo_header) => dwo_header,
                None => return Ok(None),
            };

            let mut dwo_unit = dwo_dwarf.unit(dwo_header)?;
            dwo_unit.copy_relocated_attributes(&self.dw_unit);
            Ok(Some(Box::new(DwoUnit {
                sections: dwo_dwarf,
                dw_unit: dwo_unit,
            })))
        };

        SimpleLookup::new_needs_load(
            SplitDwarfLoad {
                dwo_id,
                comp_dir,
                path,
                parent: ctx.sections.clone(),
            },
            move |dwo_dwarf| map_dwo(self.dwo.borrow_with(|| process_dwo(dwo_dwarf))),
        )
    }

    pub(crate) fn parse_lines(&self, sections: &gimli::Dwarf<R>) -> Result<Option<&Lines>, Error> {
        // NB: line information is always stored in the main debug file so this does not need
        // to handle DWOs.
        let ilnp = match self.dw_unit.line_program {
            Some(ref ilnp) => ilnp,
            None => return Ok(None),
        };
        self.lines.borrow(self.unit_ref(sections), ilnp).map(Some)
    }

    pub(crate) fn parse_functions<'unit, 'ctx: 'unit>(
        &'unit self,
        ctx: &'ctx Context<R>,
    ) -> LookupResult<impl LookupContinuation<Output = Result<&'unit Functions<R>, Error>, Buf = R>>
    {
        self.dwarf_and_unit(ctx).map(move |r| {
            let (_file, unit) = r?;
            self.functions.borrow(unit)
        })
    }

    pub(crate) fn parse_inlined_functions<'unit, 'ctx: 'unit>(
        &'unit self,
        ctx: &'ctx Context<R>,
    ) -> LookupResult<impl LookupContinuation<Output = Result<(), Error>, Buf = R> + 'unit> {
        self.dwarf_and_unit(ctx).map(move |r| {
            let (file, unit) = r?;
            self.functions
                .borrow(unit)?
                .parse_inlined_functions(file, unit, ctx)
        })
    }

    pub(crate) fn find_location(
        &self,
        probe: u64,
        sections: &gimli::Dwarf<R>,
    ) -> Result<Option<Location<'_>>, Error> {
        let Some(lines) = self.parse_lines(sections)? else {
            return Ok(None);
        };
        lines.find_location(probe)
    }

    #[inline]
    pub(crate) fn find_location_range(
        &self,
        probe_low: u64,
        probe_high: u64,
        sections: &gimli::Dwarf<R>,
    ) -> Result<Option<LineLocationRangeIter<'_>>, Error> {
        let Some(lines) = self.parse_lines(sections)? else {
            return Ok(None);
        };
        lines.find_location_range(probe_low, probe_high).map(Some)
    }

    pub(crate) fn find_function_or_location<'unit, 'ctx: 'unit>(
        &'unit self,
        probe: u64,
        ctx: &'ctx Context<R>,
    ) -> LookupResult<
        impl LookupContinuation<
            Output = Result<(Option<&'unit Function<R>>, Option<Location<'unit>>), Error>,
            Buf = R,
        >,
    > {
        self.dwarf_and_unit(ctx).map(move |r| {
            let (file, unit) = r?;
            let functions = self.functions.borrow(unit)?;
            let function = match functions.find_address(probe) {
                Some(address) => {
                    let function_index = functions.addresses[address].function;
                    let function = &functions.functions[function_index];
                    Some(function.borrow(file, unit, ctx)?)
                }
                None => None,
            };
            let location = self.find_location(probe, unit.dwarf)?;
            Ok((function, location))
        })
    }
}

pub(crate) struct ResUnits<R: gimli::Reader> {
    ranges: Box<[UnitRange]>,
    units: Box<[ResUnit<R>]>,
}

impl<R: gimli::Reader> ResUnits<R> {
    pub(crate) fn parse(sections: &gimli::Dwarf<R>) -> Result<Self, Error> {
        // Find all the references to compilation units in .debug_aranges.
        // Note that we always also iterate through all of .debug_info to
        // find compilation units, because .debug_aranges may be missing some.
        let mut aranges = Vec::new();
        let mut headers = sections.debug_aranges.headers();
        while let Some(header) = headers.next()? {
            aranges.push((header.debug_info_offset(), header.offset()));
        }
        aranges.sort_by_key(|i| i.0);

        let mut unit_ranges = Vec::new();
        let mut res_units = Vec::new();
        let mut units = sections.units();
        while let Some(header) = units.next()? {
            let unit_id = res_units.len();
            let offset = match header.offset().as_debug_info_offset() {
                Some(offset) => offset,
                None => continue,
            };
            // We mainly want compile units, but we may need to follow references to entries
            // within other units for function names.  We don't need anything from type units.
            let mut need_unit_range = match header.type_() {
                gimli::UnitType::Type { .. } | gimli::UnitType::SplitType { .. } => continue,
                gimli::UnitType::Partial => {
                    // Partial units are only needed for references from other units.
                    // They shouldn't have any address ranges.
                    false
                }
                _ => true,
            };
            let dw_unit = match sections.unit(header) {
                Ok(dw_unit) => dw_unit,
                Err(_) => continue,
            };
            let dw_unit_ref = gimli::UnitRef::new(sections, &dw_unit);

            let mut lang = None;
            if need_unit_range {
                let mut entries = dw_unit_ref.entries_raw(None)?;

                let abbrev = match entries.read_abbreviation()? {
                    Some(abbrev) => abbrev,
                    None => continue,
                };

                let mut ranges = RangeAttributes::default();
                for spec in abbrev.attributes() {
                    let attr = entries.read_attribute(*spec)?;
                    match attr.name() {
                        gimli::DW_AT_low_pc => match attr.value() {
                            gimli::AttributeValue::Addr(val) => ranges.low_pc = Some(val),
                            gimli::AttributeValue::DebugAddrIndex(index) => {
                                ranges.low_pc = Some(dw_unit_ref.address(index)?);
                            }
                            _ => {}
                        },
                        gimli::DW_AT_high_pc => match attr.value() {
                            gimli::AttributeValue::Addr(val) => ranges.high_pc = Some(val),
                            gimli::AttributeValue::DebugAddrIndex(index) => {
                                ranges.high_pc = Some(dw_unit_ref.address(index)?);
                            }
                            gimli::AttributeValue::Udata(val) => ranges.size = Some(val),
                            _ => {}
                        },
                        gimli::DW_AT_ranges => {
                            ranges.ranges_offset = dw_unit_ref.attr_ranges_offset(attr.value())?;
                        }
                        gimli::DW_AT_language => {
                            if let gimli::AttributeValue::Language(val) = attr.value() {
                                lang = Some(val);
                            }
                        }
                        _ => {}
                    }
                }

                // Find the address ranges for the CU, using in order of preference:
                // - DW_AT_ranges
                // - .debug_aranges
                // - DW_AT_low_pc/DW_AT_high_pc
                //
                // Using DW_AT_ranges before .debug_aranges is possibly an arbitrary choice,
                // but the feeling is that DW_AT_ranges is more likely to be reliable or complete
                // if it is present.
                //
                // .debug_aranges must be used before DW_AT_low_pc/DW_AT_high_pc because
                // it has been observed on macOS that DW_AT_ranges was not emitted even for
                // discontiguous CUs.
                let i = match ranges.ranges_offset {
                    Some(_) => None,
                    None => aranges.binary_search_by_key(&offset, |x| x.0).ok(),
                };
                if let Some(mut i) = i {
                    // There should be only one set per CU, but in practice multiple
                    // sets have been observed. This is probably a compiler bug, but
                    // either way we need to handle it.
                    while i > 0 && aranges[i - 1].0 == offset {
                        i -= 1;
                    }
                    for (_, aranges_offset) in aranges[i..].iter().take_while(|x| x.0 == offset) {
                        let aranges_header = sections.debug_aranges.header(*aranges_offset)?;
                        let mut aranges = aranges_header.entries();
                        while let Some(arange) = aranges.next()? {
                            if arange.length() != 0 {
                                unit_ranges.push(UnitRange {
                                    range: arange.range(),
                                    unit_id,
                                    min_begin: 0,
                                });
                                need_unit_range = false;
                            }
                        }
                    }
                } else {
                    need_unit_range &= !ranges.for_each_range(dw_unit_ref, |range| {
                        unit_ranges.push(UnitRange {
                            range,
                            unit_id,
                            min_begin: 0,
                        });
                    })?;
                }
            }

            let lines = LazyLines::new();
            if need_unit_range {
                // The unit did not declare any ranges.
                // Try to get some ranges from the line program sequences.
                if let Some(ref ilnp) = dw_unit_ref.line_program {
                    if let Ok(lines) = lines.borrow(dw_unit_ref, ilnp) {
                        for range in lines.ranges() {
                            unit_ranges.push(UnitRange {
                                range,
                                unit_id,
                                min_begin: 0,
                            })
                        }
                    }
                }
            }

            res_units.push(ResUnit {
                offset,
                dw_unit,
                lang,
                lines,
                functions: LazyFunctions::new(),
                dwo: LazyResult::new(),
            });
        }

        // Sort this for faster lookup in `Self::find_range`.
        unit_ranges.sort_by_key(|i| i.range.end);

        // Calculate the `min_begin` field now that we've determined the order of
        // CUs.
        let mut min = !0;
        for i in unit_ranges.iter_mut().rev() {
            min = min.min(i.range.begin);
            i.min_begin = min;
        }

        Ok(ResUnits {
            ranges: unit_ranges.into_boxed_slice(),
            units: res_units.into_boxed_slice(),
        })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &ResUnit<R>> {
        self.units.iter()
    }

    pub(crate) fn find_offset(
        &self,
        offset: gimli::DebugInfoOffset<R::Offset>,
    ) -> Result<&gimli::Unit<R>, Error> {
        match self
            .units
            .binary_search_by_key(&offset.0, |unit| unit.offset.0)
        {
            // There is never a DIE at the unit offset or before the first unit.
            Ok(_) | Err(0) => Err(gimli::Error::NoEntryAtGivenOffset),
            Err(i) => Ok(&self.units[i - 1].dw_unit),
        }
    }

    /// Finds the CUs for the function address given.
    ///
    /// There might be multiple CUs whose range contains this address.
    /// Weak symbols have shown up in the wild which cause this to happen
    /// but otherwise this can happen if the CU has non-contiguous functions
    /// but only reports a single range.
    ///
    /// Consequently we return an iterator for all CUs which may contain the
    /// address, and the caller must check if there is actually a function or
    /// location in the CU for that address.
    pub(crate) fn find(&self, probe: u64) -> impl Iterator<Item = &ResUnit<R>> {
        self.find_range(probe, probe + 1).map(|(unit, _range)| unit)
    }

    /// Finds the CUs covering the range of addresses given.
    ///
    /// The range is [low, high) (ie, the upper bound is exclusive). This can return multiple
    /// ranges for the same unit.
    #[inline]
    pub(crate) fn find_range(
        &self,
        probe_low: u64,
        probe_high: u64,
    ) -> impl Iterator<Item = (&ResUnit<R>, &gimli::Range)> {
        // Find the position of the next range after a range which
        // ends at `probe_low` or lower.
        let pos = match self
            .ranges
            .binary_search_by_key(&probe_low, |i| i.range.end)
        {
            Ok(i) => i + 1, // Range `i` ends at exactly `probe_low`.
            Err(i) => i,    // Range `i - 1` ends at a lower address.
        };

        // Iterate from that position to find matching CUs.
        self.ranges[pos..]
            .iter()
            .take_while(move |i| {
                // We know that this CU's end is at least `probe_low` because
                // of our sorted array.
                debug_assert!(i.range.end >= probe_low);

                // Each entry keeps track of the minimum begin address for the
                // remainder of the array of unit ranges. If our probe is before
                // the minimum range begin of this entry, then it's guaranteed
                // to not fit in any subsequent entries, so we break out.
                probe_high > i.min_begin
            })
            .filter_map(move |i| {
                // If this CU doesn't actually contain this address, move to the
                // next CU.
                if probe_low >= i.range.end || probe_high <= i.range.begin {
                    return None;
                }
                Some((&self.units[i.unit_id], &i.range))
            })
    }

    pub(crate) fn find_location_range<'a>(
        &'a self,
        probe_low: u64,
        probe_high: u64,
        sections: &'a gimli::Dwarf<R>,
    ) -> Result<LocationRangeIter<'a, R>, Error> {
        let unit_iter = Box::new(self.find_range(probe_low, probe_high));
        Ok(LocationRangeIter {
            unit_iter,
            iter: None,
            probe_low,
            probe_high,
            sections,
        })
    }
}

/// A DWO unit has its own DWARF sections.
struct DwoUnit<R: gimli::Reader> {
    sections: Arc<gimli::Dwarf<R>>,
    dw_unit: gimli::Unit<R>,
}

impl<R: gimli::Reader> DwoUnit<R> {
    fn unit_ref(&self) -> gimli::UnitRef<R> {
        gimli::UnitRef::new(&self.sections, &self.dw_unit)
    }
}

pub(crate) struct SupUnit<R: gimli::Reader> {
    offset: gimli::DebugInfoOffset<R::Offset>,
    dw_unit: gimli::Unit<R>,
}

pub(crate) struct SupUnits<R: gimli::Reader> {
    units: Box<[SupUnit<R>]>,
}

impl<R: gimli::Reader> Default for SupUnits<R> {
    fn default() -> Self {
        SupUnits {
            units: Box::default(),
        }
    }
}

impl<R: gimli::Reader> SupUnits<R> {
    pub(crate) fn parse(sections: &gimli::Dwarf<R>) -> Result<Self, Error> {
        let mut sup_units = Vec::new();
        let mut units = sections.units();
        while let Some(header) = units.next()? {
            let offset = match header.offset().as_debug_info_offset() {
                Some(offset) => offset,
                None => continue,
            };
            let dw_unit = match sections.unit(header) {
                Ok(dw_unit) => dw_unit,
                Err(_) => continue,
            };
            sup_units.push(SupUnit { dw_unit, offset });
        }
        Ok(SupUnits {
            units: sup_units.into_boxed_slice(),
        })
    }

    pub(crate) fn find_offset(
        &self,
        offset: gimli::DebugInfoOffset<R::Offset>,
    ) -> Result<&gimli::Unit<R>, Error> {
        match self
            .units
            .binary_search_by_key(&offset.0, |unit| unit.offset.0)
        {
            // There is never a DIE at the unit offset or before the first unit.
            Ok(_) | Err(0) => Err(gimli::Error::NoEntryAtGivenOffset),
            Err(i) => Ok(&self.units[i - 1].dw_unit),
        }
    }
}

/// Iterator over `Location`s in a range of addresses, returned by `Context::find_location_range`.
pub struct LocationRangeIter<'ctx, R: gimli::Reader> {
    unit_iter: Box<dyn Iterator<Item = (&'ctx ResUnit<R>, &'ctx gimli::Range)> + 'ctx>,
    iter: Option<LineLocationRangeIter<'ctx>>,

    probe_low: u64,
    probe_high: u64,
    sections: &'ctx gimli::Dwarf<R>,
}

impl<'ctx, R: gimli::Reader> LocationRangeIter<'ctx, R> {
    fn next_loc(&mut self) -> Result<Option<(u64, u64, Location<'ctx>)>, Error> {
        loop {
            let iter = self.iter.take();
            match iter {
                None => match self.unit_iter.next() {
                    Some((unit, range)) => {
                        self.iter = unit.find_location_range(
                            cmp::max(self.probe_low, range.begin),
                            cmp::min(self.probe_high, range.end),
                            self.sections,
                        )?;
                    }
                    None => return Ok(None),
                },
                Some(mut iter) => {
                    if let item @ Some(_) = iter.next() {
                        self.iter = Some(iter);
                        return Ok(item);
                    }
                }
            }
        }
    }
}

impl<'ctx, R> Iterator for LocationRangeIter<'ctx, R>
where
    R: gimli::Reader + 'ctx,
{
    type Item = (u64, u64, Location<'ctx>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_loc().unwrap_or_default()
    }
}

#[cfg(feature = "fallible-iterator")]
impl<'ctx, R> fallible_iterator::FallibleIterator for LocationRangeIter<'ctx, R>
where
    R: gimli::Reader + 'ctx,
{
    type Item = (u64, u64, Location<'ctx>);
    type Error = Error;

    #[inline]
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.next_loc()
    }
}
