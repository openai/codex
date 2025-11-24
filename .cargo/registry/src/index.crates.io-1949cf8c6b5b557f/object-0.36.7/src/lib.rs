//! # `object`
//!
//! The `object` crate provides a unified interface to working with object files
//! across platforms. It supports reading relocatable object files and executable files,
//! and writing relocatable object files and some executable files.
//!
//! ## Raw struct definitions
//!
//! Raw structs are defined for: [ELF](elf), [Mach-O](macho), [PE/COFF](pe),
//! [XCOFF](xcoff), [archive].
//! Types and traits for zerocopy support are defined in the [`pod`] and [`endian`] modules.
//!
//! ## Unified read API
//!
//! The [`read`] module provides a unified read API using the [`read::Object`] trait.
//! There is an implementation of this trait for [`read::File`], which allows reading any
//! file format, as well as implementations for each file format.
//!
//! ## Low level read API
//!
//! The [`read#modules`] submodules define helpers that operate on the raw structs.
//! These can be used instead of the unified API, or in conjunction with it to access
//! details that are not available via the unified API.
//!
//! ## Unified write API
//!
//! The [`mod@write`] module provides a unified write API for relocatable object files
//! using [`write::Object`]. This does not support writing executable files.
//!
//! ## Low level write API
//!
//! The [`mod@write#modules`] submodules define helpers for writing the raw structs.
//!
//! ## Build API
//!
//! The [`mod@build`] submodules define helpers for building object files, either from
//! scratch or by modifying existing files.
//!
//! ## Shared definitions
//!
//! The crate provides a number of definitions that are used by both the read and write
//! APIs. These are defined at the top level module, but none of these are the main entry
//! points of the crate.

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![no_std]
#![warn(rust_2018_idioms)]
// Style.
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::manual_flatten)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::single_match)]
#![allow(clippy::type_complexity)]
// Occurs due to fallible iteration.
#![allow(clippy::should_implement_trait)]
// Unit errors are converted to other types by callers.
#![allow(clippy::result_unit_err)]

#[cfg(feature = "cargo-all")]
compile_error!("'--all-features' is not supported; use '--features all' instead");

#[cfg(any(feature = "read_core", feature = "write_core"))]
#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

#[cfg(feature = "std")]
#[allow(unused_imports)]
#[macro_use]
extern crate std;

mod common;
pub use common::*;

#[macro_use]
pub mod endian;
pub use endian::*;

#[macro_use]
pub mod pod;
pub use pod::*;

#[cfg(feature = "read_core")]
pub mod read;
#[cfg(feature = "read_core")]
pub use read::*;

#[cfg(feature = "write_core")]
pub mod write;

#[cfg(feature = "build_core")]
pub mod build;

#[cfg(feature = "archive")]
pub mod archive;
#[cfg(feature = "elf")]
pub mod elf;
#[cfg(feature = "macho")]
pub mod macho;
#[cfg(any(feature = "coff", feature = "pe"))]
pub mod pe;
#[cfg(feature = "xcoff")]
pub mod xcoff;
