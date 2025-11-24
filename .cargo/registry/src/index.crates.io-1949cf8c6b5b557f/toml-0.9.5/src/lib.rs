//! A [serde]-compatible [TOML]-parsing library
//!
//! TOML itself is a simple, ergonomic, and readable configuration format:
//!
//! ```toml
//! [package]
//! name = "toml"
//!
//! [dependencies]
//! serde = "1.0"
//! ```
//!
//! The TOML format tends to be relatively common throughout the Rust community
//! for configuration, notably being used by [Cargo], Rust's package manager.
//!
//! ## TOML values
//!
//! A TOML document is represented with the [`Table`] type which maps `String` to the [`Value`] enum:
//!
#![cfg_attr(not(feature = "default"), doc = " ```ignore")]
#![cfg_attr(feature = "default", doc = " ```")]
//! # use toml::value::{Datetime, Array, Table};
//! pub enum Value {
//!     String(String),
//!     Integer(i64),
//!     Float(f64),
//!     Boolean(bool),
//!     Datetime(Datetime),
//!     Array(Array),
//!     Table(Table),
//! }
//! ```
//!
//! ## Parsing TOML
//!
//! The easiest way to parse a TOML document is via the [`Table`] type:
//!
#![cfg_attr(not(feature = "default"), doc = " ```ignore")]
#![cfg_attr(feature = "default", doc = " ```")]
//! use toml::Table;
//!
//! let value = "foo = 'bar'".parse::<Table>().unwrap();
//!
//! assert_eq!(value["foo"].as_str(), Some("bar"));
//! ```
//!
//! The [`Table`] type implements a number of convenience methods and
//! traits; the example above uses [`FromStr`] to parse a [`str`] into a
//! [`Table`].
//!
//! ## Deserialization and Serialization
//!
//! This crate supports [`serde`] 1.0 with a number of
//! implementations of the `Deserialize`, `Serialize`, `Deserializer`, and
//! `Serializer` traits. Namely, you'll find:
//!
//! * `Deserialize for Table`
//! * `Serialize for Table`
//! * `Deserialize for Value`
//! * `Serialize for Value`
//! * `Deserialize for Datetime`
//! * `Serialize for Datetime`
//! * `Deserializer for de::Deserializer`
//! * `Serializer for ser::Serializer`
//! * `Deserializer for Table`
//! * `Deserializer for Value`
//!
//! This means that you can use Serde to deserialize/serialize the
//! [`Table`] type as well as [`Value`] and [`Datetime`] type in this crate. You can also
//! use the [`Deserializer`], [`Serializer`], or [`Table`] type itself to act as
//! a deserializer/serializer for arbitrary types.
//!
//! An example of deserializing with TOML is:
//!
#![cfg_attr(not(feature = "default"), doc = " ```ignore")]
#![cfg_attr(feature = "default", doc = " ```")]
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Config {
//!     ip: String,
//!     port: Option<u16>,
//!     keys: Keys,
//! }
//!
//! #[derive(Deserialize)]
//! struct Keys {
//!     github: String,
//!     travis: Option<String>,
//! }
//!
//! let config: Config = toml::from_str(r#"
//!     ip = '127.0.0.1'
//!
//!     [keys]
//!     github = 'xxxxxxxxxxxxxxxxx'
//!     travis = 'yyyyyyyyyyyyyyyyy'
//! "#).unwrap();
//!
//! assert_eq!(config.ip, "127.0.0.1");
//! assert_eq!(config.port, None);
//! assert_eq!(config.keys.github, "xxxxxxxxxxxxxxxxx");
//! assert_eq!(config.keys.travis.as_ref().unwrap(), "yyyyyyyyyyyyyyyyy");
//! ```
//!
//! You can serialize types in a similar fashion:
//!
#![cfg_attr(not(feature = "default"), doc = " ```ignore")]
#![cfg_attr(feature = "default", doc = " ```")]
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Config {
//!     ip: String,
//!     port: Option<u16>,
//!     keys: Keys,
//! }
//!
//! #[derive(Serialize)]
//! struct Keys {
//!     github: String,
//!     travis: Option<String>,
//! }
//!
//! let config = Config {
//!     ip: "127.0.0.1".to_string(),
//!     port: None,
//!     keys: Keys {
//!         github: "xxxxxxxxxxxxxxxxx".to_string(),
//!         travis: Some("yyyyyyyyyyyyyyyyy".to_string()),
//!     },
//! };
//!
//! let toml = toml::to_string(&config).unwrap();
//! ```
//!
//! [TOML]: https://github.com/toml-lang/toml
//! [Cargo]: https://crates.io/
//! [`serde`]: https://serde.rs/
//! [serde]: https://serde.rs/

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::std_instead_of_alloc)]
// Makes rustc abort compilation if there are any unsafe blocks in the crate.
// Presence of this annotation is picked up by tools such as cargo-geiger
// and lets them ensure that there is indeed no unsafe code as opposed to
// something they couldn't detect (e.g. unsafe added via macro expansion, etc).
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[allow(unused_extern_crates)]
extern crate alloc;

pub(crate) mod alloc_prelude {
    pub(crate) use alloc::borrow::ToOwned as _;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::String;
    pub(crate) use alloc::string::ToString as _;
    pub(crate) use alloc::vec::Vec;
}

pub mod map;
#[cfg(feature = "serde")]
pub mod value;

pub mod de;
#[cfg(feature = "serde")]
pub mod ser;

#[doc(hidden)]
#[cfg(feature = "serde")]
pub mod macros;

#[cfg(feature = "serde")]
mod table;

#[doc(inline)]
#[cfg(feature = "parse")]
#[cfg(feature = "serde")]
pub use crate::de::{from_slice, from_str, Deserializer};
#[doc(inline)]
#[cfg(feature = "display")]
#[cfg(feature = "serde")]
pub use crate::ser::{to_string, to_string_pretty, Serializer};
#[doc(inline)]
#[cfg(feature = "serde")]
pub use crate::value::Value;
pub use serde_spanned::Spanned;
#[cfg(feature = "serde")]
pub use table::Table;

// Shortcuts for the module doc-comment
#[allow(unused_imports)]
use core::str::FromStr;
#[allow(unused_imports)]
use toml_datetime::Datetime;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
