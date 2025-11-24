//! Read DWARF debugging information.
//!
//! * [Example Usage](#example-usage)
//! * [API Structure](#api-structure)
//! * [Using with `FallibleIterator`](#using-with-fallibleiterator)
//!
//! ## Example Usage
//!
//! Print out all of the functions in the debuggee program:
//!
//! ```rust,no_run
//! # fn example() -> Result<(), gimli::Error> {
//! # type R = gimli::EndianSlice<'static, gimli::LittleEndian>;
//! # let get_file_section_reader = |name| -> Result<R, gimli::Error> { unimplemented!() };
//! # let get_sup_file_section_reader = |name| -> Result<R, gimli::Error> { unimplemented!() };
//! // Read the DWARF sections with whatever object loader you're using.
//! // These closures should return a `Reader` instance (e.g. `EndianSlice`).
//! let loader = |section: gimli::SectionId| { get_file_section_reader(section.name()) };
//! let sup_loader = |section: gimli::SectionId| { get_sup_file_section_reader(section.name()) };
//! let mut dwarf = gimli::Dwarf::load(loader)?;
//! dwarf.load_sup(sup_loader)?;
//!
//! // Iterate over all compilation units.
//! let mut iter = dwarf.units();
//! while let Some(header) = iter.next()? {
//!     // Parse the abbreviations and other information for this compilation unit.
//!     let unit = dwarf.unit(header)?;
//!
//!     // Iterate over all of this compilation unit's entries.
//!     let mut entries = unit.entries();
//!     while let Some((_, entry)) = entries.next_dfs()? {
//!         // If we find an entry for a function, print it.
//!         if entry.tag() == gimli::DW_TAG_subprogram {
//!             println!("Found a function: {:?}", entry);
//!         }
//!     }
//! }
//! # unreachable!()
//! # }
//! ```
//!
//! Full example programs:
//!
//!   * [A simple parser](https://github.com/gimli-rs/gimli/blob/master/crates/examples/src/bin/simple.rs)
//!
//!   * [A `dwarfdump`
//!     clone](https://github.com/gimli-rs/gimli/blob/master/crates/examples/src/bin/dwarfdump.rs)
//!
//!   * [An `addr2line` clone](https://github.com/gimli-rs/addr2line)
//!
//!   * [`ddbug`](https://github.com/gimli-rs/ddbug), a utility giving insight into
//!     code generation by making debugging information readable
//!
//!   * [`dwprod`](https://github.com/fitzgen/dwprod), a tiny utility to list the
//!     compilers used to create each compilation unit within a shared library or
//!     executable (via `DW_AT_producer`)
//!
//!   * [`dwarf-validate`](https://github.com/gimli-rs/gimli/blob/master/crates/examples/src/bin/dwarf-validate.rs),
//!     a program to validate the integrity of some DWARF and its references
//!     between sections and compilation units.
//!
//! ## API Structure
//!
//! * Basic familiarity with DWARF is assumed.
//!
//! * The [`Dwarf`](./struct.Dwarf.html) type contains the commonly used DWARF
//!   sections. It has methods that simplify access to debugging data that spans
//!   multiple sections. Use of this type is optional, but recommended.
//!
//! * The [`DwarfPackage`](./struct.Dwarf.html) type contains the DWARF
//!   package (DWP) sections. It has methods to find a DWARF object (DWO)
//!   within the package.
//!
//! * Each section gets its own type. Consider these types the entry points to
//!   the library:
//!
//!   * [`DebugAbbrev`](./struct.DebugAbbrev.html): The `.debug_abbrev` section.
//!
//!   * [`DebugAddr`](./struct.DebugAddr.html): The `.debug_addr` section.
//!
//!   * [`DebugAranges`](./struct.DebugAranges.html): The `.debug_aranges`
//!     section.
//!
//!   * [`DebugFrame`](./struct.DebugFrame.html): The `.debug_frame` section.
//!
//!   * [`DebugInfo`](./struct.DebugInfo.html): The `.debug_info` section.
//!
//!   * [`DebugLine`](./struct.DebugLine.html): The `.debug_line` section.
//!
//!   * [`DebugLineStr`](./struct.DebugLineStr.html): The `.debug_line_str` section.
//!
//!   * [`DebugLoc`](./struct.DebugLoc.html): The `.debug_loc` section.
//!
//!   * [`DebugLocLists`](./struct.DebugLocLists.html): The `.debug_loclists` section.
//!
//!   * [`DebugPubNames`](./struct.DebugPubNames.html): The `.debug_pubnames`
//!     section.
//!
//!   * [`DebugPubTypes`](./struct.DebugPubTypes.html): The `.debug_pubtypes`
//!     section.
//!
//!   * [`DebugRanges`](./struct.DebugRanges.html): The `.debug_ranges` section.
//!
//!   * [`DebugRngLists`](./struct.DebugRngLists.html): The `.debug_rnglists` section.
//!
//!   * [`DebugStr`](./struct.DebugStr.html): The `.debug_str` section.
//!
//!   * [`DebugStrOffsets`](./struct.DebugStrOffsets.html): The `.debug_str_offsets` section.
//!
//!   * [`DebugTypes`](./struct.DebugTypes.html): The `.debug_types` section.
//!
//!   * [`DebugCuIndex`](./struct.DebugCuIndex.html): The `.debug_cu_index` section.
//!
//!   * [`DebugTuIndex`](./struct.DebugTuIndex.html): The `.debug_tu_index` section.
//!
//!   * [`EhFrame`](./struct.EhFrame.html): The `.eh_frame` section.
//!
//!   * [`EhFrameHdr`](./struct.EhFrameHdr.html): The `.eh_frame_hdr` section.
//!
//! * Each section type exposes methods for accessing the debugging data encoded
//!   in that section. For example, the [`DebugInfo`](./struct.DebugInfo.html)
//!   struct has the [`units`](./struct.DebugInfo.html#method.units) method for
//!   iterating over the compilation units defined within it.
//!
//! * Offsets into a section are strongly typed: an offset into `.debug_info` is
//!   the [`DebugInfoOffset`](./struct.DebugInfoOffset.html) type. It cannot be
//!   used to index into the [`DebugLine`](./struct.DebugLine.html) type because
//!   `DebugLine` represents the `.debug_line` section. There are similar types
//!   for offsets relative to a compilation unit rather than a section.
//!
//! ## Using with `FallibleIterator`
//!
//! The standard library's `Iterator` trait and related APIs do not play well
//! with iterators where the `next` operation is fallible. One can make the
//! `Iterator`'s associated `Item` type be a `Result<T, E>`, however the
//! provided methods cannot gracefully handle the case when an `Err` is
//! returned.
//!
//! This situation led to the
//! [`fallible-iterator`](https://crates.io/crates/fallible-iterator) crate's
//! existence. You can read more of the rationale for its existence in its
//! docs. The crate provides the helpers you have come to expect (eg `map`,
//! `filter`, etc) for iterators that can fail.
//!
//! `gimli`'s many lazy parsing iterators are a perfect match for the
//! `fallible-iterator` crate's `FallibleIterator` trait because parsing is not
//! done eagerly. Parse errors later in the input might only be discovered after
//! having iterated through many items.
//!
//! To use `gimli` iterators with `FallibleIterator`, import the crate and trait
//! into your code:
//!
//! ```
//! # #[cfg(feature = "fallible-iterator")]
//! # fn foo() {
//! // Use the `FallibleIterator` trait so its methods are in scope!
//! use fallible_iterator::FallibleIterator;
//! use gimli::{DebugAranges, EndianSlice, LittleEndian};
//!
//! fn find_sum_of_address_range_lengths(aranges: DebugAranges<EndianSlice<LittleEndian>>)
//!     -> gimli::Result<u64>
//! {
//!     // `DebugAranges::headers` returns a `FallibleIterator`!
//!     aranges.headers()
//!         // `flat_map` is provided by `FallibleIterator`!
//!         .flat_map(|header| Ok(header.entries()))
//!         // `map` is provided by `FallibleIterator`!
//!         .map(|arange| Ok(arange.length()))
//!         // `fold` is provided by `FallibleIterator`!
//!         .fold(0, |sum, len| Ok(sum + len))
//! }
//! # }
//! # fn main() {}
//! ```

