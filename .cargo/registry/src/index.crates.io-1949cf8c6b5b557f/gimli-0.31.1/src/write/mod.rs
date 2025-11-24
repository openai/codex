//! Write DWARF debugging information.
//!
//! ## API Structure
//!
//! This module works by building up a representation of the debugging information
//! in memory, and then writing it all at once. It supports two major use cases:
//!
//! * Use the [`DwarfUnit`](./struct.DwarfUnit.html) type when writing DWARF
//!   for a single compilation unit.
//!
//! * Use the [`Dwarf`](./struct.Dwarf.html) type when writing DWARF for multiple
//!   compilation units.
//!
//! The module also supports reading in DWARF debugging information and writing it out
//! again, possibly after modifying it. Create a [`read::Dwarf`](../read/struct.Dwarf.html)
//! instance, and then use [`Dwarf::from`](./struct.Dwarf.html#method.from) to convert
//! it to a writable instance.
//!
//! ## Example Usage
//!
//! Write a compilation unit containing only the top level DIE.
//!
//! ```rust
//! use gimli::write::{
//!     Address, AttributeValue, DwarfUnit, EndianVec, Error, Range, RangeList, Sections,
//! };
//!
//! fn example() -> Result<(), Error> {
//!     // Choose the encoding parameters.
//!     let encoding = gimli::Encoding {
//!         format: gimli::Format::Dwarf32,
//!         version: 5,
//!         address_size: 8,
//!     };
//!     // Create a container for a single compilation unit.
//!     let mut dwarf = DwarfUnit::new(encoding);
//!     // Set a range attribute on the root DIE.
//!     let range_list = RangeList(vec![Range::StartLength {
//!         begin: Address::Constant(0x100),
//!         length: 42,
//!     }]);
//!     let range_list_id = dwarf.unit.ranges.add(range_list);
//!     let root = dwarf.unit.root();
//!     dwarf.unit.get_mut(root).set(
//!         gimli::DW_AT_ranges,
//!         AttributeValue::RangeListRef(range_list_id),
//!     );
//!     // Create a `Vec` for each DWARF section.
//!     let mut sections = Sections::new(EndianVec::new(gimli::LittleEndian));
//!     // Finally, write the DWARF data to the sections.
//!     dwarf.write(&mut sections)?;
//!     sections.for_each(|id, data| {
//!         // Here you can add the data to the output object file.
//!         Ok(())
//!     })
//! }
//! # fn main() {
//! #     example().unwrap();
//! # }

use std::error;
use std::fmt;
use std::result;

use crate::constants;

mod endian_vec;
pub use self::endian_vec::*;

mod writer;
pub use self::writer::*;

mod relocate;
pub use self::relocate::*;

#[macro_use]
mod section;
pub use self::section::*;

macro_rules! define_id {
    ($name:ident, $docs:expr) => {
        #[doc=$docs]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            base_id: BaseId,
            index: usize,
        }

        impl $name {
            #[inline]
            fn new(base_id: BaseId, index: usize) -> Self {
                $name { base_id, index }
            }
        }
    };
}

macro_rules! define_offsets {
    ($offsets:ident: $id:ident => $offset:ident, $off_doc:expr) => {
        #[doc=$off_doc]
        #[derive(Debug)]
        pub struct $offsets {
            base_id: BaseId,
            // We know ids start at 0.
            offsets: Vec<$offset>,
        }

        impl $offsets {
            /// Return an empty list of offsets.
            #[inline]
            pub fn none() -> Self {
                $offsets {
                    base_id: BaseId::default(),
                    offsets: Vec::new(),
                }
            }

            /// Get the offset
            ///
            /// # Panics
            ///
            /// Panics if `id` is invalid.
            #[inline]
            pub fn get(&self, id: $id) -> $offset {
                debug_assert_eq!(self.base_id, id.base_id);
                self.offsets[id.index]
            }

            /// Return the number of offsets.
            #[inline]
            pub fn count(&self) -> usize {
                self.offsets.len()
            }
        }
    };
}

mod abbrev;
pub use self::abbrev::*;

mod cfi;
pub use self::cfi::*;

mod dwarf;
pub use self::dwarf::*;

mod line;
pub use self::line::*;

mod loc;
pub use self::loc::*;

mod op;
pub use self::op::*;

mod range;
pub use self::range::*;

mod str;
pub use self::str::*;

mod unit;
pub use self::unit::*;

