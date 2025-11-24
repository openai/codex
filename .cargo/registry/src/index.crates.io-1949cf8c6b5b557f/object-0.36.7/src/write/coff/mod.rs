//! Support for writing COFF files.
//!
//! Provides [`Writer`] for low level writing of COFF files.
//! This is also used to provide COFF support for [`write::Object`](crate::write::Object).

mod object;
pub use self::object::*;

mod writer;
pub use writer::*;
