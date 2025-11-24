//! Serializing Rust structures into TOML.
//!
//! This module contains all the Serde support for serializing Rust structures
//! into TOML documents (as strings). Note that some top-level functions here
//! are also provided at the top of the crate.

#[cfg(feature = "display")]
mod document;
mod error;
#[cfg(feature = "display")]
mod style;
#[cfg(feature = "display")]
mod value;

use crate::alloc_prelude::*;

#[cfg(feature = "display")]
pub use document::Buffer;
#[cfg(feature = "display")]
pub use document::Serializer;
pub use error::Error;
pub(crate) use error::ErrorInner;
#[cfg(feature = "display")]
pub use value::ValueSerializer;

/// Serialize the given data structure as a String of TOML.
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, if `T` contains a map with non-string keys, or if `T` attempts to
/// serialize an unsupported datatype such as an enum, tuple, or tuple struct.
///
/// To serialize TOML values, instead of documents, see [`ValueSerializer`].
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
/// let toml = toml::to_string(&config).unwrap();
/// println!("{}", toml)
/// ```
#[cfg(feature = "display")]
pub fn to_string<T>(value: &T) -> Result<String, Error>
where
    T: serde::ser::Serialize + ?Sized,
{
    let mut output = Buffer::new();
    let serializer = Serializer::new(&mut output);
    value.serialize(serializer)?;
    Ok(output.to_string())
}

/// Serialize the given data structure as a "pretty" String of TOML.
///
/// This is identical to `to_string` except the output string has a more
/// "pretty" output. See `Serializer::pretty` for more details.
///
/// To serialize TOML values, instead of documents, see [`ValueSerializer`].
///
/// For greater customization, instead serialize to a
/// [`toml_edit::DocumentMut`](https://docs.rs/toml_edit/latest/toml_edit/struct.DocumentMut.html).
#[cfg(feature = "display")]
pub fn to_string_pretty<T>(value: &T) -> Result<String, Error>
where
    T: serde::ser::Serialize + ?Sized,
{
    let mut output = Buffer::new();
    let serializer = Serializer::pretty(&mut output);
    value.serialize(serializer)?;
    Ok(output.to_string())
}
