//! Serializing Rust structures into TOML.
//!
//! This module contains all the Serde support for serializing Rust structures into TOML.

mod array;
mod error;
mod key;
mod map;
mod pretty;
mod value;

use crate::visit_mut::VisitMut as _;
#[allow(clippy::wildcard_imports)]
use array::*;
#[allow(clippy::wildcard_imports)]
use map::*;

pub use error::Error;
pub use value::ValueSerializer;

/// Serialize the given data structure as a TOML byte vector.
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, if `T` contains a map with non-string keys, or if `T` attempts to
/// serialize an unsupported datatype such as an enum, tuple, or tuple struct.
#[cfg(feature = "display")]
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>, Error>
where
    T: serde::ser::Serialize + ?Sized,
{
    to_string(value).map(|e| e.into_bytes())
}

/// Serialize the given data structure as a String of TOML.
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, if `T` contains a map with non-string keys, or if `T` attempts to
/// serialize an unsupported datatype such as an enum, tuple, or tuple struct.
///
/// # Examples
///
/// ```
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Config {
///     database: Database,
/// }
///
/// #[derive(Serialize)]
/// struct Database {
///     ip: String,
///     port: Vec<u16>,
///     connection_max: u32,
///     enabled: bool,
/// }
///
/// let config = Config {
///     database: Database {
///         ip: "192.168.1.1".to_string(),
///         port: vec![8001, 8002, 8003],
///         connection_max: 5000,
///         enabled: false,
///     },
/// };
///
/// let toml = toml_edit::ser::to_string(&config).unwrap();
/// println!("{}", toml)
/// ```
#[cfg(feature = "display")]
pub fn to_string<T>(value: &T) -> Result<String, Error>
where
    T: serde::ser::Serialize + ?Sized,
{
    to_document(value).map(|e| e.to_string())
}

/// Serialize the given data structure as a "pretty" String of TOML.
///
/// This is identical to `to_string` except the output string has a more
/// "pretty" output. See `ValueSerializer::pretty` for more details.
#[cfg(feature = "display")]
pub fn to_string_pretty<T>(value: &T) -> Result<String, Error>
where
    T: serde::ser::Serialize + ?Sized,
{
    let mut document = to_document(value)?;
    pretty::Pretty::new().visit_document_mut(&mut document);
    Ok(document.to_string())
}

/// Serialize the given data structure into a TOML document.
///
/// This would allow custom formatting to be applied, mixing with format preserving edits, etc.
pub fn to_document<T>(value: &T) -> Result<crate::DocumentMut, Error>
where
    T: serde::ser::Serialize + ?Sized,
{
    let value = value.serialize(ValueSerializer::new())?;
    let item = crate::Item::Value(value);
    let root = item
        .into_table()
        .map_err(|_| Error::UnsupportedType(None))?;
    Ok(root.into())
}
