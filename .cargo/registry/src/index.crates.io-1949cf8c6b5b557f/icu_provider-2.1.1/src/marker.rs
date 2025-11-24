// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::fallback::{LocaleFallbackConfig, LocaleFallbackPriority};
use crate::{DataError, DataErrorKind, DataLocale, DataProvider, DataProviderWithMarker};
use core::fmt;
use core::marker::PhantomData;
use icu_locale_core::preferences::LocalePreferences;
use yoke::Yokeable;
use zerovec::ule::*;

/// Trait marker for data structs. All types delivered by the data provider must be associated with
/// something implementing this trait.
///
/// Data markers normally generated with the [`data_marker`](crate::data_marker) macro.
///
/// Also see [`DataMarker`].
///
/// Note: `DynamicDataMarker`s are quasi-const-generic compile-time objects, and as such are expected
/// to be unit structs. As this is not something that can be enforced by the type system, we
/// currently only have a `'static` bound on them (which is needed by a lot of our code).
///
/// # Examples
///
/// Manually implementing DynamicDataMarker for a custom type:
///
/// ```
/// use icu_provider::prelude::*;
/// use std::borrow::Cow;
///
/// #[derive(yoke::Yokeable, zerofrom::ZeroFrom)]
/// struct MyDataStruct<'data> {
///     message: Cow<'data, str>,
/// }
///
/// struct MyDataStructMarker;
///
/// impl DynamicDataMarker for MyDataStructMarker {
///     type DataStruct = MyDataStruct<'static>;
/// }
///
/// // We can now use MyDataStruct with DataProvider:
/// let s = MyDataStruct {
///     message: Cow::Owned("Hello World".into()),
/// };
/// let payload = DataPayload::<MyDataStructMarker>::from_owned(s);
/// assert_eq!(payload.get().message, "Hello World");
/// ```
///
/// [`data_struct`]: crate::data_struct
pub trait DynamicDataMarker: 'static {
    /// A type that implements [`Yokeable`]. This should typically be the `'static` version of a
    /// data struct.
    type DataStruct: for<'a> Yokeable<'a>;
}

/// A [`DynamicDataMarker`] with a [`DataMarkerInfo`] attached.
///
/// Structs implementing this trait are normally generated with the [`data_struct!`] macro.
///
/// Implementing this trait enables this marker to be used with the main [`DataProvider`] trait.
/// Most markers should be associated with a specific marker and should therefore implement this
/// trait.
///
/// [`BufferMarker`] is an example of a marker that does _not_ implement this trait.
///
/// Note: `DataMarker`s are quasi-const-generic compile-time objects, and as such are expected
/// to be unit structs. As this is not something that can be enforced by the type system, we
/// currently only have a `'static` bound on them (which is needed by a lot of our code).
///
/// [`data_struct!`]: crate::data_struct
/// [`DataProvider`]: crate::DataProvider
/// [`BufferMarker`]: crate::buf::BufferMarker
pub trait DataMarker: DynamicDataMarker {
    /// The single [`DataMarkerInfo`] associated with this marker.
    const INFO: DataMarkerInfo;
}

/// Extension trait for methods on [`DataMarker`]
pub trait DataMarkerExt: DataMarker + Sized {
    /// Binds a [`DataMarker`] to a provider supporting it.
    fn bind<P>(provider: P) -> DataProviderWithMarker<Self, P>
    where
        P: DataProvider<Self>;
    /// Constructs a [`DataLocale`] using fallback preferences from this [`DataMarker`].
    fn make_locale(locale: LocalePreferences) -> DataLocale;
}

impl<M: DataMarker + Sized> DataMarkerExt for M {
    fn bind<P>(provider: P) -> DataProviderWithMarker<Self, P>
    where
        P: DataProvider<Self>,
    {
        DataProviderWithMarker::new(provider)
    }

    fn make_locale(locale: LocalePreferences) -> DataLocale {
        M::INFO.make_locale(locale)
    }
}

