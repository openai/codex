// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! `icu_provider` is one of the `ICU4X` components.
//!
//! Unicode's experience with ICU4X's parent projects, ICU4C and ICU4J, led the team to realize
//! that data management is the most critical aspect of deploying internationalization, and that it requires
//! a high level of customization for the needs of the platform it is embedded in. As a result
//! ICU4X comes with a selection of providers that should allow for ICU4X to naturally fit into
//! different business and technological needs of customers.
//!
//! `icu_provider` defines traits and structs for transmitting data through the ICU4X locale
//! data pipeline. The primary trait is [`DataProvider`]. It is parameterized by a
//! [`DataMarker`], which is the type-system-level data identifier. [`DataProvider`] has a single method,
//! [`DataProvider::load`], which transforms a [`DataRequest`] into a [`DataResponse`].
//!
//! - [`DataRequest`] contains selectors to choose a specific variant of the marker, such as a locale.
//! - [`DataResponse`] contains the data if the request was successful.
//!
//! The most common types required for this crate are included via the prelude:
//!
//! ```
//! use icu_provider::prelude::*;
//! ```
//!
//! ## Dynamic Data Providers
//!
//! If the type system cannot be leveraged to load data (such as when dynamically loading from I/O),
//! there's another form of the [`DataProvider`]: [`DynamicDataProvider`]. While [`DataProvider`] is parametrized
//! on the type-system level by a [`DataMarker`] (which are distinct types implementing this trait),
//! [`DynamicDataProvider`]s are parametrized at runtime by a [`DataMarkerInfo`] struct, which essentially is the runtime
//! representation of the [`DataMarker`] type.
//!
//! The [`DynamicDataProvider`] is still type-level parametrized by the type that it loads, and there are two
//! implementations that should be called out
//!
//! - [`DynamicDataProvider<BufferMarker>`], a.k.a. [`BufferProvider`](buf::BufferProvider) returns data as `[u8]` buffers.
//!
//! ### BufferProvider
//!
//! These providers are able to return unstructured data typically represented as
//! [`serde`]-serialized buffers. Users can call [`as_deserializing()`] to get an object
//! implementing [`DataProvider`] by invoking Serde Deserialize.
//!
//! Examples of BufferProviders:
//!
//! - [`FsDataProvider`] reads individual buffers from the filesystem.
//! - [`BlobDataProvider`] reads buffers from a large in-memory blob.
//!
//! ## Provider Adapters
//!
//! ICU4X offers several built-in modules to combine providers in interesting ways.
//! These can be found in the [`icu_provider_adapters`] crate.
//!
//! ## Testing Provider
//!
//! This crate also contains a concrete provider for demonstration purposes:
//!
//! - [`HelloWorldProvider`] returns "hello world" strings in several languages.
//!
//! ## Types and Lifetimes
//!
//! Types compatible with [`Yokeable`] can be passed through the data provider, so long as they are
//! associated with a marker type implementing [`DynamicDataMarker`].
//!
//! Data structs should generally have one lifetime argument: `'data`. This lifetime allows data
//! structs to borrow zero-copy data.
//!
//! [`FixedProvider`]: https://docs.rs/icu_provider_adapters/latest/fixed/any_payload/struct.FixedProvider.html
//! [`HelloWorldProvider`]: hello_world::HelloWorldProvider
//! [`Yokeable`]: yoke::Yokeable
//! [`impl_dynamic_data_provider!`]: dynutil::impl_dynamic_data_provider
//! [`icu_provider_adapters`]: https://docs.rs/icu_provider_adapters/latest/icu_provider_adapters/index.html
//! [`SourceDataProvider`]: https://docs.rs/icu_provider_source/latest/icu_provider_source/struct.SourceDataProvider.html
//! [`as_deserializing()`]: buf::AsDeserializingBufferProvider::as_deserializing
//! [`FsDataProvider`]: https://docs.rs/icu_provider_fs/latest/icu_provider_fs/struct.FsDataProvider.html
//! [`BlobDataProvider`]: https://docs.rs/icu_provider_blob/latest/icu_provider_blob/struct.BlobDataProvider.html

// https://github.com/unicode-org/icu4x/blob/main/documents/process/boilerplate.md#library-annotations
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::exhaustive_structs,
        clippy::exhaustive_enums,
        clippy::trivially_copy_pass_by_ref,
        missing_debug_implementations,
    )
)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "baked")]
pub mod baked;
pub mod buf;
pub mod constructors;
pub mod dynutil;
#[cfg(feature = "export")]
pub mod export;
#[cfg(feature = "alloc")]
pub mod hello_world;