use core::fmt::{self, Debug};
use core::result;
#[cfg(feature = "std")]
use std::{error, io};

use crate::common::{Register, SectionId};
use crate::constants;

mod util;
pub use util::*;

mod addr;
pub use self::addr::*;

mod cfi;
pub use self::cfi::*;

#[cfg(feature = "read")]
mod dwarf;
#[cfg(feature = "read")]
pub use self::dwarf::*;

mod endian_slice;
pub use self::endian_slice::*;

#[cfg(feature = "endian-reader")]
mod endian_reader;
#[cfg(feature = "endian-reader")]
pub use self::endian_reader::*;

mod reader;
pub use self::reader::*;

mod relocate;
pub use self::relocate::*;

#[cfg(feature = "read")]
mod abbrev;
#[cfg(feature = "read")]
pub use self::abbrev::*;

mod aranges;
pub use self::aranges::*;

mod index;
pub use self::index::*;

#[cfg(feature = "read")]
mod line;
#[cfg(feature = "read")]
pub use self::line::*;

mod lists;

mod loclists;
pub use self::loclists::*;

#[cfg(feature = "read")]
mod lookup;

mod op;
pub use self::op::*;

#[cfg(feature = "read")]
mod pubnames;
#[cfg(feature = "read")]
pub use self::pubnames::*;

