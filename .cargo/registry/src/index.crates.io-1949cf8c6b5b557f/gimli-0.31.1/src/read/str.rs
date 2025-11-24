use crate::common::{
    DebugLineStrOffset, DebugStrOffset, DebugStrOffsetsBase, DebugStrOffsetsIndex, DwarfFileType,
    Encoding, SectionId,
};
use crate::endianity::Endianity;
use crate::read::{EndianSlice, Reader, ReaderOffset, Result, Section};
use crate::Format;

/// The `DebugStr` struct represents the DWARF strings
/// found in the `.debug_str` section.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebugStr<R> {
    debug_str_section: R,
}

impl<'input, Endian> DebugStr<EndianSlice<'input, Endian>>
where
    Endian: Endianity,
{
    /// Construct a new `DebugStr` instance from the data in the `.debug_str`
    /// section.
    ///
    /// It is the caller's responsibility to read the `.debug_str` section and
    /// present it as a `&[u8]` slice. That means using some ELF loader on
    /// Linux, a Mach-O loader on macOS, etc.
    ///
    /// ```
    /// use gimli::{DebugStr, LittleEndian};
    ///
    /// # let buf = [0x00, 0x01, 0x02, 0x03];
    /// # let read_debug_str_section_somehow = || &buf;
    /// let debug_str = DebugStr::new(read_debug_str_section_somehow(), LittleEndian);
    /// ```
    pub fn new(debug_str_section: &'input [u8], endian: Endian) -> Self {
        Self::from(EndianSlice::new(debug_str_section, endian))
    }
}

impl<R: Reader> DebugStr<R> {
    /// Lookup a string from the `.debug_str` section by DebugStrOffset.
    ///
    /// ```
    /// use gimli::{DebugStr, DebugStrOffset, LittleEndian};
    ///
    /// # let buf = [0x01, 0x02, 0x00];
    /// # let offset = DebugStrOffset(0);
    /// # let read_debug_str_section_somehow = || &buf;
    /// # let debug_str_offset_somehow = || offset;
    /// let debug_str = DebugStr::new(read_debug_str_section_somehow(), LittleEndian);
    /// println!("Found string {:?}", debug_str.get_str(debug_str_offset_somehow()));
    /// ```
    pub fn get_str(&self, offset: DebugStrOffset<R::Offset>) -> Result<R> {
        let input = &mut self.debug_str_section.clone();
        input.skip(offset.0)?;
        input.read_null_terminated_slice()
    }
}

impl<T> DebugStr<T> {
    /// Create a `DebugStr` section that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// Used by `DwarfSections::borrow`.
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F) -> DebugStr<R>
    where
        F: FnMut(&'a T) -> R,
    {
        borrow(&self.debug_str_section).into()
    }
}

impl<R> Section<R> for DebugStr<R> {
    fn id() -> SectionId {
        SectionId::DebugStr
    }

    fn reader(&self) -> &R {
        &self.debug_str_section
    }
}

impl<R> From<R> for DebugStr<R> {
    fn from(debug_str_section: R) -> Self {
        DebugStr { debug_str_section }
    }
}

/// The raw contents of the `.debug_str_offsets` section.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebugStrOffsets<R> {
    section: R,
}

impl<R: Reader> DebugStrOffsets<R> {
    // TODO: add an iterator over the sets of entries in the section.
    // This is not needed for common usage of the section though.

    /// Returns the `.debug_str` offset at the given `base` and `index`.
    ///
    /// A set of entries in the `.debug_str_offsets` section consists of a header
    /// followed by a series of string table offsets.
    ///
    /// The `base` must be the `DW_AT_str_offsets_base` value from the compilation unit DIE.
    /// This is an offset that points to the first entry following the header.
    ///
    /// The `index` is the value of a `DW_FORM_strx` attribute.
    ///
    /// The `format` must be the DWARF format of the compilation unit. This format must
    /// match the header. However, note that we do not parse the header to validate this,
    /// since locating the header is unreliable, and the GNU extensions do not emit it.
    pub fn get_str_offset(
        &self,
        format: Format,
        base: DebugStrOffsetsBase<R::Offset>,
        index: DebugStrOffsetsIndex<R::Offset>,
    ) -> Result<DebugStrOffset<R::Offset>> {
        let input = &mut self.section.clone();
        input.skip(base.0)?;
        input.skip(R::Offset::from_u64(
            index.0.into_u64() * u64::from(format.word_size()),
        )?)?;
        input.read_offset(format).map(DebugStrOffset)
    }
}

impl<T> DebugStrOffsets<T> {
    /// Create a `DebugStrOffsets` section that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// Used by `DwarfSections::borrow`.
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F) -> DebugStrOffsets<R>
    where
        F: FnMut(&'a T) -> R,
    {
        borrow(&self.section).into()
    }
}