// TODO: put this in a separate crate
#[cfg(all(feature = "serde", feature = "alloc"))]
#[doc(hidden)]
pub mod serde_borrow_de_utils;

mod data_provider;
pub use data_provider::{
    BoundDataProvider, DataProvider, DataProviderWithMarker, DryDataProvider, DynamicDataProvider,
    DynamicDryDataProvider,
};
#[cfg(feature = "alloc")]
pub use data_provider::{IterableDataProvider, IterableDynamicDataProvider};

mod error;
pub use error::{DataError, DataErrorKind, ResultDataError};

mod request;
pub use request::{DataLocale, DataMarkerAttributes, DataRequest, DataRequestMetadata, *};

mod response;
#[doc(hidden)] // TODO(#4467): establish this as an internal API
pub use response::DataPayloadOr;
pub use response::{Cart, DataPayload, DataResponse, DataResponseMetadata};

#[path = "marker.rs"]
mod marker_full;

pub use marker_full::{DataMarker, DataMarkerInfo, DynamicDataMarker};
pub mod marker {
    //! Additional [`DataMarker`](super::DataMarker) helpers.

    #[doc(inline)]
    pub use super::marker_full::impl_data_provider_never_marker;
    pub use super::marker_full::{
        DataMarkerExt, DataMarkerId, DataMarkerIdHash, ErasedMarker, NeverMarker,
    };
}

mod varule_traits;
pub mod ule {
    //! Traits that data provider implementations can use to optimize storage
    //! by using [`VarULE`](zerovec::ule::VarULE).
    //!
    //! See [`MaybeAsVarULE`] for details.

    pub use super::varule_traits::MaybeAsVarULE;
    #[cfg(feature = "export")]
    pub use super::varule_traits::MaybeEncodeAsVarULE;
}

/// Core selection of APIs and structures for the ICU4X data provider.
pub mod prelude {
    #[doc(no_inline)]
    #[cfg(feature = "serde")]
    pub use crate::buf::AsDeserializingBufferProvider;
    #[doc(no_inline)]
    pub use crate::buf::{BufferMarker, BufferProvider};
    #[doc(no_inline)]
    pub use crate::{
        data_marker, data_struct, marker::DataMarkerExt, request::AttributeParseError,
        request::DataIdentifierBorrowed, BoundDataProvider, DataError, DataErrorKind, DataLocale,
        DataMarker, DataMarkerAttributes, DataMarkerInfo, DataPayload, DataProvider, DataRequest,
        DataRequestMetadata, DataResponse, DataResponseMetadata, DryDataProvider,
        DynamicDataMarker, DynamicDataProvider, DynamicDryDataProvider, ResultDataError,
    };
    #[cfg(feature = "alloc")]
    #[doc(no_inline)]
    pub use crate::{
        request::DataIdentifierCow, IterableDataProvider, IterableDynamicDataProvider,
    };

    #[doc(no_inline)]
    pub use icu_locale_core;
    #[doc(no_inline)]
    pub use yoke;
    #[doc(no_inline)]
    pub use zerofrom;
}

#[doc(hidden)] // internal
pub mod fallback;

#[doc(hidden)] // internal
#[cfg(feature = "logging")]
pub use log;

#[doc(hidden)] // internal
#[cfg(all(
    not(feature = "logging"),
    all(debug_assertions, feature = "alloc", not(target_os = "none"))
))]
pub mod log {
    extern crate std;
    pub use std::eprintln as error;
    pub use std::eprintln as warn;
    pub use std::eprintln as info;
    pub use std::eprintln as debug;
    pub use std::eprintln as trace;
}

#[cfg(all(
    not(feature = "logging"),
    not(all(debug_assertions, feature = "alloc", not(target_os = "none"),))
))]
#[doc(hidden)] // internal
pub mod log {
    #[macro_export]
    macro_rules! _internal_noop_log {
        ($($t:expr),*) => {};
    }
    pub use crate::_internal_noop_log as error;
    pub use crate::_internal_noop_log as warn;
    pub use crate::_internal_noop_log as info;
    pub use crate::_internal_noop_log as debug;
    pub use crate::_internal_noop_log as trace;
}

#[test]
fn test_logging() {
    // This should compile on all combinations of features
    crate::log::info!("Hello World");
}