/// An error that occurred when writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The given offset is out of bounds.
    OffsetOutOfBounds,
    /// The given length is out of bounds.
    LengthOutOfBounds,
    /// The attribute value is an invalid for writing.
    InvalidAttributeValue,
    /// The value is too large for the encoding form.
    ValueTooLarge,
    /// Unsupported word size.
    UnsupportedWordSize(u8),
    /// Unsupported DWARF version.
    UnsupportedVersion(u16),
    /// The unit length is too large for the requested DWARF format.
    InitialLengthOverflow,
    /// The address is invalid.
    InvalidAddress,
    /// The reference is invalid.
    InvalidReference,
    /// A requested feature requires a different DWARF version.
    NeedVersion(u16),
    /// Strings in line number program have mismatched forms.
    LineStringFormMismatch,
    /// The range is empty or otherwise invalid.
    InvalidRange,
    /// The line number program encoding is incompatible with the unit encoding.
    IncompatibleLineProgramEncoding,
    /// Could not encode code offset for a frame instruction.
    InvalidFrameCodeOffset(u32),
    /// Could not encode data offset for a frame instruction.
    InvalidFrameDataOffset(i32),
    /// Unsupported eh_frame pointer encoding.
    UnsupportedPointerEncoding(constants::DwEhPe),
    /// Unsupported reference in CFI expression.
    UnsupportedCfiExpressionReference,
    /// Unsupported forward reference in expression.
    UnsupportedExpressionForwardReference,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> result::Result<(), fmt::Error> {
        match *self {
            Error::OffsetOutOfBounds => write!(f, "The given offset is out of bounds."),
            Error::LengthOutOfBounds => write!(f, "The given length is out of bounds."),
            Error::InvalidAttributeValue => {
                write!(f, "The attribute value is an invalid for writing.")
            }
            Error::ValueTooLarge => write!(f, "The value is too large for the encoding form."),
            Error::UnsupportedWordSize(size) => write!(f, "Unsupported word size: {}", size),
            Error::UnsupportedVersion(version) => {
                write!(f, "Unsupported DWARF version: {}", version)
            }
            Error::InitialLengthOverflow => write!(
                f,
                "The unit length is too large for the requested DWARF format."
            ),
            Error::InvalidAddress => write!(f, "The address is invalid."),
            Error::InvalidReference => write!(f, "The reference is invalid."),
            Error::NeedVersion(version) => write!(
                f,
                "A requested feature requires a DWARF version {}.",
                version
            ),
            Error::LineStringFormMismatch => {
                write!(f, "Strings in line number program have mismatched forms.")
            }
            Error::InvalidRange => write!(f, "The range is empty or otherwise invalid."),
            Error::IncompatibleLineProgramEncoding => write!(
                f,
                "The line number program encoding is incompatible with the unit encoding."
            ),
            Error::InvalidFrameCodeOffset(offset) => write!(
                f,
                "Could not encode code offset ({}) for a frame instruction.",
                offset,
            ),
            Error::InvalidFrameDataOffset(offset) => write!(
                f,
                "Could not encode data offset ({}) for a frame instruction.",
                offset,
            ),
            Error::UnsupportedPointerEncoding(eh_pe) => {
                write!(f, "Unsupported eh_frame pointer encoding ({}).", eh_pe)
            }
            Error::UnsupportedCfiExpressionReference => {
                write!(f, "Unsupported reference in CFI expression.")
            }
            Error::UnsupportedExpressionForwardReference => {
                write!(f, "Unsupported forward reference in expression.")
            }
        }
    }
}

impl error::Error for Error {}

/// The result of a write.
pub type Result<T> = result::Result<T, Error>;

/// An address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Address {
    /// A fixed address that does not require relocation.
    Constant(u64),
    /// An address that is relative to a symbol which may be relocated.
    Symbol {
        /// The symbol that the address is relative to.
        ///
        /// The meaning of this value is decided by the writer, but
        /// will typically be an index into a symbol table.
        symbol: usize,
        /// The offset of the address relative to the symbol.
        ///
        /// This will typically be used as the addend in a relocation.
        addend: i64,
    },
}

/// A reference to a `.debug_info` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reference {
    /// An external symbol.
    ///
    /// The meaning of this value is decided by the writer, but
    /// will typically be an index into a symbol table.
    Symbol(usize),
    /// An entry in the same section.
    ///
    /// This only supports references in units that are emitted together.
    Entry(UnitId, UnitEntryId),
}

// This type is only used in debug assertions.
#[cfg(not(debug_assertions))]
type BaseId = ();

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BaseId(usize);