impl<R> Section<R> for DebugStrOffsets<R> {
    fn id() -> SectionId {
        SectionId::DebugStrOffsets
    }

    fn reader(&self) -> &R {
        &self.section
    }
}

impl<R> From<R> for DebugStrOffsets<R> {
    fn from(section: R) -> Self {
        DebugStrOffsets { section }
    }
}

impl<Offset> DebugStrOffsetsBase<Offset>
where
    Offset: ReaderOffset,
{
    /// Returns a `DebugStrOffsetsBase` with the default value of DW_AT_str_offsets_base
    /// for the given `Encoding` and `DwarfFileType`.
    pub fn default_for_encoding_and_file(
        encoding: Encoding,
        file_type: DwarfFileType,
    ) -> DebugStrOffsetsBase<Offset> {
        if encoding.version >= 5 && file_type == DwarfFileType::Dwo {
            // In .dwo files, the compiler omits the DW_AT_str_offsets_base attribute (because there is
            // only a single unit in the file) but we must skip past the header, which the attribute
            // would normally do for us.
            // initial_length_size + version + 2 bytes of padding.
            DebugStrOffsetsBase(Offset::from_u8(
                encoding.format.initial_length_size() + 2 + 2,
            ))
        } else {
            DebugStrOffsetsBase(Offset::from_u8(0))
        }
    }
}

/// The `DebugLineStr` struct represents the DWARF strings
/// found in the `.debug_line_str` section.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebugLineStr<R> {
    section: R,
}

impl<'input, Endian> DebugLineStr<EndianSlice<'input, Endian>>
where
    Endian: Endianity,
{
    /// Construct a new `DebugLineStr` instance from the data in the `.debug_line_str`
    /// section.
    ///
    /// It is the caller's responsibility to read the `.debug_line_str` section and
    /// present it as a `&[u8]` slice. That means using some ELF loader on
    /// Linux, a Mach-O loader on macOS, etc.
    ///
    /// ```
    /// use gimli::{DebugLineStr, LittleEndian};
    ///
    /// # let buf = [0x00, 0x01, 0x02, 0x03];
    /// # let read_debug_line_str_section_somehow = || &buf;
    /// let debug_str = DebugLineStr::new(read_debug_line_str_section_somehow(), LittleEndian);
    /// ```
    pub fn new(debug_line_str_section: &'input [u8], endian: Endian) -> Self {
        Self::from(EndianSlice::new(debug_line_str_section, endian))
    }
}

impl<R: Reader> DebugLineStr<R> {
    /// Lookup a string from the `.debug_line_str` section by DebugLineStrOffset.
    pub fn get_str(&self, offset: DebugLineStrOffset<R::Offset>) -> Result<R> {
        let input = &mut self.section.clone();
        input.skip(offset.0)?;
        input.read_null_terminated_slice()
    }
}

impl<T> DebugLineStr<T> {
    /// Create a `DebugLineStr` section that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// Used by `DwarfSections::borrow`.
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F) -> DebugLineStr<R>
    where
        F: FnMut(&'a T) -> R,
    {
        borrow(&self.section).into()
    }
}

impl<R> Section<R> for DebugLineStr<R> {
    fn id() -> SectionId {
        SectionId::DebugLineStr
    }

    fn reader(&self) -> &R {
        &self.section
    }
}

impl<R> From<R> for DebugLineStr<R> {
    fn from(section: R) -> Self {
        DebugLineStr { section }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::GimliSectionMethods;
    use crate::LittleEndian;
    use test_assembler::{Endian, Label, LabelMaker, Section};

    #[test]
    fn test_get_str_offset() {
        for format in [Format::Dwarf32, Format::Dwarf64] {
            let zero = Label::new();
            let length = Label::new();
            let start = Label::new();
            let first = Label::new();
            let end = Label::new();
            let mut section = Section::with_endian(Endian::Little)
                .mark(&zero)
                .initial_length(format, &length, &start)
                .D16(5)
                .D16(0)
                .mark(&first);
            for i in 0..20 {
                section = section.word(format.word_size(), 1000 + i);
            }
            section = section.mark(&end);
            length.set_const((&end - &start) as u64);

            let section = section.get_contents().unwrap();
            let debug_str_offsets = DebugStrOffsets::from(EndianSlice::new(&section, LittleEndian));
            let base = DebugStrOffsetsBase((&first - &zero) as usize);

            assert_eq!(
                debug_str_offsets.get_str_offset(format, base, DebugStrOffsetsIndex(0)),
                Ok(DebugStrOffset(1000))
            );
            assert_eq!(
                debug_str_offsets.get_str_offset(format, base, DebugStrOffsetsIndex(19)),
                Ok(DebugStrOffset(1019))
            );
        }
    }
}
