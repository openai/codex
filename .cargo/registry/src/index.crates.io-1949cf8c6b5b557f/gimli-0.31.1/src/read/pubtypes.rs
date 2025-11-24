use crate::common::{DebugInfoOffset, SectionId};
use crate::endianity::Endianity;
use crate::read::lookup::{DebugLookup, LookupEntryIter, PubStuffEntry, PubStuffParser};
use crate::read::{EndianSlice, Reader, Result, Section, UnitOffset};

/// A single parsed pubtype.
#[derive(Debug, Clone)]
pub struct PubTypesEntry<R: Reader> {
    unit_header_offset: DebugInfoOffset<R::Offset>,
    die_offset: UnitOffset<R::Offset>,
    name: R,
}

impl<R: Reader> PubTypesEntry<R> {
    /// Returns the name of the type this entry refers to.
    pub fn name(&self) -> &R {
        &self.name
    }

    /// Returns the offset into the .debug_info section for the header of the compilation unit
    /// which contains the type with this name.
    pub fn unit_header_offset(&self) -> DebugInfoOffset<R::Offset> {
        self.unit_header_offset
    }

    /// Returns the offset into the compilation unit for the debugging information entry which
    /// the type with this name.
    pub fn die_offset(&self) -> UnitOffset<R::Offset> {
        self.die_offset
    }
}

impl<R: Reader> PubStuffEntry<R> for PubTypesEntry<R> {
    fn new(
        die_offset: UnitOffset<R::Offset>,
        name: R,
        unit_header_offset: DebugInfoOffset<R::Offset>,
    ) -> Self {
        PubTypesEntry {
            unit_header_offset,
            die_offset,
            name,
        }
    }
}

/// The `DebugPubTypes` struct represents the DWARF public types information
/// found in the `.debug_info` section.
#[derive(Debug, Clone)]
pub struct DebugPubTypes<R: Reader>(DebugLookup<R, PubStuffParser<R, PubTypesEntry<R>>>);

impl<'input, Endian> DebugPubTypes<EndianSlice<'input, Endian>>
where
    Endian: Endianity,
{
    /// Construct a new `DebugPubTypes` instance from the data in the `.debug_pubtypes`
    /// section.
    ///
    /// It is the caller's responsibility to read the `.debug_pubtypes` section and
    /// present it as a `&[u8]` slice. That means using some ELF loader on
    /// Linux, a Mach-O loader on macOS, etc.
    ///
    /// ```
    /// use gimli::{DebugPubTypes, LittleEndian};
    ///
    /// # let buf = [];
    /// # let read_debug_pubtypes_somehow = || &buf;
    /// let debug_pubtypes =
    ///     DebugPubTypes::new(read_debug_pubtypes_somehow(), LittleEndian);
    /// ```
    pub fn new(debug_pubtypes_section: &'input [u8], endian: Endian) -> Self {
        Self::from(EndianSlice::new(debug_pubtypes_section, endian))
    }
}

impl<R: Reader> DebugPubTypes<R> {
    /// Iterate the pubtypes in the `.debug_pubtypes` section.
    ///
    /// ```
    /// use gimli::{DebugPubTypes, EndianSlice, LittleEndian};
    ///
    /// # let buf = [];
    /// # let read_debug_pubtypes_section_somehow = || &buf;
    /// let debug_pubtypes =
    ///     DebugPubTypes::new(read_debug_pubtypes_section_somehow(), LittleEndian);
    ///
    /// let mut iter = debug_pubtypes.items();
    /// while let Some(pubtype) = iter.next().unwrap() {
    ///   println!("pubtype {} found!", pubtype.name().to_string_lossy());
    /// }
    /// ```
    pub fn items(&self) -> PubTypesEntryIter<R> {
        PubTypesEntryIter(self.0.items())
    }
}

impl<R: Reader> Section<R> for DebugPubTypes<R> {
    fn id() -> SectionId {
        SectionId::DebugPubTypes
    }

    fn reader(&self) -> &R {
        self.0.reader()
    }
}

impl<R: Reader> From<R> for DebugPubTypes<R> {
    fn from(debug_pubtypes_section: R) -> Self {
        DebugPubTypes(DebugLookup::from(debug_pubtypes_section))
    }
}

/// An iterator over the pubtypes from a `.debug_pubtypes` section.
///
/// Can be [used with
/// `FallibleIterator`](./index.html#using-with-fallibleiterator).
#[derive(Debug, Clone)]
pub struct PubTypesEntryIter<R: Reader>(LookupEntryIter<R, PubStuffParser<R, PubTypesEntry<R>>>);

impl<R: Reader> PubTypesEntryIter<R> {
    /// Advance the iterator and return the next pubtype.
    ///
    /// Returns the newly parsed pubtype as `Ok(Some(pubtype))`. Returns
    /// `Ok(None)` when iteration is complete and all pubtypes have already been
    /// parsed and yielded. If an error occurs while parsing the next pubtype,
    /// then this error is returned as `Err(e)`, and all subsequent calls return
    /// `Ok(None)`.
    pub fn next(&mut self) -> Result<Option<PubTypesEntry<R>>> {
        self.0.next()
    }
}

#[cfg(feature = "fallible-iterator")]
impl<R: Reader> fallible_iterator::FallibleIterator for PubTypesEntryIter<R> {
    type Item = PubTypesEntry<R>;
    type Error = crate::read::Error;

    fn next(&mut self) -> ::core::result::Result<Option<Self::Item>, Self::Error> {
        self.0.next()
    }
}
