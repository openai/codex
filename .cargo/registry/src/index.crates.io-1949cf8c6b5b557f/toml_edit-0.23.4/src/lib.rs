//! # `toml_edit`
//!
//! This crate allows you to parse and modify toml
//! documents, while preserving comments, spaces *and
//! relative order* of items.
//!
//! If you also need the ease of a more traditional API, see the [`toml`] crate.
//!
//! # Example
//!
//! ```rust
//! # #[cfg(feature = "parse")] {
//! # #[cfg(feature = "display")] {
//! use toml_edit::{DocumentMut, value};
//!
//! let toml = r#"
//! "hello" = 'toml!' # comment
//! ['a'.b]
//! "#;
//! let mut doc = toml.parse::<DocumentMut>().expect("invalid doc");
//! assert_eq!(doc.to_string(), toml);
//! // let's add a new key/value pair inside a.b: c = {d = "hello"}
//! doc["a"]["b"]["c"]["d"] = value("hello");
//! // autoformat inline table a.b.c: { d = "hello" }
//! doc["a"]["b"]["c"].as_inline_table_mut().map(|t| t.fmt());
//! let expected = r#"
//! "hello" = 'toml!' # comment
//! ['a'.b]
//! c = { d = "hello" }
//! "#;
//! assert_eq!(doc.to_string(), expected);
//! # }
//! # }
//! ```
//!
//! ## Controlling formatting
//!
//! By default, values are created with default formatting
//! ```rust
//! # #[cfg(feature = "display")] {
//! # #[cfg(feature = "parse")] {
//! let mut doc = toml_edit::DocumentMut::new();
//! doc["foo"] = toml_edit::value("bar");
//! let expected = r#"foo = "bar"
//! "#;
//! assert_eq!(doc.to_string(), expected);
//! # }
//! # }
//! ```
//!
//! You can choose a custom TOML representation by parsing the value.
//! ```rust
//! # #[cfg(feature = "display")] {
//! # #[cfg(feature = "parse")] {
//! let mut doc = toml_edit::DocumentMut::new();
//! doc["foo"] = "'bar'".parse::<toml_edit::Item>().unwrap();
//! let expected = r#"foo = 'bar'
//! "#;
//! assert_eq!(doc.to_string(), expected);
//! # }
//! # }
//! ```
//!
//! ## Limitations
//!
//! Things it does not preserve:
//!
//! * Order of dotted keys, see [issue](https://github.com/toml-rs/toml/issues/163).
//!
//! [`toml`]: https://docs.rs/toml/latest/toml/

// https://github.com/Marwes/combine/issues/172
#![recursion_limit = "256"]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

mod array;
mod array_of_tables;
mod document;
#[cfg(feature = "display")]
mod encode;
mod error;
mod index;
mod inline_table;
mod item;
mod key;
#[cfg(feature = "parse")]
mod parser;
mod raw_string;
mod repr;
mod table;
mod value;

#[cfg(feature = "serde")]
pub mod de;
#[cfg(feature = "serde")]
pub mod ser;

pub mod visit;
pub mod visit_mut;

pub use crate::array::{Array, ArrayIntoIter, ArrayIter, ArrayIterMut};
pub use crate::array_of_tables::{
    ArrayOfTables, ArrayOfTablesIntoIter, ArrayOfTablesIter, ArrayOfTablesIterMut,
};
pub use crate::document::DocumentMut;
/// Type representing a parsed TOML document
#[deprecated(since = "0.23.0", note = "Replaced with `Document`")]
pub type ImDocument<S> = Document<S>;
pub use crate::document::Document;
pub use crate::error::TomlError;
pub use crate::inline_table::{
    InlineEntry, InlineOccupiedEntry, InlineTable, InlineTableIntoIter, InlineTableIter,
    InlineTableIterMut, InlineVacantEntry,
};
pub use crate::item::{array, table, value, Item};
pub use crate::key::{Key, KeyMut};
pub use crate::raw_string::RawString;
pub use crate::repr::{Decor, Formatted, Repr};
pub use crate::table::{
    Entry, IntoIter, Iter, IterMut, OccupiedEntry, Table, TableLike, VacantEntry,
};
pub use crate::value::Value;
pub use toml_datetime::*;

// Prevent users from some traits.
pub(crate) mod private {
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for str {}
    impl Sealed for String {}
    impl Sealed for i64 {}
    impl Sealed for f64 {}
    impl Sealed for bool {}
    impl Sealed for crate::Datetime {}
    impl<T: ?Sized> Sealed for &T where T: Sealed {}
    impl Sealed for crate::Table {}
    impl Sealed for crate::InlineTable {}
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
#[cfg(feature = "display")]
#[cfg(feature = "parse")]
pub struct ReadmeDoctests;