#[cfg(debug_assertions)]
impl Default for BaseId {
    fn default() -> Self {
        use std::sync::atomic;
        static BASE_ID: atomic::AtomicUsize = atomic::AtomicUsize::new(0);
        BaseId(BASE_ID.fetch_add(1, atomic::Ordering::Relaxed))
    }
}

#[cfg(feature = "read")]
mod convert {
    use super::*;
    use crate::read;

    pub(crate) use super::unit::convert::*;

    /// An error that occurred when converting a read value into a write value.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConvertError {
        /// An error occurred when reading.
        Read(read::Error),
        /// Writing of this attribute value is not implemented yet.
        UnsupportedAttributeValue,
        /// This attribute value is an invalid name/form combination.
        InvalidAttributeValue,
        /// A `.debug_info` reference does not refer to a valid entry.
        InvalidDebugInfoOffset,
        /// An address could not be converted.
        InvalidAddress,
        /// Writing this line number instruction is not implemented yet.
        UnsupportedLineInstruction,
        /// Writing this form of line string is not implemented yet.
        UnsupportedLineStringForm,
        /// A `.debug_line` file index is invalid.
        InvalidFileIndex,
        /// A `.debug_line` directory index is invalid.
        InvalidDirectoryIndex,
        /// A `.debug_line` line base is invalid.
        InvalidLineBase,
        /// A `.debug_line` reference is invalid.
        InvalidLineRef,
        /// A `.debug_info` unit entry reference is invalid.
        InvalidUnitRef,
        /// A `.debug_info` reference is invalid.
        InvalidDebugInfoRef,
        /// Invalid relative address in a range list.
        InvalidRangeRelativeAddress,
        /// Writing this CFI instruction is not implemented yet.
        UnsupportedCfiInstruction,
        /// Writing indirect pointers is not implemented yet.
        UnsupportedIndirectAddress,
        /// Writing this expression operation is not implemented yet.
        UnsupportedOperation,
        /// Operation branch target is invalid.
        InvalidBranchTarget,
        /// Writing this unit type is not supported yet.
        UnsupportedUnitType,
    }

    impl fmt::Display for ConvertError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> result::Result<(), fmt::Error> {
            use self::ConvertError::*;
            match *self {
                Read(ref e) => e.fmt(f),
                UnsupportedAttributeValue => {
                    write!(f, "Writing of this attribute value is not implemented yet.")
                }
                InvalidAttributeValue => write!(
                    f,
                    "This attribute value is an invalid name/form combination."
                ),
                InvalidDebugInfoOffset => write!(
                    f,
                    "A `.debug_info` reference does not refer to a valid entry."
                ),
                InvalidAddress => write!(f, "An address could not be converted."),
                UnsupportedLineInstruction => write!(
                    f,
                    "Writing this line number instruction is not implemented yet."
                ),
                UnsupportedLineStringForm => write!(
                    f,
                    "Writing this form of line string is not implemented yet."
                ),
                InvalidFileIndex => write!(f, "A `.debug_line` file index is invalid."),
                InvalidDirectoryIndex => write!(f, "A `.debug_line` directory index is invalid."),
                InvalidLineBase => write!(f, "A `.debug_line` line base is invalid."),
                InvalidLineRef => write!(f, "A `.debug_line` reference is invalid."),
                InvalidUnitRef => write!(f, "A `.debug_info` unit entry reference is invalid."),
                InvalidDebugInfoRef => write!(f, "A `.debug_info` reference is invalid."),
                InvalidRangeRelativeAddress => {
                    write!(f, "Invalid relative address in a range list.")
                }
                UnsupportedCfiInstruction => {
                    write!(f, "Writing this CFI instruction is not implemented yet.")
                }
                UnsupportedIndirectAddress => {
                    write!(f, "Writing indirect pointers is not implemented yet.")
                }
                UnsupportedOperation => write!(
                    f,
                    "Writing this expression operation is not implemented yet."
                ),
                InvalidBranchTarget => write!(f, "Operation branch target is invalid."),
                UnsupportedUnitType => write!(f, "Writing this unit type is not supported yet."),
            }
        }
    }

    impl error::Error for ConvertError {}

    impl From<read::Error> for ConvertError {
        fn from(e: read::Error) -> Self {
            ConvertError::Read(e)
        }
    }

    /// The result of a conversion.
    pub type ConvertResult<T> = result::Result<T, ConvertError>;
}
#[cfg(feature = "read")]
pub use self::convert::*;
