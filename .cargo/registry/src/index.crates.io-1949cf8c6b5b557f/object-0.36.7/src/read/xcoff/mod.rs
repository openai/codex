//! Support for reading AIX XCOFF files.
//!
//! Traits are used to abstract over the difference between 32-bit and 64-bit XCOFF.
//! The primary trait for this is [`FileHeader`].
//!
//! ## High level API
//!
//! [`XcoffFile`] implements the [`Object`](crate::read::Object) trait for XCOFF files.
//! [`XcoffFile`] is parameterised by [`FileHeader`] to allow reading both 32-bit and
//! 64-bit XCOFF. There are type aliases for these parameters ([`XcoffFile32`] and
//! [`XcoffFile64`]).
//!
//! ## Low level API
//!
//! The [`FileHeader`] trait can be directly used to parse both [`xcoff::FileHeader32`]
//! and [`xcoff::FileHeader64`].
//!
//! ### Example for low level API
//!  ```no_run
//! use object::xcoff;
//! use object::read::xcoff::{FileHeader, SectionHeader, Symbol};
//! use std::error::Error;
//! use std::fs;
//!
//! /// Reads a file and displays the name of each section and symbol.
//! fn main() -> Result<(), Box<dyn Error>> {
//! #   #[cfg(feature = "std")] {
//!     let data = fs::read("path/to/binary")?;
//!     let mut offset = 0;
//!     let header = xcoff::FileHeader64::parse(&*data, &mut offset)?;
//!     let aux_header = header.aux_header(&*data, &mut offset)?;
//!     let sections = header.sections(&*data, &mut offset)?;
//!     let symbols = header.symbols(&*data)?;
//!     for section in sections.iter() {
//!         println!("{}", String::from_utf8_lossy(section.name()));
//!     }
//!     for (_index, symbol) in symbols.iter() {
//!         println!("{}", String::from_utf8_lossy(symbol.name(symbols.strings())?));
//!     }
//! #   }
//!     Ok(())
//! }
//! ```
#[cfg(doc)]
use crate::xcoff;

mod file;
pub use file::*;

mod section;
pub use section::*;

mod symbol;
pub use symbol::*;

mod relocation;
pub use relocation::*;

mod comdat;
pub use comdat::*;

mod segment;
pub use segment::*;