/// A [`DynamicDataMarker`] that never returns data.
///
/// All types that have non-blanket impls of `DataProvider<M>` are expected to explicitly
/// implement `DataProvider<NeverMarker<Y>>`, returning [`DataErrorKind::MarkerNotFound`].
/// See [`impl_data_provider_never_marker!`].
///
/// [`DataErrorKind::MarkerNotFound`]: crate::DataErrorKind::MarkerNotFound
/// [`impl_data_provider_never_marker!`]: crate::marker::impl_data_provider_never_marker
///
/// # Examples
///
/// ```
/// use icu_locale_core::langid;
/// use icu_provider::hello_world::*;
/// use icu_provider::marker::NeverMarker;
/// use icu_provider::prelude::*;
///
/// let buffer_provider = HelloWorldProvider.into_json_provider();
///
/// let result = DataProvider::<NeverMarker<HelloWorld<'static>>>::load(
///     &buffer_provider.as_deserializing(),
///     DataRequest {
///         id: DataIdentifierBorrowed::for_locale(&langid!("en").into()),
///         ..Default::default()
///     },
/// );
///
/// assert!(matches!(
///     result,
///     Err(DataError {
///         kind: DataErrorKind::MarkerNotFound,
///         ..
///     })
/// ));
/// ```
#[derive(Debug, Copy, Clone)]
pub struct NeverMarker<Y>(PhantomData<Y>);

impl<Y> DynamicDataMarker for NeverMarker<Y>
where
    for<'a> Y: Yokeable<'a>,
{
    type DataStruct = Y;
}

impl<Y> DataMarker for NeverMarker<Y>
where
    for<'a> Y: Yokeable<'a>,
{
    const INFO: DataMarkerInfo = DataMarkerInfo::from_id(DataMarkerId {
        #[cfg(any(feature = "export", debug_assertions))]
        debug: "NeverMarker",
        hash: *b"nevermar",
    });
}

/// Implements `DataProvider<NeverMarker<Y>>` on a struct.
///
/// For more information, see [`NeverMarker`].
///
/// # Examples
///
/// ```
/// use icu_locale_core::langid;
/// use icu_provider::hello_world::*;
/// use icu_provider::marker::NeverMarker;
/// use icu_provider::prelude::*;
///
/// struct MyProvider;
///
/// icu_provider::marker::impl_data_provider_never_marker!(MyProvider);
///
/// let result = DataProvider::<NeverMarker<HelloWorld<'static>>>::load(
///     &MyProvider,
///     DataRequest {
///         id: DataIdentifierBorrowed::for_locale(&langid!("und").into()),
///         ..Default::default()
///     },
/// );
///
/// assert!(matches!(
///     result,
///     Err(DataError {
///         kind: DataErrorKind::MarkerNotFound,
///         ..
///     })
/// ));
/// ```
#[doc(hidden)] // macro
#[macro_export]
macro_rules! __impl_data_provider_never_marker {
    ($ty:path) => {
        impl<Y> $crate::DataProvider<$crate::marker::NeverMarker<Y>> for $ty
        where
            for<'a> Y: $crate::prelude::yoke::Yokeable<'a>,
        {
            fn load(
                &self,
                req: $crate::DataRequest,
            ) -> Result<$crate::DataResponse<$crate::marker::NeverMarker<Y>>, $crate::DataError>
            {
                Err($crate::DataErrorKind::MarkerNotFound.with_req(
                    <$crate::marker::NeverMarker<Y> as $crate::DataMarker>::INFO,
                    req,
                ))
            }
        }
    };
}
#[doc(inline)]
pub use __impl_data_provider_never_marker as impl_data_provider_never_marker;

/// A compact hash of a [`DataMarkerInfo`]. Useful for keys in maps.
///
/// The hash will be stable over time within major releases.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, ULE)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct DataMarkerIdHash([u8; 4]);

impl DataMarkerIdHash {
    /// Magic bytes to locate [`DataMarkerIdHash`]es in binaries.
    pub const LEADING_TAG: &[u8] = b"tdmh";

    /// Gets the hash value as a byte array.
    pub const fn to_bytes(self) -> [u8; 4] {
        self.0
    }
}