#[cfg(feature = "read")]
mod pubtypes;
#[cfg(feature = "read")]
pub use self::pubtypes::*;

mod rnglists;
pub use self::rnglists::*;

mod str;
pub use self::str::*;

/// An offset into the current compilation or type unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct UnitOffset<T = usize>(pub T);

#[cfg(feature = "read")]
mod unit;
#[cfg(feature = "read")]
pub use self::unit::*;

mod value;
pub use self::value::*;

/// Indicates that storage should be allocated on heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreOnHeap;

/// `EndianBuf` has been renamed to `EndianSlice`. For ease of upgrading across
/// `gimli` versions, we export this type alias.
#[deprecated(note = "EndianBuf has been renamed to EndianSlice, use that instead.")]
pub type EndianBuf<'input, Endian> = EndianSlice<'input, Endian>;

/// An error that occurred when parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// An I/O error occurred while reading.
    Io,
    /// Found a PC relative pointer, but the section base is undefined.
    PcRelativePointerButSectionBaseIsUndefined,
    /// Found a `.text` relative pointer, but the `.text` base is undefined.
    TextRelativePointerButTextBaseIsUndefined,
    /// Found a data relative pointer, but the data base is undefined.
    DataRelativePointerButDataBaseIsUndefined,
    /// Found a function relative pointer in a context that does not have a
    /// function base.
    FuncRelativePointerInBadContext,
    /// Cannot parse a pointer with a `DW_EH_PE_omit` encoding.
    CannotParseOmitPointerEncoding,
    /// An error parsing an unsigned LEB128 value.
    BadUnsignedLeb128,
    /// An error parsing a signed LEB128 value.
    BadSignedLeb128,
    /// An abbreviation declared that its tag is zero, but zero is reserved for
    /// null records.
    AbbreviationTagZero,
    /// An attribute specification declared that its form is zero, but zero is
    /// reserved for null records.
    AttributeFormZero,
    /// The abbreviation's has-children byte was not one of
    /// `DW_CHILDREN_{yes,no}`.
    BadHasChildren,
    /// The specified length is impossible.
    BadLength,
    /// Found an unknown `DW_FORM_*` type.
    UnknownForm(constants::DwForm),
    /// Expected a zero, found something else.
    ExpectedZero,
    /// Found an abbreviation code that has already been used.
    DuplicateAbbreviationCode,
    /// Found a duplicate arange.
    DuplicateArange,
    /// Found an unknown reserved length value.
    UnknownReservedLength,
    /// Found an unknown DWARF version.
    UnknownVersion(u64),
    /// Found a record with an unknown abbreviation code.
    UnknownAbbreviation(u64),
    /// Hit the end of input before it was expected.
    UnexpectedEof(ReaderOffsetId),
    /// Read a null entry before it was expected.
    UnexpectedNull,
    /// Found an unknown standard opcode.
    UnknownStandardOpcode(constants::DwLns),
    /// Found an unknown extended opcode.
    UnknownExtendedOpcode(constants::DwLne),
    /// Found an unknown location-lists format.
    UnknownLocListsEntry(constants::DwLle),
    /// Found an unknown range-lists format.
    UnknownRangeListsEntry(constants::DwRle),
    /// The specified address size is not supported.
    UnsupportedAddressSize(u8),
    /// The specified offset size is not supported.
    UnsupportedOffsetSize(u8),
    /// The specified field size is not supported.
    UnsupportedFieldSize(u8),
    /// The minimum instruction length must not be zero.
    MinimumInstructionLengthZero,
    /// The maximum operations per instruction must not be zero.
    MaximumOperationsPerInstructionZero,
    /// The line range must not be zero.
    LineRangeZero,
    /// The opcode base must not be zero.
    OpcodeBaseZero,
    /// Found an invalid UTF-8 string.
    BadUtf8,
    /// Expected to find the CIE ID, but found something else.
    NotCieId,
    /// Expected to find a pointer to a CIE, but found the CIE ID instead.
    NotCiePointer,
    /// Expected to find a pointer to an FDE, but found a CIE instead.
    NotFdePointer,
    /// Invalid branch target for a DW_OP_bra or DW_OP_skip.
    BadBranchTarget(u64),
    /// DW_OP_push_object_address used but no address passed in.
    InvalidPushObjectAddress,
    /// Not enough items on the stack when evaluating an expression.
    NotEnoughStackItems,
    /// Too many iterations to compute the expression.
    TooManyIterations,
    /// An unrecognized operation was found while parsing a DWARF
    /// expression.
    InvalidExpression(constants::DwOp),
    /// An unsupported operation was found while evaluating a DWARF expression.
    UnsupportedEvaluation,
    /// The expression had a piece followed by an expression
    /// terminator without a piece.
    InvalidPiece,
    /// An expression-terminating operation was followed by something
    /// other than the end of the expression or a piece operation.
    InvalidExpressionTerminator(u64),
    /// Division or modulus by zero when evaluating an expression.
    DivisionByZero,
    /// An expression operation used mismatching types.
    TypeMismatch,
    /// An expression operation required an integral type but saw a
    /// floating point type.
    IntegralTypeRequired,
    /// An expression operation used types that are not supported.
    UnsupportedTypeOperation,
    /// The shift value in an expression must be a non-negative integer.
    InvalidShiftExpression,
    /// The size of a deref expression must not be larger than the size of an address.
    InvalidDerefSize(u8),
    /// An unknown DW_CFA_* instruction.
    UnknownCallFrameInstruction(constants::DwCfa),
    /// The end of an address range was before the beginning.
    InvalidAddressRange,
    /// An address calculation overflowed.
    ///
    /// This is returned in cases where the address is expected to be
    /// larger than a previous address, but the calculation overflowed.
    AddressOverflow,
    /// Encountered a call frame instruction in a context in which it is not
    /// valid.
    CfiInstructionInInvalidContext,
    /// When evaluating call frame instructions, found a `DW_CFA_restore_state`
    /// stack pop instruction, but the stack was empty, and had nothing to pop.
    PopWithEmptyStack,
    /// Do not have unwind info for the given address.
    NoUnwindInfoForAddress,
    /// An offset value was larger than the maximum supported value.
    UnsupportedOffset,
    /// The given pointer encoding is either unknown or invalid.
    UnknownPointerEncoding(constants::DwEhPe),
    /// Did not find an entry at the given offset.
    NoEntryAtGivenOffset,
    /// The given offset is out of bounds.
    OffsetOutOfBounds,
    /// Found an unknown CFI augmentation.
    UnknownAugmentation,
    /// We do not support the given pointer encoding yet.
    UnsupportedPointerEncoding,
    /// Registers larger than `u16` are not supported.
    UnsupportedRegister(u64),
    /// The CFI program defined more register rules than we have storage for.
    TooManyRegisterRules,
    /// Attempted to push onto the CFI or evaluation stack, but it was already
    /// at full capacity.
    StackFull,
    /// The `.eh_frame_hdr` binary search table claims to be variable-length encoded,
    /// which makes binary search impossible.
    VariableLengthSearchTable,
    /// The `DW_UT_*` value for this unit is not supported yet.
    UnsupportedUnitType,
    /// Ranges using AddressIndex are not supported yet.
    UnsupportedAddressIndex,
    /// Nonzero segment selector sizes aren't supported yet.
    UnsupportedSegmentSize,
    /// A compilation unit or type unit is missing its top level DIE.
    MissingUnitDie,
    /// A DIE attribute used an unsupported form.
    UnsupportedAttributeForm,
    /// Missing DW_LNCT_path in file entry format.
    MissingFileEntryFormatPath,
    /// Expected an attribute value to be a string form.
    ExpectedStringAttributeValue,
    /// `DW_FORM_implicit_const` used in an invalid context.
    InvalidImplicitConst,
    /// Invalid section count in `.dwp` index.
    InvalidIndexSectionCount,
    /// Invalid slot count in `.dwp` index.
    InvalidIndexSlotCount,
    /// Invalid hash row in `.dwp` index.
    InvalidIndexRow,
    /// Unknown section type in `.dwp` index.
    UnknownIndexSection(constants::DwSect),
    /// Unknown section type in version 2 `.dwp` index.
    UnknownIndexSectionV2(constants::DwSectV2),
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> ::core::result::Result<(), fmt::Error> {
        write!(f, "{}", self.description())
    }
}

