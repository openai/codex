use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::lazy::LazyResult;
use crate::maybe_small;
use crate::{Context, DebugFile, Error, RangeAttributes};

pub(crate) struct LazyFunctions<R: gimli::Reader>(LazyResult<Functions<R>>);

impl<R: gimli::Reader> LazyFunctions<R> {
    pub(crate) fn new() -> Self {
        LazyFunctions(LazyResult::new())
    }

    pub(crate) fn borrow(&self, unit: gimli::UnitRef<R>) -> Result<&Functions<R>, Error> {
        self.0
            .borrow_with(|| Functions::parse(unit))
            .as_ref()
            .map_err(Error::clone)
    }
}

pub(crate) struct Functions<R: gimli::Reader> {
    /// List of all `DW_TAG_subprogram` details in the unit.
    pub(crate) functions: Box<[LazyFunction<R>]>,
    /// List of `DW_TAG_subprogram` address ranges in the unit.
    pub(crate) addresses: Box<[FunctionAddress]>,
}

/// A single address range for a function.
///
/// It is possible for a function to have multiple address ranges; this
/// is handled by having multiple `FunctionAddress` entries with the same
/// `function` field.
pub(crate) struct FunctionAddress {
    range: gimli::Range,
    /// An index into `Functions::functions`.
    pub(crate) function: usize,
}

pub(crate) struct LazyFunction<R: gimli::Reader> {
    dw_die_offset: gimli::UnitOffset<R::Offset>,
    lazy: LazyResult<Function<R>>,
}

impl<R: gimli::Reader> LazyFunction<R> {
    fn new(dw_die_offset: gimli::UnitOffset<R::Offset>) -> Self {
        LazyFunction {
            dw_die_offset,
            lazy: LazyResult::new(),
        }
    }

    pub(crate) fn borrow(
        &self,
        file: DebugFile,
        unit: gimli::UnitRef<R>,
        ctx: &Context<R>,
    ) -> Result<&Function<R>, Error> {
        self.lazy
            .borrow_with(|| Function::parse(self.dw_die_offset, file, unit, ctx))
            .as_ref()
            .map_err(Error::clone)
    }
}

pub(crate) struct Function<R: gimli::Reader> {
    pub(crate) dw_die_offset: gimli::UnitOffset<R::Offset>,
    pub(crate) name: Option<R>,
    /// List of all `DW_TAG_inlined_subroutine` details in this function.
    inlined_functions: Box<[InlinedFunction<R>]>,
    /// List of `DW_TAG_inlined_subroutine` address ranges in this function.
    inlined_addresses: Box<[InlinedFunctionAddress]>,
}

pub(crate) struct InlinedFunctionAddress {
    range: gimli::Range,
    call_depth: usize,
    /// An index into `Function::inlined_functions`.
    function: usize,
}

pub(crate) struct InlinedFunction<R: gimli::Reader> {
    pub(crate) dw_die_offset: gimli::UnitOffset<R::Offset>,
    pub(crate) name: Option<R>,
    pub(crate) call_file: Option<u64>,
    pub(crate) call_line: u32,
    pub(crate) call_column: u32,
}

impl<R: gimli::Reader> Functions<R> {
    fn parse(unit: gimli::UnitRef<R>) -> Result<Functions<R>, Error> {
        let mut functions = Vec::new();
        let mut addresses = Vec::new();
        let mut entries = unit.entries_raw(None)?;
        while !entries.is_empty() {
            let dw_die_offset = entries.next_offset();
            if let Some(abbrev) = entries.read_abbreviation()? {
                if abbrev.tag() == gimli::DW_TAG_subprogram {
                    let mut ranges = RangeAttributes::default();
                    for spec in abbrev.attributes() {
                        match entries.read_attribute(*spec) {
                            Ok(ref attr) => {
                                match attr.name() {
                                    gimli::DW_AT_low_pc => match attr.value() {
                                        gimli::AttributeValue::Addr(val) => {
                                            ranges.low_pc = Some(val)
                                        }
                                        gimli::AttributeValue::DebugAddrIndex(index) => {
                                            ranges.low_pc = Some(unit.address(index)?);
                                        }
                                        _ => {}
                                    },
                                    gimli::DW_AT_high_pc => match attr.value() {
                                        gimli::AttributeValue::Addr(val) => {
                                            ranges.high_pc = Some(val)
                                        }
                                        gimli::AttributeValue::DebugAddrIndex(index) => {
                                            ranges.high_pc = Some(unit.address(index)?);
                                        }
                                        gimli::AttributeValue::Udata(val) => {
                                            ranges.size = Some(val)
                                        }
                                        _ => {}
                                    },
                                    gimli::DW_AT_ranges => {
                                        ranges.ranges_offset =
                                            unit.attr_ranges_offset(attr.value())?;
                                    }
                                    _ => {}
                                };
                            }
                            Err(e) => return Err(e),
                        }
                    }

                    let function_index = functions.len();
                    let has_address = ranges.for_each_range(unit, |range| {
                        addresses.push(FunctionAddress {
                            range,
                            function: function_index,
                        });
                    })?;
                    if has_address {
                        functions.push(LazyFunction::new(dw_die_offset));
                    }
                } else {
                    entries.skip_attributes(abbrev.attributes())?;
                }
            }
        }

        // The binary search requires the addresses to be sorted.
        //
        // It also requires them to be non-overlapping.  In practice, overlapping
        // function ranges are unlikely, so we don't try to handle that yet.
        //
        // It's possible for multiple functions to have the same address range if the
        // compiler can detect and remove functions with identical code.  In that case
        // we'll nondeterministically return one of them.
        addresses.sort_by_key(|x| x.range.begin);

        Ok(Functions {
            functions: functions.into_boxed_slice(),
            addresses: addresses.into_boxed_slice(),
        })
    }