/// Const function to compute the FxHash of a byte array.
///
/// FxHash is a speedy hash algorithm used within rustc. The algorithm is satisfactory for our
/// use case since the strings being hashed originate from a trusted source (the ICU4X
/// components), and the hashes are computed at compile time, so we can check for collisions.
///
/// We could have considered a SHA or other cryptographic hash function. However, we are using
/// FxHash because:
///
/// 1. There is precedent for this algorithm in Rust
/// 2. The algorithm is easy to implement as a const function
/// 3. The amount of code is small enough that we can reasonably keep the algorithm in-tree
/// 4. FxHash is designed to output 32-bit or 64-bit values, whereas SHA outputs more bits,
///    such that truncation would be required in order to fit into a u32, partially reducing
///    the benefit of a cryptographically secure algorithm
// The indexing operations in this function have been reviewed in detail and won't panic.
#[expect(clippy::indexing_slicing)]
const fn fxhash_32(bytes: &[u8]) -> u32 {
    // This code is adapted from https://github.com/rust-lang/rustc-hash,
    // whose license text is reproduced below.
    //
    // Copyright 2015 The Rust Project Developers. See the COPYRIGHT
    // file at the top-level directory of this distribution and at
    // http://rust-lang.org/COPYRIGHT.
    //
    // Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
    // http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
    // <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
    // option. This file may not be copied, modified, or distributed
    // except according to those terms.

    #[inline]
    const fn hash_word_32(mut hash: u32, word: u32) -> u32 {
        const ROTATE: u32 = 5;
        const SEED32: u32 = 0x9e_37_79_b9;
        hash = hash.rotate_left(ROTATE);
        hash ^= word;
        hash = hash.wrapping_mul(SEED32);
        hash
    }

    let mut cursor = 0;
    let end = bytes.len();
    let mut hash = 0;

    while end - cursor >= 4 {
        let word = u32::from_le_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]);
        hash = hash_word_32(hash, word);
        cursor += 4;
    }

    if end - cursor >= 2 {
        let word = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
        hash = hash_word_32(hash, word as u32);
        cursor += 2;
    }

    if end - cursor >= 1 {
        hash = hash_word_32(hash, bytes[cursor] as u32);
    }

    hash
}

#[cfg(feature = "alloc")]
impl<'a> zerovec::maps::ZeroMapKV<'a> for DataMarkerIdHash {
    type Container = zerovec::ZeroVec<'a, DataMarkerIdHash>;
    type Slice = zerovec::ZeroSlice<DataMarkerIdHash>;
    type GetType = <DataMarkerIdHash as AsULE>::ULE;
    type OwnedType = DataMarkerIdHash;
}

impl AsULE for DataMarkerIdHash {
    type ULE = Self;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned
    }
}

// Safe since the ULE type is `self`.
unsafe impl EqULE for DataMarkerIdHash {}

/// The ID of a data marker.
///
/// This is generally a [`DataMarkerIdHash`]. If debug assertions or the `export` Cargo feature
/// are enabled, this also contains a human-readable string for an improved `Debug` implementation.
#[derive(Debug, Copy, Clone, Eq)]
pub struct DataMarkerId {
    #[cfg(any(feature = "export", debug_assertions))]
    debug: &'static str,
    hash: [u8; 8],
}

impl PartialEq for DataMarkerId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Ord for DataMarkerId {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.hash.cmp(&other.hash)
    }
}

impl PartialOrd for DataMarkerId {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::hash::Hash for DataMarkerId {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state)
    }
}

impl DataMarkerId {
    #[doc(hidden)]
    // macro use
    // Error is a str of the expected character class and the index where it wasn't encountered
    // The indexing operations in this function have been reviewed in detail and won't panic.
    pub const fn from_name(name: &'static str) -> Result<Self, (&'static str, usize)> {
        #![allow(clippy::indexing_slicing)]
        if !name.as_bytes()[name.len() - 1].is_ascii_digit() {
            return Err(("[0-9]", name.len()));
        }
        let mut i = name.len() - 1;
        while name.as_bytes()[i - 1].is_ascii_digit() {
            i -= 1;
        }
        if name.as_bytes()[i - 1] != b'V' {
            return Err(("V", i));
        }

        let magic = DataMarkerIdHash::LEADING_TAG;
        let hash = fxhash_32(name.as_bytes()).to_le_bytes();

        Ok(Self {
            #[cfg(any(feature = "export", debug_assertions))]
            debug: name,
            hash: [
                magic[0], magic[1], magic[2], magic[3], hash[0], hash[1], hash[2], hash[3],
            ],
        })
    }

    /// Gets a platform-independent hash of a [`DataMarkerId`].
    ///
    /// The hash is 4 bytes and allows for fast comparison.
    ///
    /// # Example
    ///
    /// ```
    /// use icu_provider::prelude::*;
    ///
    /// icu_provider::data_marker!(FooV1, &'static str);
    ///
    /// assert_eq!(FooV1::INFO.id.hashed().to_bytes(), [198, 217, 86, 48]);
    /// ```
    #[inline]
    pub const fn hashed(self) -> DataMarkerIdHash {
        let [.., h1, h2, h3, h4] = self.hash;
        DataMarkerIdHash([h1, h2, h3, h4])
    }