impl Error {
    /// A short description of the error.
    pub fn description(&self) -> &str {
        match *self {
            Error::Io => "An I/O error occurred while reading.",
            Error::PcRelativePointerButSectionBaseIsUndefined => {
                "Found a PC relative pointer, but the section base is undefined."
            }
            Error::TextRelativePointerButTextBaseIsUndefined => {
                "Found a `.text` relative pointer, but the `.text` base is undefined."
            }
            Error::DataRelativePointerButDataBaseIsUndefined => {
                "Found a data relative pointer, but the data base is undefined."
            }
            Error::FuncRelativePointerInBadContext => {
                "Found a function relative pointer in a context that does not have a function base."
            }
            Error::CannotParseOmitPointerEncoding => {
                "Cannot parse a pointer with a `DW_EH_PE_omit` encoding."
            }
            Error::BadUnsignedLeb128 => "An error parsing an unsigned LEB128 value",
            Error::BadSignedLeb128 => "An error parsing a signed LEB128 value",
            Error::AbbreviationTagZero => {
                "An abbreviation declared that its tag is zero,
                 but zero is reserved for null records"
            }
            Error::AttributeFormZero => {
                "An attribute specification declared that its form is zero,
                 but zero is reserved for null records"
            }
            Error::BadHasChildren => {
                "The abbreviation's has-children byte was not one of
                 `DW_CHILDREN_{yes,no}`"
            }
            Error::BadLength => "The specified length is impossible",
            Error::UnknownForm(_) => "Found an unknown `DW_FORM_*` type",
            Error::ExpectedZero => "Expected a zero, found something else",
            Error::DuplicateAbbreviationCode => {
                "Found an abbreviation code that has already been used"
            }
            Error::DuplicateArange => "Found a duplicate arange",
            Error::UnknownReservedLength => "Found an unknown reserved length value",
            Error::UnknownVersion(_) => "Found an unknown DWARF version",
            Error::UnknownAbbreviation(_) => "Found a record with an unknown abbreviation code",
            Error::UnexpectedEof(_) => "Hit the end of input before it was expected",
            Error::UnexpectedNull => "Read a null entry before it was expected.",
            Error::UnknownStandardOpcode(_) => "Found an unknown standard opcode",
            Error::UnknownExtendedOpcode(_) => "Found an unknown extended opcode",
            Error::UnknownLocListsEntry(_) => "Found an unknown location lists entry",
            Error::UnknownRangeListsEntry(_) => "Found an unknown range lists entry",
            Error::UnsupportedAddressSize(_) => "The specified address size is not supported",
            Error::UnsupportedOffsetSize(_) => "The specified offset size is not supported",
            Error::UnsupportedFieldSize(_) => "The specified field size is not supported",
            Error::MinimumInstructionLengthZero => {
                "The minimum instruction length must not be zero."
            }
            Error::MaximumOperationsPerInstructionZero => {
                "The maximum operations per instruction must not be zero."
            }
            Error::LineRangeZero => "The line range must not be zero.",
            Error::OpcodeBaseZero => "The opcode base must not be zero.",
            Error::BadUtf8 => "Found an invalid UTF-8 string.",
            Error::NotCieId => "Expected to find the CIE ID, but found something else.",
            Error::NotCiePointer => "Expected to find a CIE pointer, but found the CIE ID instead.",
            Error::NotFdePointer => {
                "Expected to find an FDE pointer, but found a CIE pointer instead."
            }
            Error::BadBranchTarget(_) => "Invalid branch target in DWARF expression",
            Error::InvalidPushObjectAddress => {
                "DW_OP_push_object_address used but no object address given"
            }
            Error::NotEnoughStackItems => "Not enough items on stack when evaluating expression",
            Error::TooManyIterations => "Too many iterations to evaluate DWARF expression",
            Error::InvalidExpression(_) => "Invalid opcode in DWARF expression",
            Error::UnsupportedEvaluation => "Unsupported operation when evaluating expression",
            Error::InvalidPiece => {
                "DWARF expression has piece followed by non-piece expression at end"
            }
            Error::InvalidExpressionTerminator(_) => "Expected DW_OP_piece or DW_OP_bit_piece",
            Error::DivisionByZero => "Division or modulus by zero when evaluating expression",
            Error::TypeMismatch => "Type mismatch when evaluating expression",
            Error::IntegralTypeRequired => "Integral type expected when evaluating expression",
            Error::UnsupportedTypeOperation => {
                "An expression operation used types that are not supported"
            }
            Error::InvalidShiftExpression => {
                "The shift value in an expression must be a non-negative integer."
            }
            Error::InvalidDerefSize(_) => {
                "The size of a deref expression must not be larger than the size of an address."
            }
            Error::UnknownCallFrameInstruction(_) => "An unknown DW_CFA_* instructiion",
            Error::InvalidAddressRange => {
                "The end of an address range must not be before the beginning."
            }
            Error::AddressOverflow => "An address calculation overflowed.",
            Error::CfiInstructionInInvalidContext => {
                "Encountered a call frame instruction in a context in which it is not valid."
            }
            Error::PopWithEmptyStack => {
                "When evaluating call frame instructions, found a `DW_CFA_restore_state` stack pop \
                 instruction, but the stack was empty, and had nothing to pop."
            }
            Error::NoUnwindInfoForAddress => "Do not have unwind info for the given address.",
            Error::UnsupportedOffset => {
                "An offset value was larger than the maximum supported value."
            }
            Error::UnknownPointerEncoding(_) => {
                "The given pointer encoding is either unknown or invalid."
            }
            Error::NoEntryAtGivenOffset => "Did not find an entry at the given offset.",
            Error::OffsetOutOfBounds => "The given offset is out of bounds.",
            Error::UnknownAugmentation => "Found an unknown CFI augmentation.",
            Error::UnsupportedPointerEncoding => {
                "We do not support the given pointer encoding yet."
            }
            Error::UnsupportedRegister(_) => "Registers larger than `u16` are not supported.",
            Error::TooManyRegisterRules => {
                "The CFI program defined more register rules than we have storage for."
            }
            Error::StackFull => {
                "Attempted to push onto the CFI stack, but it was already at full capacity."
            }
            Error::VariableLengthSearchTable => {
                "The `.eh_frame_hdr` binary search table claims to be variable-length encoded, \
                 which makes binary search impossible."
            }
            Error::UnsupportedUnitType => "The `DW_UT_*` value for this unit is not supported yet",
            Error::UnsupportedAddressIndex => "Ranges involving AddressIndex are not supported yet",
            Error::UnsupportedSegmentSize => "Nonzero segment size not supported yet",
            Error::MissingUnitDie => {
                "A compilation unit or type unit is missing its top level DIE."
            }
            Error::UnsupportedAttributeForm => "A DIE attribute used an unsupported form.",
            Error::MissingFileEntryFormatPath => "Missing DW_LNCT_path in file entry format.",
            Error::ExpectedStringAttributeValue => {
                "Expected an attribute value to be a string form."
            }
            Error::InvalidImplicitConst => "DW_FORM_implicit_const used in an invalid context.",
            Error::InvalidIndexSectionCount => "Invalid section count in `.dwp` index.",
            Error::InvalidIndexSlotCount => "Invalid slot count in `.dwp` index.",
            Error::InvalidIndexRow => "Invalid hash row in `.dwp` index.",
            Error::UnknownIndexSection(_) => "Unknown section type in `.dwp` index.",
            Error::UnknownIndexSectionV2(_) => "Unknown section type in version 2 `.dwp` index.",
        }
    }
}

