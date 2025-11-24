// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Traits for data providers that produce opaque buffers.

use crate::prelude::*;

#[cfg(feature = "serde")]
mod serde;
#[cfg(feature = "serde")]
pub use self::serde::*;

/// [`DynamicDataMarker`] for raw buffers. Returned by [`BufferProvider`].
///
/// The data is expected to be deserialized before it can be used; see
/// [`DataPayload::into_deserialized`].
#[non_exhaustive]
#[derive(Debug)]
pub struct BufferMarker;

impl DynamicDataMarker for BufferMarker {
    type DataStruct = &'static [u8];
}

/// A data provider that returns opaque bytes.
///
/// Generally, these bytes are expected to be deserializable with Serde. To get an object
/// implementing [`DataProvider`] via Serde, use [`as_deserializing()`].
///
/// Passing a  `BufferProvider` to a `*_with_buffer_provider` constructor requires enabling
/// the deserialization Cargo feature for the expected format(s):
/// - `deserialize_json`
/// - `deserialize_postcard_1`
/// - `deserialize_bincode_1`
///
/// Along with [`DataProvider`], this is one of the two foundational traits in this crate.
///
/// [`BufferProvider`] can be made into a trait object. It is used over FFI.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "deserialize_json")] {
/// use icu_locale_core::langid;
/// use icu_provider::hello_world::*;
/// use icu_provider::prelude::*;
/// use std::borrow::Cow;
///
/// let buffer_provider = HelloWorldProvider.into_json_provider();
///
/// // Deserializing manually
/// assert_eq!(
///     serde_json::from_slice::<HelloWorld>(
///         buffer_provider
///             .load_data(
///                 HelloWorldV1::INFO,
///                 DataRequest {
///                     id: DataIdentifierBorrowed::for_locale(
///                         &langid!("de").into()
///                     ),
///                     ..Default::default()
///                 }
///             )
///             .expect("load should succeed")
///             .payload
///             .get()
///     )
///     .expect("should deserialize"),
///     HelloWorld {
///         message: Cow::Borrowed("Hallo Welt"),
///     },
/// );
///
/// // Deserialize automatically
/// let deserializing_provider: &dyn DataProvider<HelloWorldV1> =
///     &buffer_provider.as_deserializing();
///
/// assert_eq!(
///     deserializing_provider
///         .load(DataRequest {
///             id: DataIdentifierBorrowed::for_locale(&langid!("de").into()),
///             ..Default::default()
///         })
///         .expect("load should succeed")
///         .payload
///         .get(),
///     &HelloWorld {
///         message: Cow::Borrowed("Hallo Welt"),
///     },
/// );
/// # }
/// ```
///
/// [`as_deserializing()`]: AsDeserializingBufferProvider::as_deserializing
pub trait BufferProvider: DynamicDataProvider<BufferMarker> {}

impl<P: DynamicDataProvider<BufferMarker> + ?Sized> BufferProvider for P {}

/// An enum expressing all Serde formats known to ICU4X.
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[non_exhaustive]
pub enum BufferFormat {
    /// Serialize using JavaScript Object Notation (JSON), using the [`serde_json`] crate.
    Json,
    /// Serialize using the [`bincode`] crate, version 1.
    Bincode1,
    /// Serialize using the [`postcard`] crate, version 1.
    Postcard1,
}

impl BufferFormat {
    /// Returns an error if the buffer format is not enabled.
    pub fn check_available(&self) -> Result<(), DataError> {
        match self {
            #[cfg(feature = "deserialize_json")]
            BufferFormat::Json => Ok(()),
            #[cfg(not(feature = "deserialize_json"))]
            BufferFormat::Json => Err(DataErrorKind::Deserialize.with_str_context("deserializing `BufferFormat::Json` requires the `deserialize_json` Cargo feature")),

            #[cfg(feature = "deserialize_bincode_1")]
            BufferFormat::Bincode1 => Ok(()),
            #[cfg(not(feature = "deserialize_bincode_1"))]
            BufferFormat::Bincode1 => Err(DataErrorKind::Deserialize.with_str_context("deserializing `BufferFormat::Bincode1` requires the `deserialize_bincode_1` Cargo feature")),

            #[cfg(feature = "deserialize_postcard_1")]
            BufferFormat::Postcard1 => Ok(()),
            #[cfg(not(feature = "deserialize_postcard_1"))]
            BufferFormat::Postcard1 => Err(DataErrorKind::Deserialize.with_str_context("deserializing `BufferFormat::Postcard1` requires the `deserialize_postcard_1` Cargo feature")),
        }
    }
}
