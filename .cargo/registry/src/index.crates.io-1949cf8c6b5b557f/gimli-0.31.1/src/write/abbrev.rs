use alloc::vec::Vec;
use indexmap::IndexSet;
use std::ops::{Deref, DerefMut};

use crate::common::{DebugAbbrevOffset, SectionId};
use crate::constants;
use crate::write::{Result, Section, Writer};

/// A table of abbreviations that will be stored in a `.debug_abbrev` section.
// Requirements:
// - values are `Abbreviation`
// - insertion returns an abbreviation code for use in writing a DIE
// - inserting a duplicate returns the code of the existing value
#[derive(Debug, Default)]
pub(crate) struct AbbreviationTable {
    abbrevs: IndexSet<Abbreviation>,
}

impl AbbreviationTable {
    /// Add an abbreviation to the table and return its code.
    pub fn add(&mut self, abbrev: Abbreviation) -> u64 {
        let (code, _) = self.abbrevs.insert_full(abbrev);
        // Code must be non-zero
        (code + 1) as u64
    }

    /// Write the abbreviation table to the `.debug_abbrev` section.
    pub fn write<W: Writer>(&self, w: &mut DebugAbbrev<W>) -> Result<()> {
        for (code, abbrev) in self.abbrevs.iter().enumerate() {
            w.write_uleb128((code + 1) as u64)?;
            abbrev.write(w)?;
        }
        // Null abbreviation code
        w.write_u8(0)
    }
}

/// An abbreviation describes the shape of a `DebuggingInformationEntry`'s type:
/// its tag type, whether it has children, and its set of attributes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Abbreviation {
    tag: constants::DwTag,
    has_children: bool,
    attributes: Vec<AttributeSpecification>,
}

impl Abbreviation {
    /// Construct a new `Abbreviation`.
    #[inline]
    pub fn new(
        tag: constants::DwTag,
        has_children: bool,
        attributes: Vec<AttributeSpecification>,
    ) -> Abbreviation {
        Abbreviation {
            tag,
            has_children,
            attributes,
        }
    }

    /// Write the abbreviation to the `.debug_abbrev` section.
    pub fn write<W: Writer>(&self, w: &mut DebugAbbrev<W>) -> Result<()> {
        w.write_uleb128(self.tag.0.into())?;
        w.write_u8(if self.has_children {
            constants::DW_CHILDREN_yes.0
        } else {
            constants::DW_CHILDREN_no.0
        })?;
        for attr in &self.attributes {
            attr.write(w)?;
        }
        // Null name and form
        w.write_u8(0)?;
        w.write_u8(0)
    }
}

/// The description of an attribute in an abbreviated type.
// TODO: support implicit const
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AttributeSpecification {
    name: constants::DwAt,
    form: constants::DwForm,
}

impl AttributeSpecification {
    /// Construct a new `AttributeSpecification`.
    #[inline]
    pub fn new(name: constants::DwAt, form: constants::DwForm) -> AttributeSpecification {
        AttributeSpecification { name, form }
    }

    /// Write the attribute specification to the `.debug_abbrev` section.
    #[inline]
    pub fn write<W: Writer>(&self, w: &mut DebugAbbrev<W>) -> Result<()> {
        w.write_uleb128(self.name.0.into())?;
        w.write_uleb128(self.form.0.into())
    }
}

define_section!(
    DebugAbbrev,
    DebugAbbrevOffset,
    "A writable `.debug_abbrev` section."
);

#[cfg(test)]
#[cfg(feature = "read")]
mod tests {
    use super::*;
    use crate::constants;
    use crate::read;
    use crate::write::EndianVec;
    use crate::LittleEndian;

    #[test]
    fn test_abbreviation_table() {
        let mut abbrevs = AbbreviationTable::default();
        let abbrev1 = Abbreviation::new(
            constants::DW_TAG_subprogram,
            false,
            vec![AttributeSpecification::new(
                constants::DW_AT_name,
                constants::DW_FORM_string,
            )],
        );
        let abbrev2 = Abbreviation::new(
            constants::DW_TAG_compile_unit,
            true,
            vec![
                AttributeSpecification::new(constants::DW_AT_producer, constants::DW_FORM_strp),
                AttributeSpecification::new(constants::DW_AT_language, constants::DW_FORM_data2),
            ],
        );
        let code1 = abbrevs.add(abbrev1.clone());
        assert_eq!(code1, 1);
        let code2 = abbrevs.add(abbrev2.clone());
        assert_eq!(code2, 2);
        assert_eq!(abbrevs.add(abbrev1.clone()), code1);
        assert_eq!(abbrevs.add(abbrev2.clone()), code2);

        let mut debug_abbrev = DebugAbbrev::from(EndianVec::new(LittleEndian));
        let debug_abbrev_offset = debug_abbrev.offset();
        assert_eq!(debug_abbrev_offset, DebugAbbrevOffset(0));
        abbrevs.write(&mut debug_abbrev).unwrap();
        assert_eq!(debug_abbrev.offset(), DebugAbbrevOffset(17));

        let read_debug_abbrev = read::DebugAbbrev::new(debug_abbrev.slice(), LittleEndian);
        let read_abbrevs = read_debug_abbrev
            .abbreviations(debug_abbrev_offset)
            .unwrap();

        let read_abbrev1 = read_abbrevs.get(code1).unwrap();
        assert_eq!(abbrev1.tag, read_abbrev1.tag());
        assert_eq!(abbrev1.has_children, read_abbrev1.has_children());
        assert_eq!(abbrev1.attributes.len(), read_abbrev1.attributes().len());
        assert_eq!(
            abbrev1.attributes[0].name,
            read_abbrev1.attributes()[0].name()
        );
        assert_eq!(
            abbrev1.attributes[0].form,
            read_abbrev1.attributes()[0].form()
        );

        let read_abbrev2 = read_abbrevs.get(code2).unwrap();
        assert_eq!(abbrev2.tag, read_abbrev2.tag());
        assert_eq!(abbrev2.has_children, read_abbrev2.has_children());
        assert_eq!(abbrev2.attributes.len(), read_abbrev2.attributes().len());
        assert_eq!(
            abbrev2.attributes[0].name,
            read_abbrev2.attributes()[0].name()
        );
        assert_eq!(
            abbrev2.attributes[0].form,
            read_abbrev2.attributes()[0].form()
        );
        assert_eq!(
            abbrev2.attributes[1].name,
            read_abbrev2.attributes()[1].name()
        );
        assert_eq!(
            abbrev2.attributes[1].form,
            read_abbrev2.attributes()[1].form()
        );
    }
}
