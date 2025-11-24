//! A low-level interface for writing out TOML
//!
//! Considerations when serializing arbitrary data:
//! - Verify the implementation with [`toml-test-harness`](https://docs.rs/toml-test-harness)
//! - Be sure to group keys under a table before writing another table
//! - Watch for extra trailing newlines and leading newlines, both when starting with top-level
//!   keys or a table
//! - When serializing an array-of-tables, be sure to verify that all elements of the array
//!   serialize as tables
//! - Standard tables and inline tables may need separate implementations of corner cases,
//!   requiring verifying them both
//!
//! When serializing Rust data structures
//! - `Option`: Skip key-value pairs with a value of `None`, otherwise error when seeing `None`
//!   - When skipping key-value pairs, be careful that a deeply nested `None` doesn't get skipped
//! - Scalars and arrays are unsupported as top-level data types
//! - Tuples and tuple variants seriallize as arrays
//! - Structs, struct variants, and maps serialize as tables
//! - Newtype variants serialize as to the inner type
//! - Unit variants serialize to a string
//! - Unit and unit structs don't have a clear meaning in TOML
//!
//! # Example
//!
//! ```rust
//! use toml_writer::TomlWrite as _;
//!
//! # fn main() -> std::fmt::Result {
//! let mut output = String::new();
//! output.newline()?;
//! output.open_table_header()?;
//! output.key("table")?;
//! output.close_table_header()?;
//! output.newline()?;
//!
//! output.key("key")?;
//! output.space()?;
//! output.keyval_sep()?;
//! output.space()?;
//! output.value("value")?;
//! output.newline()?;
//!
//! assert_eq!(output, r#"
//! [table]
//! key = "value"
//! "#);
//! #   Ok(())
//! # }
//! ```

#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![forbid(unsafe_code)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::std_instead_of_alloc)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod integer;
mod key;
mod string;
mod value;
mod write;

pub use integer::TomlInteger;
pub use integer::TomlIntegerFormat;
#[cfg(feature = "alloc")]
pub use key::ToTomlKey;
pub use key::WriteTomlKey;
pub use string::TomlKey;
pub use string::TomlKeyBuilder;
pub use string::TomlString;
pub use string::TomlStringBuilder;
#[cfg(feature = "alloc")]
pub use value::ToTomlValue;
pub use value::WriteTomlValue;
pub use write::TomlWrite;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
