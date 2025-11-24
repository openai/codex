//! Deserializing TOML into Rust structures.
//!
//! This module contains all the Serde support for deserializing TOML documents
//! into Rust structures. Note that some top-level functions here are also
//! provided at the top of the crate.

#[cfg(feature = "parse")]
#[cfg(feature = "serde")]
mod deserializer;
mod error;
#[cfg(feature = "parse")]
mod parser;

#[cfg(feature = "parse")]
#[cfg(feature = "serde")]
pub use deserializer::Deserializer;
#[cfg(feature = "parse")]
#[cfg(feature = "serde")]
pub use deserializer::ValueDeserializer;
#[cfg(feature = "parse")]
pub use parser::DeArray;
#[cfg(feature = "parse")]
pub use parser::DeFloat;
#[cfg(feature = "parse")]
pub use parser::DeInteger;
#[cfg(feature = "parse")]
pub use parser::DeString;
#[cfg(feature = "parse")]
pub use parser::DeTable;
#[cfg(feature = "parse")]
pub use parser::DeValue;

pub use error::Error;

use crate::alloc_prelude::*;

/// Deserializes a string into a type.
///
/// This function will attempt to interpret `s` as a TOML document and
/// deserialize `T` from the document.
///
/// To deserializes TOML values, instead of documents, see [`ValueDeserializer`].
///
/// # Examples
///
/// ```
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Config {
///     title: String,
///     owner: Owner,
/// }
///
/// #[derive(Deserialize)]
/// struct Owner {
///     name: String,
/// }
///
/// let config: Config = toml::from_str(r#"
///     title = 'TOML Example'
///
///     [owner]
///     name = 'Lisa'
/// "#).unwrap();
///
/// assert_eq!(config.title, "TOML Example");
/// assert_eq!(config.owner.name, "Lisa");
/// ```
#[cfg(feature = "parse")]
#[cfg(feature = "serde")]
pub fn from_str<'de, T>(s: &'de str) -> Result<T, Error>
where
    T: serde::de::Deserialize<'de>,
{
    T::deserialize(Deserializer::parse(s)?)
}

/// Deserializes bytes into a type.
///
/// This function will attempt to interpret `s` as a TOML document and
/// deserialize `T` from the document.
///
/// To deserializes TOML values, instead of documents, see [`ValueDeserializer`].
#[cfg(feature = "parse")]
#[cfg(feature = "serde")]
pub fn from_slice<'de, T>(s: &'de [u8]) -> Result<T, Error>
where
    T: serde::de::Deserialize<'de>,
{
    let s = core::str::from_utf8(s).map_err(|e| Error::custom(e.to_string(), None))?;
    from_str(s)
}