#[cfg(feature = "std")]
impl error::Error for Error {}

#[cfg(feature = "std")]
impl From<io::Error> for Error {
    fn from(_: io::Error) -> Self {
        Error::Io
    }
}

/// The result of a parse.
pub type Result<T> = result::Result<T, Error>;

/// A convenience trait for loading DWARF sections from object files.  To be
/// used like:
///
/// ```
/// use gimli::{DebugInfo, EndianSlice, LittleEndian, Reader, Section};
///
/// let buf = [0x00, 0x01, 0x02, 0x03];
/// let reader = EndianSlice::new(&buf, LittleEndian);
/// let loader = |name| -> Result<_, ()> { Ok(reader) };
///
/// let debug_info: DebugInfo<_> = Section::load(loader).unwrap();
/// ```
pub trait Section<R>: From<R> {
    /// Returns the section id for this type.
    fn id() -> SectionId;

    /// Returns the ELF section name for this type.
    fn section_name() -> &'static str {
        Self::id().name()
    }

    /// Returns the ELF section name (if any) for this type when used in a dwo
    /// file.
    fn dwo_section_name() -> Option<&'static str> {
        Self::id().dwo_name()
    }

    /// Returns the XCOFF section name (if any) for this type when used in a XCOFF
    /// file.
    fn xcoff_section_name() -> Option<&'static str> {
        Self::id().xcoff_name()
    }

    /// Try to load the section using the given loader function.
    fn load<F, E>(f: F) -> core::result::Result<Self, E>
    where
        F: FnOnce(SectionId) -> core::result::Result<R, E>,
    {
        f(Self::id()).map(From::from)
    }

    /// Returns the `Reader` for this section.
    fn reader(&self) -> &R
    where
        R: Reader;

    /// Returns the subrange of the section that is the contribution of
    /// a unit in a `.dwp` file.
    fn dwp_range(&self, offset: u32, size: u32) -> Result<Self>
    where
        R: Reader,
    {
        let mut data = self.reader().clone();
        data.skip(R::Offset::from_u32(offset))?;
        data.truncate(R::Offset::from_u32(size))?;
        Ok(data.into())
    }

    /// Returns the `Reader` for this section.
    fn lookup_offset_id(&self, id: ReaderOffsetId) -> Option<(SectionId, R::Offset)>
    where
        R: Reader,
    {
        self.reader()
            .lookup_offset_id(id)
            .map(|offset| (Self::id(), offset))
    }
}