    /// Returns the marker name.
    ///
    /// For size reasons, this is only available with the `export` Cargo feature.
    #[cfg(feature = "export")]
    pub const fn name(self) -> &'static str {
        self.debug
    }
}

/// Used for loading data from a dynamic ICU4X data provider.
///
/// A data marker is tightly coupled with the code that uses it to load data at runtime.
/// Executables can be searched for `DataMarkerInfo` instances to produce optimized data files.
/// Therefore, users should not generally create DataMarkerInfo instances; they should instead use
/// the ones exported by a component.
#[derive(Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DataMarkerInfo {
    /// The ID of this marker.
    pub id: DataMarkerId,
    /// Whether this data marker only has a single payload, not keyed by a data identifier.
    pub is_singleton: bool,
    /// Whether this data marker uses checksums for integrity purposes.
    pub has_checksum: bool,
    /// The fallback to use for this data marker.
    pub fallback_config: LocaleFallbackConfig,
    /// The attributes domain for this data marker. This can be used for filtering marker
    /// attributes during provider export.
    #[cfg(feature = "export")]
    pub attributes_domain: &'static str,
    /// Whether to create constants for each data struct in baked data.
    #[cfg(feature = "export")]
    pub expose_baked_consts: bool,
}

impl PartialOrd for DataMarkerInfo {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DataMarkerInfo {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl core::hash::Hash for DataMarkerInfo {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl DataMarkerInfo {
    /// See [`Default::default`]
    pub const fn from_id(id: DataMarkerId) -> Self {
        Self {
            id,
            fallback_config: LocaleFallbackConfig::default(),
            is_singleton: false,
            has_checksum: false,
            #[cfg(feature = "export")]
            attributes_domain: "",
            #[cfg(feature = "export")]
            expose_baked_consts: false,
        }
    }

    /// Returns [`Ok`] if this data marker matches the argument, or the appropriate error.
    ///
    /// Convenience method for data providers that support a single [`DataMarkerInfo`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu_provider::hello_world::*;
    /// use icu_provider::prelude::*;
    ///
    /// icu_provider::data_marker!(
    ///     DummyV1,
    ///     <HelloWorldV1 as DynamicDataMarker>::DataStruct
    /// );
    ///
    /// assert!(matches!(
    ///     HelloWorldV1::INFO.match_marker(HelloWorldV1::INFO),
    ///     Ok(())
    /// ));
    /// assert!(matches!(
    ///     HelloWorldV1::INFO.match_marker(DummyV1::INFO),
    ///     Err(DataError {
    ///         kind: DataErrorKind::MarkerNotFound,
    ///         ..
    ///     })
    /// ));
    ///
    /// // The error context contains the argument:
    /// assert_eq!(
    ///     HelloWorldV1::INFO
    ///         .match_marker(DummyV1::INFO)
    ///         .unwrap_err()
    ///         .marker,
    ///     Some(DummyV1::INFO.id)
    /// );
    /// ```
    pub fn match_marker(self, marker: Self) -> Result<(), DataError> {
        if self == marker {
            Ok(())
        } else {
            Err(DataErrorKind::MarkerNotFound.with_marker(marker))
        }
    }

