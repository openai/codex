//! Support for reading PE files.
//!
//! Traits are used to abstract over the difference between PE32 and PE32+.
//! The primary trait for this is [`ImageNtHeaders`].
//!
//! ## High level API
//!
//! [`PeFile`] implements the [`Object`](crate::read::Object) trait for
//! PE files. [`PeFile`] is parameterised by [`ImageNtHeaders`] to allow
//! reading both PE32 and PE32+. There are type aliases for these parameters
//! ([`PeFile32`] and [`PeFile64`]).
//!
//! ## Low level API
//!
//! The [`ImageNtHeaders`] trait can be directly used to parse both
//! [`pe::ImageNtHeaders32`] and [`pe::ImageNtHeaders64`].
//!
//! ### Example for low level API
//!  ```no_run
//! use object::pe;
//! use object::read::pe::ImageNtHeaders;
//! use std::error::Error;
//! use std::fs;
//!
//! /// Reads a file and displays the name of each section.
//! fn main() -> Result<(), Box<dyn Error>> {
//! #   #[cfg(feature = "std")] {
//!     let data = fs::read("path/to/binary")?;
//!     let dos_header = pe::ImageDosHeader::parse(&*data)?;
//!     let mut offset = dos_header.nt_headers_offset().into();
//!     let (nt_headers, data_directories) = pe::ImageNtHeaders64::parse(&*data, &mut offset)?;
//!     let sections = nt_headers.sections(&*data, offset)?;
//!     let symbols = nt_headers.symbols(&*data)?;
//!     for section in sections.iter() {
//!         println!("{}", String::from_utf8_lossy(section.name(symbols.strings())?));
//!     }
//! #   }
//!     Ok(())
//! }
//! ```
#[cfg(doc)]
use crate::pe;

mod file;
pub use file::*;

mod section;
pub use section::*;

mod data_directory;
pub use data_directory::*;

mod export;
pub use export::*;

mod import;
pub use import::*;

mod relocation;
pub use relocation::*;

mod resource;
pub use resource::*;

mod rich;
pub use rich::*;

pub use super::coff::{SectionTable, SymbolTable};