    pub(crate) fn find_address(&self, probe: u64) -> Option<usize> {
        self.addresses
            .binary_search_by(|address| {
                if probe < address.range.begin {
                    Ordering::Greater
                } else if probe >= address.range.end {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            })
            .ok()
    }

    pub(crate) fn parse_inlined_functions(
        &self,
        file: DebugFile,
        unit: gimli::UnitRef<R>,
        ctx: &Context<R>,
    ) -> Result<(), Error> {
        for function in &*self.functions {
            function.borrow(file, unit, ctx)?;
        }
        Ok(())
    }
}

impl<R: gimli::Reader> Function<R> {
    fn parse(
        dw_die_offset: gimli::UnitOffset<R::Offset>,
        file: DebugFile,
        unit: gimli::UnitRef<R>,
        ctx: &Context<R>,
    ) -> Result<Self, Error> {
        let mut entries = unit.entries_raw(Some(dw_die_offset))?;
        let depth = entries.next_depth();
        let abbrev = entries.read_abbreviation()?.unwrap();
        debug_assert_eq!(abbrev.tag(), gimli::DW_TAG_subprogram);

        let mut name = None;
        for spec in abbrev.attributes() {
            match entries.read_attribute(*spec) {
                Ok(ref attr) => {
                    match attr.name() {
                        gimli::DW_AT_linkage_name | gimli::DW_AT_MIPS_linkage_name => {
                            if let Ok(val) = unit.attr_string(attr.value()) {
                                name = Some(val);
                            }
                        }
                        gimli::DW_AT_name => {
                            if name.is_none() {
                                name = unit.attr_string(attr.value()).ok();
                            }
                        }
                        gimli::DW_AT_abstract_origin | gimli::DW_AT_specification => {
                            if name.is_none() {
                                name = name_attr(attr.value(), file, unit, ctx, 16)?;
                            }
                        }
                        _ => {}
                    };
                }
                Err(e) => return Err(e),
            }
        }

        let mut state = InlinedState {
            entries,
            functions: Vec::new(),
            addresses: Vec::new(),
            file,
            unit,
            ctx,
        };
        Function::parse_children(&mut state, depth, 0)?;

        // Sort ranges in "breadth-first traversal order", i.e. first by call_depth
        // and then by range.begin. This allows finding the range containing an
        // address at a certain depth using binary search.
        // Note: Using DFS order, i.e. ordering by range.begin first and then by
        // call_depth, would not work! Consider the two examples
        // "[0..10 at depth 0], [0..2 at depth 1], [6..8 at depth 1]"  and
        // "[0..5 at depth 0], [0..2 at depth 1], [5..10 at depth 0], [6..8 at depth 1]".
        // In this example, if you want to look up address 7 at depth 0, and you
        // encounter [0..2 at depth 1], are you before or after the target range?
        // You don't know.
        state.addresses.sort_by(|r1, r2| {
            if r1.call_depth < r2.call_depth {
                Ordering::Less
            } else if r1.call_depth > r2.call_depth {
                Ordering::Greater
            } else if r1.range.begin < r2.range.begin {
                Ordering::Less
            } else if r1.range.begin > r2.range.begin {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });

        Ok(Function {
            dw_die_offset,
            name,
            inlined_functions: state.functions.into_boxed_slice(),
            inlined_addresses: state.addresses.into_boxed_slice(),
        })
    }

    fn parse_children(
        state: &mut InlinedState<R>,
        depth: isize,
        inlined_depth: usize,
    ) -> Result<(), Error> {
        loop {
            let dw_die_offset = state.entries.next_offset();
            let next_depth = state.entries.next_depth();
            if next_depth <= depth {
                return Ok(());
            }
            if let Some(abbrev) = state.entries.read_abbreviation()? {
                match abbrev.tag() {
                    gimli::DW_TAG_subprogram => {
                        Function::skip(&mut state.entries, abbrev, next_depth)?;
                    }
                    gimli::DW_TAG_inlined_subroutine => {
                        InlinedFunction::parse(
                            state,
                            dw_die_offset,
                            abbrev,
                            next_depth,
                            inlined_depth,
                        )?;
                    }
                    _ => {
                        state.entries.skip_attributes(abbrev.attributes())?;
                    }
                }
            }
        }
    }

    fn skip(
        entries: &mut gimli::EntriesRaw<'_, '_, R>,
        abbrev: &gimli::Abbreviation,
        depth: isize,
    ) -> Result<(), Error> {
        // TODO: use DW_AT_sibling
        entries.skip_attributes(abbrev.attributes())?;
        while entries.next_depth() > depth {
            if let Some(abbrev) = entries.read_abbreviation()? {
                entries.skip_attributes(abbrev.attributes())?;
            }
        }
        Ok(())
    }

    /// Build the list of inlined functions that contain `probe`.
    pub(crate) fn find_inlined_functions(
        &self,
        probe: u64,
    ) -> maybe_small::Vec<&InlinedFunction<R>> {
        // `inlined_functions` is ordered from outside to inside.
        let mut inlined_functions = maybe_small::Vec::new();
        let mut inlined_addresses = &self.inlined_addresses[..];
        loop {
            let current_depth = inlined_functions.len();
            // Look up (probe, current_depth) in inline_ranges.
            // `inlined_addresses` is sorted in "breadth-first traversal order", i.e.
            // by `call_depth` first, and then by `range.begin`. See the comment at
            // the sort call for more information about why.
            let search = inlined_addresses.binary_search_by(|range| {
                if range.call_depth > current_depth {
                    Ordering::Greater
                } else if range.call_depth < current_depth {
                    Ordering::Less
                } else if range.range.begin > probe {
                    Ordering::Greater
                } else if range.range.end <= probe {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });
            if let Ok(index) = search {
                let function_index = inlined_addresses[index].function;
                inlined_functions.push(&self.inlined_functions[function_index]);
                inlined_addresses = &inlined_addresses[index + 1..];
            } else {
                break;
            }
        }
        inlined_functions
    }
}

impl<R: gimli::Reader> InlinedFunction<R> {
    fn parse(
        state: &mut InlinedState<R>,
        dw_die_offset: gimli::UnitOffset<R::Offset>,
        abbrev: &gimli::Abbreviation,
        depth: isize,
        inlined_depth: usize,
    ) -> Result<(), Error> {
        let unit = state.unit;
        let mut ranges = RangeAttributes::default();
        let mut name = None;
        let mut call_file = None;
        let mut call_line = 0;
        let mut call_column = 0;
        for spec in abbrev.attributes() {
            match state.entries.read_attribute(*spec) {
                Ok(ref attr) => match attr.name() {
                    gimli::DW_AT_low_pc => match attr.value() {
                        gimli::AttributeValue::Addr(val) => ranges.low_pc = Some(val),
                        gimli::AttributeValue::DebugAddrIndex(index) => {
                            ranges.low_pc = Some(unit.address(index)?);
                        }
                        _ => {}
                    },
                    gimli::DW_AT_high_pc => match attr.value() {
                        gimli::AttributeValue::Addr(val) => ranges.high_pc = Some(val),
                        gimli::AttributeValue::DebugAddrIndex(index) => {
                            ranges.high_pc = Some(unit.address(index)?);
                        }
                        gimli::AttributeValue::Udata(val) => ranges.size = Some(val),
                        _ => {}
                    },
                    gimli::DW_AT_ranges => {
                        ranges.ranges_offset = unit.attr_ranges_offset(attr.value())?;
                    }
                    gimli::DW_AT_linkage_name | gimli::DW_AT_MIPS_linkage_name => {
                        if let Ok(val) = unit.attr_string(attr.value()) {
                            name = Some(val);
                        }
                    }
                    gimli::DW_AT_name => {
                        if name.is_none() {
                            name = unit.attr_string(attr.value()).ok();
                        }
                    }
                    gimli::DW_AT_abstract_origin | gimli::DW_AT_specification => {
                        if name.is_none() {
                            name = name_attr(attr.value(), state.file, unit, state.ctx, 16)?;
                        }
                    }
                    gimli::DW_AT_call_file => {
                        // There is a spec issue [1] with how DW_AT_call_file is specified in DWARF 5.
                        // Before, a file index of 0 would indicate no source file, however in
                        // DWARF 5 this could be a valid index into the file table.
                        //
                        // Implementations such as LLVM generates a file index of 0 when DWARF 5 is
                        // used.
                        //
                        // Thus, if we see a version of 5 or later, treat a file index of 0 as such.
                        // [1]: http://wiki.dwarfstd.org/index.php?title=DWARF5_Line_Table_File_Numbers
                        if let gimli::AttributeValue::FileIndex(fi) = attr.value() {
                            if fi > 0 || unit.header.version() >= 5 {
                                call_file = Some(fi);
                            }
                        }
                    }
                    gimli::DW_AT_call_line => {
                        call_line = attr.udata_value().unwrap_or(0) as u32;
                    }
                    gimli::DW_AT_call_column => {
                        call_column = attr.udata_value().unwrap_or(0) as u32;
                    }
                    _ => {}
                },
                Err(e) => return Err(e),
            }
        }

        let function_index = state.functions.len();
        state.functions.push(InlinedFunction {
            dw_die_offset,
            name,
            call_file,
            call_line,
            call_column,
        });

        ranges.for_each_range(unit, |range| {
            state.addresses.push(InlinedFunctionAddress {
                range,
                call_depth: inlined_depth,
                function: function_index,
            });
        })?;

        Function::parse_children(state, depth, inlined_depth + 1)
    }
}

struct InlinedState<'a, R: gimli::Reader> {
    // Mutable fields.
    entries: gimli::EntriesRaw<'a, 'a, R>,
    functions: Vec<InlinedFunction<R>>,
    addresses: Vec<InlinedFunctionAddress>,

    // Constant fields.
    file: DebugFile,
    unit: gimli::UnitRef<'a, R>,
    ctx: &'a Context<R>,
}

fn name_attr<R>(
    attr: gimli::AttributeValue<R>,
    mut file: DebugFile,
    unit: gimli::UnitRef<R>,
    ctx: &Context<R>,
    recursion_limit: usize,
) -> Result<Option<R>, Error>
where
    R: gimli::Reader,
{
    if recursion_limit == 0 {
        return Ok(None);
    }

    match attr {
        gimli::AttributeValue::UnitRef(offset) => {
            name_entry(file, unit, offset, ctx, recursion_limit)
        }
        gimli::AttributeValue::DebugInfoRef(dr) => {
            let sections = unit.dwarf;
            let (unit, offset) = ctx.find_unit(dr, file)?;
            let unit = gimli::UnitRef::new(sections, unit);
            name_entry(file, unit, offset, ctx, recursion_limit)
        }
        gimli::AttributeValue::DebugInfoRefSup(dr) => {
            if let Some(sup_sections) = unit.dwarf.sup.as_ref() {
                file = DebugFile::Supplementary;
                let (unit, offset) = ctx.find_unit(dr, file)?;
                let unit = gimli::UnitRef::new(sup_sections, unit);
                name_entry(file, unit, offset, ctx, recursion_limit)
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

fn name_entry<R>(
    file: DebugFile,
    unit: gimli::UnitRef<R>,
    offset: gimli::UnitOffset<R::Offset>,
    ctx: &Context<R>,
    recursion_limit: usize,
) -> Result<Option<R>, Error>
where
    R: gimli::Reader,
{
    let mut entries = unit.entries_raw(Some(offset))?;
    let abbrev = if let Some(abbrev) = entries.read_abbreviation()? {
        abbrev
    } else {
        return Err(gimli::Error::NoEntryAtGivenOffset);
    };

    let mut name = None;
    let mut next = None;
    for spec in abbrev.attributes() {
        match entries.read_attribute(*spec) {
            Ok(ref attr) => match attr.name() {
                gimli::DW_AT_linkage_name | gimli::DW_AT_MIPS_linkage_name => {
                    if let Ok(val) = unit.attr_string(attr.value()) {
                        return Ok(Some(val));
                    }
                }
                gimli::DW_AT_name => {
                    if let Ok(val) = unit.attr_string(attr.value()) {
                        name = Some(val);
                    }
                }
                gimli::DW_AT_abstract_origin | gimli::DW_AT_specification => {
                    next = Some(attr.value());
                }
                _ => {}
            },
            Err(e) => return Err(e),
        }
    }

    if name.is_some() {
        return Ok(name);
    }

    if let Some(next) = next {
        return name_attr(next, file, unit, ctx, recursion_limit - 1);
    }

    Ok(None)
}
