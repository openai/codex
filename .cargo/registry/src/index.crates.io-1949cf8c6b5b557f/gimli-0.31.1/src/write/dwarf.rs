use alloc::vec::Vec;

use crate::common::Encoding;
use crate::write::{
    AbbreviationTable, LineProgram, LineStringTable, Result, Sections, StringTable, Unit,
    UnitTable, Writer,
};

/// Writable DWARF information for more than one unit.
#[derive(Debug, Default)]
pub struct Dwarf {
    /// A table of units. These are primarily stored in the `.debug_info` section,
    /// but they also contain information that is stored in other sections.
    pub units: UnitTable,

    /// Extra line number programs that are not associated with a unit.
    ///
    /// These should only be used when generating DWARF5 line-only debug
    /// information.
    pub line_programs: Vec<LineProgram>,

    /// A table of strings that will be stored in the `.debug_line_str` section.
    pub line_strings: LineStringTable,

    /// A table of strings that will be stored in the `.debug_str` section.
    pub strings: StringTable,
}

impl Dwarf {
    /// Create a new `Dwarf` instance.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Write the DWARF information to the given sections.
    pub fn write<W: Writer>(&mut self, sections: &mut Sections<W>) -> Result<()> {
        let line_strings = self.line_strings.write(&mut sections.debug_line_str)?;
        let strings = self.strings.write(&mut sections.debug_str)?;
        self.units.write(sections, &line_strings, &strings)?;
        for line_program in &self.line_programs {
            line_program.write(
                &mut sections.debug_line,
                line_program.encoding(),
                &line_strings,
                &strings,
            )?;
        }
        Ok(())
    }
}

/// Writable DWARF information for a single unit.
#[derive(Debug)]
pub struct DwarfUnit {
    /// A unit. This is primarily stored in the `.debug_info` section,
    /// but also contains information that is stored in other sections.
    pub unit: Unit,

    /// A table of strings that will be stored in the `.debug_line_str` section.
    pub line_strings: LineStringTable,

    /// A table of strings that will be stored in the `.debug_str` section.
    pub strings: StringTable,
}

impl DwarfUnit {
    /// Create a new `DwarfUnit`.
    ///
    /// Note: you should set `self.unit.line_program` after creation.
    /// This cannot be done earlier because it may need to reference
    /// `self.line_strings`.
    pub fn new(encoding: Encoding) -> Self {
        let unit = Unit::new(encoding, LineProgram::none());
        DwarfUnit {
            unit,
            line_strings: LineStringTable::default(),
            strings: StringTable::default(),
        }
    }

    /// Write the DWARf information to the given sections.
    pub fn write<W: Writer>(&mut self, sections: &mut Sections<W>) -> Result<()> {
        let line_strings = self.line_strings.write(&mut sections.debug_line_str)?;
        let strings = self.strings.write(&mut sections.debug_str)?;

        let abbrev_offset = sections.debug_abbrev.offset();
        let mut abbrevs = AbbreviationTable::default();

        self.unit.write(
            sections,
            abbrev_offset,
            &mut abbrevs,
            &line_strings,
            &strings,
        )?;
        // None should exist because we didn't give out any UnitId.
        assert!(sections.debug_info_refs.is_empty());
        assert!(sections.debug_loc_refs.is_empty());
        assert!(sections.debug_loclists_refs.is_empty());

        abbrevs.write(&mut sections.debug_abbrev)?;
        Ok(())
    }
}

#[cfg(feature = "read")]
pub(crate) mod convert {
    use super::*;
    use crate::read::{self, Reader};
    use crate::write::{Address, ConvertResult};

    impl Dwarf {
        /// Create a `write::Dwarf` by converting a `read::Dwarf`.
        ///
        /// `convert_address` is a function to convert read addresses into the `Address`
        /// type. For non-relocatable addresses, this function may simply return
        /// `Address::Constant(address)`. For relocatable addresses, it is the caller's
        /// responsibility to determine the symbol and addend corresponding to the address
        /// and return `Address::Symbol { symbol, addend }`.
        pub fn from<R: Reader<Offset = usize>>(
            dwarf: &read::Dwarf<R>,
            convert_address: &dyn Fn(u64) -> Option<Address>,
        ) -> ConvertResult<Dwarf> {
            let mut line_strings = LineStringTable::default();
            let mut strings = StringTable::default();
            let units = UnitTable::from(dwarf, &mut line_strings, &mut strings, convert_address)?;
            // TODO: convert the line programs that were not referenced by a unit.
            let line_programs = Vec::new();
            Ok(Dwarf {
                units,
                line_programs,
                line_strings,
                strings,
            })
        }
    }
}