    /// Constructs a [`DataLocale`] for this [`DataMarkerInfo`].
    pub fn make_locale(self, locale: LocalePreferences) -> DataLocale {
        if self.fallback_config.priority == LocaleFallbackPriority::Region {
            locale.to_data_locale_region_priority()
        } else {
            locale.to_data_locale_language_priority()
        }
    }
}

/// Creates a data marker.
///
/// # Examples
///
/// ```
/// icu_provider::data_marker!(DummyV1, &'static str);
/// ```
///
/// The identifier needs to end with a `V` followed by one or more digits (the version number).
///
/// Invalid identifiers are compile-time errors (as [`data_marker!`](crate::data_marker) uses `const`).
///
/// ```compile_fail,E0080
/// icu_provider::data_marker!(Dummy, &'static str);
/// ```
#[macro_export] // canonical location is crate root
macro_rules! data_marker {
    ($(#[$doc:meta])* $name:ident, $($debug:literal,)? $struct:ty $(, $(#[$meta:meta])* $info_field:ident = $info_val:expr)* $(,)?) => {
        $(#[$doc])*
        #[non_exhaustive]
        pub struct $name;
        impl $crate::DynamicDataMarker for $name {
            type DataStruct = $struct;
        }
        impl $crate::DataMarker for $name {
            const INFO: $crate::DataMarkerInfo = {
                $(
                    /// ```rust
                    #[doc = concat!("let ident = \"", stringify!($name), "\";")]
                    #[doc = concat!("let debug = \"", $debug, "\";")]
                    /// assert_eq!(
                    ///     debug.split('/').map(|s| {
                    ///         let mut b = s.to_ascii_lowercase().into_bytes();
                    ///         b[0] = b[0].to_ascii_uppercase();
                    ///         String::from_utf8(b).unwrap()
                    ///     })
                    ///     .collect::<Vec<_>>()
                    ///     .join(""),
                    ///     ident
                    /// );
                    /// ```
                    #[allow(dead_code)]
                    struct DebugTest;
                )?
                #[allow(unused_mut)]
                // Force evaluation even if marker is unused
                let mut info = const { $crate::DataMarkerInfo::from_id(
                     match $crate::marker::DataMarkerId::from_name(stringify!($name)) {
                        Ok(path) => path,
                        Err(_) => panic!(concat!("Invalid marker name: ", stringify!($name))),
                })};
                $(
                    $(#[$meta])*
                    {info.$info_field = $info_val;}
                )*
                info
            };
        }
    }
}

impl fmt::Debug for DataMarkerInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(any(feature = "export", debug_assertions))]
        return f.write_str(self.id.debug);
        #[cfg(not(any(feature = "export", debug_assertions)))]
        return write!(f, "{:?}", self.id);
    }
}

/// A marker for the given `DataStruct`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ErasedMarker<DataStruct: for<'a> Yokeable<'a>>(PhantomData<DataStruct>);
impl<DataStruct: for<'a> Yokeable<'a>> DynamicDataMarker for ErasedMarker<DataStruct> {
    type DataStruct = DataStruct;
}

#[test]
fn test_marker_syntax() {
    // Valid markers:
    DataMarkerId::from_name("HelloWorldV1").unwrap();
    DataMarkerId::from_name("HelloWorldFooV1").unwrap();
    DataMarkerId::from_name("HelloWorldV999").unwrap();
    DataMarkerId::from_name("Hello485FooV1").unwrap();

    // No version:
    assert_eq!(
        DataMarkerId::from_name("HelloWorld"),
        Err(("[0-9]", "HelloWorld".len()))
    );

    assert_eq!(
        DataMarkerId::from_name("HelloWorldV"),
        Err(("[0-9]", "HelloWorldV".len()))
    );
    assert_eq!(
        DataMarkerId::from_name("HelloWorldVFoo"),
        Err(("[0-9]", "HelloWorldVFoo".len()))
    );
    assert_eq!(
        DataMarkerId::from_name("HelloWorldV1Foo"),
        Err(("[0-9]", "HelloWorldV1Foo".len()))
    );
}

#[test]
fn test_id_debug() {
    assert_eq!(DataMarkerId::from_name("BarV1").unwrap().debug, "BarV1");
}

#[test]
fn test_hash_word_32() {
    assert_eq!(0, fxhash_32(b""));
    assert_eq!(0xF3051F19, fxhash_32(b"a"));
    assert_eq!(0x2F9DF119, fxhash_32(b"ab"));
    assert_eq!(0xCB1D9396, fxhash_32(b"abc"));
    assert_eq!(0x8628F119, fxhash_32(b"abcd"));
    assert_eq!(0xBEBDB56D, fxhash_32(b"abcde"));
    assert_eq!(0x1CE8476D, fxhash_32(b"abcdef"));
    assert_eq!(0xC0F176A4, fxhash_32(b"abcdefg"));
    assert_eq!(0x09AB476D, fxhash_32(b"abcdefgh"));
    assert_eq!(0xB72F5D88, fxhash_32(b"abcdefghi"));
}

#[test]
fn test_id_hash() {
    assert_eq!(
        DataMarkerId::from_name("BarV1").unwrap().hashed(),
        DataMarkerIdHash([212, 77, 158, 241]),
    );
}