impl Register {
    pub(crate) fn from_u64(x: u64) -> Result<Register> {
        let y = x as u16;
        if u64::from(y) == x {
            Ok(Register(y))
        } else {
            Err(Error::UnsupportedRegister(x))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Format;
    use crate::endianity::LittleEndian;
    use test_assembler::{Endian, Section};

    #[test]
    fn test_parse_initial_length_32_ok() {
        let section = Section::with_endian(Endian::Little).L32(0x7856_3412);
        let buf = section.get_contents().unwrap();

        let input = &mut EndianSlice::new(&buf, LittleEndian);
        match input.read_initial_length() {
            Ok((length, format)) => {
                assert_eq!(input.len(), 0);
                assert_eq!(format, Format::Dwarf32);
                assert_eq!(0x7856_3412, length);
            }
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        }
    }

    #[test]
    fn test_parse_initial_length_64_ok() {
        let section = Section::with_endian(Endian::Little)
            // Dwarf_64_INITIAL_UNIT_LENGTH
            .L32(0xffff_ffff)
            // Actual length
            .L64(0xffde_bc9a_7856_3412);
        let buf = section.get_contents().unwrap();
        let input = &mut EndianSlice::new(&buf, LittleEndian);

        #[cfg(target_pointer_width = "64")]
        match input.read_initial_length() {
            Ok((length, format)) => {
                assert_eq!(input.len(), 0);
                assert_eq!(format, Format::Dwarf64);
                assert_eq!(0xffde_bc9a_7856_3412, length);
            }
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        }

        #[cfg(target_pointer_width = "32")]
        match input.read_initial_length() {
            Err(Error::UnsupportedOffset) => {}
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        };
    }

    #[test]
    fn test_parse_initial_length_unknown_reserved_value() {
        let section = Section::with_endian(Endian::Little).L32(0xffff_fffe);
        let buf = section.get_contents().unwrap();

        let input = &mut EndianSlice::new(&buf, LittleEndian);
        match input.read_initial_length() {
            Err(Error::UnknownReservedLength) => {}
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        };
    }

    #[test]
    fn test_parse_initial_length_incomplete() {
        let buf = [0xff, 0xff, 0xff]; // Need at least 4 bytes.

        let input = &mut EndianSlice::new(&buf, LittleEndian);
        match input.read_initial_length() {
            Err(Error::UnexpectedEof(_)) => {}
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        };
    }

    #[test]
    fn test_parse_initial_length_64_incomplete() {
        let section = Section::with_endian(Endian::Little)
            // Dwarf_64_INITIAL_UNIT_LENGTH
            .L32(0xffff_ffff)
            // Actual length is not long enough.
            .L32(0x7856_3412);
        let buf = section.get_contents().unwrap();

        let input = &mut EndianSlice::new(&buf, LittleEndian);
        match input.read_initial_length() {
            Err(Error::UnexpectedEof(_)) => {}
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        };
    }

    #[test]
    fn test_parse_offset_32() {
        let section = Section::with_endian(Endian::Little).L32(0x0123_4567);
        let buf = section.get_contents().unwrap();

        let input = &mut EndianSlice::new(&buf, LittleEndian);
        match input.read_offset(Format::Dwarf32) {
            Ok(val) => {
                assert_eq!(input.len(), 0);
                assert_eq!(val, 0x0123_4567);
            }
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        };
    }

    #[test]
    fn test_parse_offset_64_small() {
        let section = Section::with_endian(Endian::Little).L64(0x0123_4567);
        let buf = section.get_contents().unwrap();

        let input = &mut EndianSlice::new(&buf, LittleEndian);
        match input.read_offset(Format::Dwarf64) {
            Ok(val) => {
                assert_eq!(input.len(), 0);
                assert_eq!(val, 0x0123_4567);
            }
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        };
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn test_parse_offset_64_large() {
        let section = Section::with_endian(Endian::Little).L64(0x0123_4567_89ab_cdef);
        let buf = section.get_contents().unwrap();

        let input = &mut EndianSlice::new(&buf, LittleEndian);
        match input.read_offset(Format::Dwarf64) {
            Ok(val) => {
                assert_eq!(input.len(), 0);
                assert_eq!(val, 0x0123_4567_89ab_cdef);
            }
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        };
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn test_parse_offset_64_large() {
        let section = Section::with_endian(Endian::Little).L64(0x0123_4567_89ab_cdef);
        let buf = section.get_contents().unwrap();

        let input = &mut EndianSlice::new(&buf, LittleEndian);
        match input.read_offset(Format::Dwarf64) {
            Err(Error::UnsupportedOffset) => {}
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        };
    }
}
