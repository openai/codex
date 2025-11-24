// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Zero-copy vector abstractions for arbitrary types, backed by byte slices.
//!
//! `zerovec` enables a far wider range of types — beyond just `&[u8]` and `&str` — to participate in
//! zero-copy deserialization from byte slices. It is `serde` compatible and comes equipped with
//! proc macros
//!
//! Clients upgrading to `zerovec` benefit from zero heap allocations when deserializing
//! read-only data.
//!
//! This crate has four main types:
//!
//! - [`ZeroVec<'a, T>`] (and [`ZeroSlice<T>`](ZeroSlice)) for fixed-width types like `u32`
//! - [`VarZeroVec<'a, T>`] (and [`VarZeroSlice<T>`](ZeroSlice)) for variable-width types like `str`
//! - [`ZeroMap<'a, K, V>`] to map from `K` to `V`
//! - [`ZeroMap2d<'a, K0, K1, V>`] to map from the pair `(K0, K1)` to `V`
//!
//! The first two are intended as close-to-drop-in replacements for `Vec<T>` in Serde structs. The third and fourth are
//! intended as a replacement for `HashMap` or [`LiteMap`](https://docs.rs/litemap). When used with Serde derives, **be sure to apply
//! `#[serde(borrow)]` to these types**, same as one would for [`Cow<'a, T>`].
//!
//! [`ZeroVec<'a, T>`], [`VarZeroVec<'a, T>`], [`ZeroMap<'a, K, V>`], and [`ZeroMap2d<'a, K0, K1, V>`] all behave like
//! [`Cow<'a, T>`] in that they abstract over either borrowed or owned data. When performing deserialization
//! from human-readable formats (like `json` and `xml`), typically these types will allocate and fully own their data, whereas if deserializing
//! from binary formats like `bincode` and `postcard`, these types will borrow data directly from the buffer being deserialized from,
//! avoiding allocations and only performing validity checks. As such, this crate can be pretty fast (see [below](#Performance) for more information)
//! on deserialization.
//!
//! See [the design doc](https://github.com/unicode-org/icu4x/blob/main/utils/zerovec/design_doc.md) for details on how this crate
//! works under the hood.
//!
//! # Cargo features
//!
//! This crate has several optional Cargo features:
//!  - `serde`: Allows serializing and deserializing `zerovec`'s abstractions via [`serde`](https://docs.rs/serde)
//!  - `yoke`: Enables implementations of `Yokeable` from the [`yoke`](https://docs.rs/yoke/) crate, which is also useful
//!    in situations involving a lot of zero-copy deserialization.
//!  - `derive`: Makes it easier to use custom types in these collections by providing the [`#[make_ule]`](crate::make_ule) and
//!    [`#[make_varule]`](crate::make_varule) proc macros, which generate appropriate [`ULE`](crate::ule::ULE) and
//!    [`VarULE`](crate::ule::VarULE)-conformant types for a given "normal" type.
//!  - `std`: Enabled `std::Error` implementations for error types. This crate is by default `no_std` with a dependency on `alloc`.
//!
//! [`ZeroVec<'a, T>`]: ZeroVec
//! [`VarZeroVec<'a, T>`]: VarZeroVec
//! [`ZeroMap<'a, K, V>`]: ZeroMap
//! [`ZeroMap2d<'a, K0, K1, V>`]: ZeroMap2d
//! [`Cow<'a, T>`]: alloc::borrow::Cow
//!
//! # Examples
//!
//! Serialize and deserialize a struct with ZeroVec and VarZeroVec with Bincode:
//!
//! ```
//! # #[cfg(feature = "serde")] {
//! use zerovec::{VarZeroVec, ZeroVec};
//!
//! // This example requires the "serde" feature
//! #[derive(serde::Serialize, serde::Deserialize)]
//! pub struct DataStruct<'data> {
//!     #[serde(borrow)]
//!     nums: ZeroVec<'data, u32>,
//!     #[serde(borrow)]
//!     chars: ZeroVec<'data, char>,
//!     #[serde(borrow)]
//!     strs: VarZeroVec<'data, str>,
//! }
//!
//! let data = DataStruct {
//!     nums: ZeroVec::from_slice_or_alloc(&[211, 281, 421, 461]),
//!     chars: ZeroVec::alloc_from_slice(&['ö', '冇', 'म']),
//!     strs: VarZeroVec::from(&["hello", "world"]),
//! };
//! let bincode_bytes =
//!     bincode::serialize(&data).expect("Serialization should be successful");
//! assert_eq!(bincode_bytes.len(), 63);
//!
//! let deserialized: DataStruct = bincode::deserialize(&bincode_bytes)
//!     .expect("Deserialization should be successful");
//! assert_eq!(deserialized.nums.first(), Some(211));
//! assert_eq!(deserialized.chars.get(1), Some('冇'));
//! assert_eq!(deserialized.strs.get(1), Some("world"));
//! // The deserialization will not have allocated anything
//! assert!(!deserialized.nums.is_owned());
//! # } // feature = "serde"
//! ```
//!
//! Use custom types inside of ZeroVec:
//!
//! ```rust
//! # #[cfg(all(feature = "serde", feature = "derive"))] {
//! use zerovec::{ZeroVec, VarZeroVec, ZeroMap};
//! use std::borrow::Cow;
//! use zerovec::ule::encode_varule_to_box;
//!
//! // custom fixed-size ULE type for ZeroVec
//! #[zerovec::make_ule(DateULE)]
//! #[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
//! struct Date {
//!     y: u64,
//!     m: u8,
//!     d: u8
//! }
//!
//! // custom variable sized VarULE type for VarZeroVec
//! #[zerovec::make_varule(PersonULE)]
//! #[zerovec::derive(Serialize, Deserialize)] // add Serde impls to PersonULE
//! #[derive(Clone, PartialEq, Eq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
//! struct Person<'a> {
//!     birthday: Date,
//!     favorite_character: char,
//!     #[serde(borrow)]
//!     name: Cow<'a, str>,
//! }
//!
//! #[derive(serde::Serialize, serde::Deserialize)]
//! struct Data<'a> {
//!     #[serde(borrow)]
//!     important_dates: ZeroVec<'a, Date>,
//!     // note: VarZeroVec always must reference the ULE type directly
//!     #[serde(borrow)]
//!     important_people: VarZeroVec<'a, PersonULE>,
//!     #[serde(borrow)]
//!     birthdays_to_people: ZeroMap<'a, Date, PersonULE>
//! }
//!
//!
//! let person1 = Person {
//!     birthday: Date { y: 1990, m: 9, d: 7},
//!     favorite_character: 'π',
//!     name: Cow::from("Kate")
//! };
//! let person2 = Person {
//!     birthday: Date { y: 1960, m: 5, d: 25},
//!     favorite_character: '冇',
//!     name: Cow::from("Jesse")
//! };
//!
//! let important_dates = ZeroVec::alloc_from_slice(&[Date { y: 1943, m: 3, d: 20}, Date { y: 1976, m: 8, d: 2}, Date { y: 1998, m: 2, d: 15}]);
//! let important_people = VarZeroVec::from(&[&person1, &person2]);
//! let mut birthdays_to_people: ZeroMap<Date, PersonULE> = ZeroMap::new();
//! // `.insert_var_v()` is slightly more convenient over `.insert()` for custom ULE types
//! birthdays_to_people.insert_var_v(&person1.birthday, &person1);
//! birthdays_to_people.insert_var_v(&person2.birthday, &person2);
//!
//! let data = Data { important_dates, important_people, birthdays_to_people };
//!
//! let bincode_bytes = bincode::serialize(&data)
//!     .expect("Serialization should be successful");
//! assert_eq!(bincode_bytes.len(), 160);
//!
//! let deserialized: Data = bincode::deserialize(&bincode_bytes)
//!     .expect("Deserialization should be successful");
//!
//! assert_eq!(deserialized.important_dates.get(0).unwrap().y, 1943);
//! assert_eq!(&deserialized.important_people.get(1).unwrap().name, "Jesse");
//! assert_eq!(&deserialized.important_people.get(0).unwrap().name, "Kate");
//! assert_eq!(&deserialized.birthdays_to_people.get(&person1.birthday).unwrap().name, "Kate");
//!
//! } // feature = serde and derive
//! ```
//!
//! # Performance
//!
//! `zerovec` is designed for fast deserialization from byte buffers with zero memory allocations
//! while minimizing performance regressions for common vector operations.
//!
//! Benchmark results on x86_64:
//!
//! | Operation | `Vec<T>` | `zerovec` |
//! |---|---|---|
//! | Deserialize vec of 100 `u32` | 233.18 ns | 14.120 ns |
//! | Compute sum of vec of 100 `u32` (read every element) | 8.7472 ns | 10.775 ns |
//! | Binary search vec of 1000 `u32` 50 times | 442.80 ns | 472.51 ns |
//! | Deserialize vec of 100 strings | 7.3740 μs\* | 1.4495 μs |
//! | Count chars in vec of 100 strings (read every element) | 747.50 ns | 955.28 ns |
//! | Binary search vec of 500 strings 10 times | 466.09 ns | 790.33 ns |
//!
//! \* *This result is reported for `Vec<String>`. However, Serde also supports deserializing to the partially-zero-copy `Vec<&str>`; this gives 1.8420 μs, much faster than `Vec<String>` but a bit slower than `zerovec`.*
//!
//! | Operation | `HashMap<K,V>`  | `LiteMap<K,V>` | `ZeroMap<K,V>` |
//! |---|---|---|---|
//! | Deserialize a small map | 2.72 μs | 1.28 μs | 480 ns |
//! | Deserialize a large map | 50.5 ms | 18.3 ms | 3.74 ms |
//! | Look up from a small deserialized map | 49 ns | 42 ns | 54 ns |
//! | Look up from a large deserialized map | 51 ns | 155 ns | 213 ns |
//!
//! Small = 16 elements, large = 131,072 elements. Maps contain `<String, String>`.
//!
//! The benches used to generate the above table can be found in the `benches` directory in the project repository.
//! `zeromap` benches are named by convention, e.g. `zeromap/deserialize/small`, `zeromap/lookup/large`. The type
//! is appended for baseline comparisons, e.g. `zeromap/lookup/small/hashmap`.

// https://github.com/unicode-org/icu4x/blob/main/documents/process/boilerplate.md#library-annotations
#![cfg_attr(not(any(test, doc)), no_std)]
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
// this crate does a lot of nuanced lifetime manipulation, being explicit
// is better here.
#![allow(clippy::needless_lifetimes)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod cow;
#[cfg(feature = "hashmap")]
pub mod hashmap;
#[cfg(feature = "alloc")]
mod map;
#[cfg(feature = "alloc")]
mod map2d;
#[cfg(test)]
pub mod samples;
mod varzerovec;
mod zerovec;

// This must be after `mod zerovec` for some impls on `ZeroSlice<RawBytesULE>`
// to show up in the right spot in the docs
pub mod ule;
#[cfg(feature = "yoke")]
mod yoke_impls;
mod zerofrom_impls;

pub use crate::cow::VarZeroCow;
#[cfg(feature = "hashmap")]
pub use crate::hashmap::ZeroHashMap;
#[cfg(feature = "alloc")]
pub use crate::map::map::ZeroMap;
#[cfg(feature = "alloc")]
pub use crate::map2d::map::ZeroMap2d;
pub use crate::varzerovec::{slice::VarZeroSlice, vec::VarZeroVec};
pub use crate::zerovec::{ZeroSlice, ZeroVec};

#[doc(hidden)] // macro use
pub mod __zerovec_internal_reexport {
    pub use zerofrom::ZeroFrom;

    #[cfg(feature = "alloc")]
    pub use alloc::borrow;
    #[cfg(feature = "alloc")]
    pub use alloc::boxed;

    #[cfg(feature = "serde")]
    pub use serde;
}

#[cfg(feature = "alloc")]
pub mod maps {
    //! This module contains additional utility types and traits for working with
    //! [`ZeroMap`] and [`ZeroMap2d`]. See their docs for more details on the general purpose
    //! of these types.
    //!
    //! [`ZeroMapBorrowed`] and [`ZeroMap2dBorrowed`] are versions of [`ZeroMap`] and [`ZeroMap2d`]
    //! that can be used when you wish to guarantee that the map data is always borrowed, leading to
    //! relaxed lifetime constraints.
    //!
    //! The [`ZeroMapKV`] trait is required to be implemented on any type that needs to be used
    //! within a map type. [`ZeroVecLike`] and [`MutableZeroVecLike`] are traits used in the
    //! internal workings of the map types, and should typically not be used or implemented by
    //! users of this crate.
    #[doc(no_inline)]
    pub use crate::map::ZeroMap;
    pub use crate::map::ZeroMapBorrowed;

    #[doc(no_inline)]
    pub use crate::map2d::ZeroMap2d;
    pub use crate::map2d::ZeroMap2dBorrowed;

    pub use crate::map::{MutableZeroVecLike, ZeroMapKV, ZeroVecLike};

    pub use crate::map2d::ZeroMap2dCursor;
}

pub mod vecs {
    //! This module contains additional utility types for working with
    //! [`ZeroVec`] and  [`VarZeroVec`]. See their docs for more details on the general purpose
    //! of these types.
    //!
    //! [`ZeroSlice`] and [`VarZeroSlice`] provide slice-like versions of the vector types
    //! for use behind references and in custom ULE types.
    //!
    //! [`VarZeroVecOwned`] is a special owned/mutable version of [`VarZeroVec`], allowing
    //! direct manipulation of the backing buffer.

    #[doc(no_inline)]
    pub use crate::zerovec::{ZeroSlice, ZeroVec};

    pub use crate::zerovec::ZeroSliceIter;

    #[doc(no_inline)]
    pub use crate::varzerovec::{VarZeroSlice, VarZeroVec};

    #[cfg(feature = "alloc")]
    pub use crate::varzerovec::VarZeroVecOwned;
    pub use crate::varzerovec::{Index16, Index32, Index8, VarZeroSliceIter, VarZeroVecFormat};

    pub type VarZeroVec16<'a, T> = VarZeroVec<'a, T, Index16>;
    pub type VarZeroVec32<'a, T> = VarZeroVec<'a, T, Index32>;
    pub type VarZeroSlice16<T> = VarZeroSlice<T, Index16>;
    pub type VarZeroSlice32<T> = VarZeroSlice<T, Index32>;
}

// Proc macro reexports
//
// These exist so that our docs can use intra-doc links.
// Due to quirks of how rustdoc does documentation on reexports, these must be in this module and not reexported from
// a submodule

/// Generate a corresponding [`ULE`] type and the relevant [`AsULE`] implementations for this type
///
/// This can be attached to structs containing only [`AsULE`] types, or C-like enums that have `#[repr(u8)]`
/// and all explicit discriminants.
///
/// The type must be [`Copy`], [`PartialEq`], and [`Eq`].
///
/// `#[make_ule]` will automatically derive the following traits on the [`ULE`] type:
///
/// - [`Ord`] and [`PartialOrd`]
/// - [`ZeroMapKV`]
///
/// To disable one of the automatic derives, use `#[zerovec::skip_derive(...)]` like so: `#[zerovec::skip_derive(ZeroMapKV)]`.
/// `Ord` and `PartialOrd` are implemented as a unit and can only be disabled as a group with `#[zerovec::skip_derive(Ord)]`.
///
/// The following traits are available to derive, but not automatic:
///
/// - [`Debug`]
///
/// To enable one of these additional derives, use `#[zerovec::derive(...)]` like so: `#[zerovec::derive(Debug)]`.
///
/// In most cases these derives will defer to the impl of the same trait on the current type, so such impls must exist.
///
/// For enums, this attribute will generate a crate-public `fn new_from_u8(value: u8) -> Option<Self>`
/// method on the main type that allows one to construct the value from a u8. If this method is desired
/// to be more public, it should be wrapped.
///
/// [`ULE`]: ule::ULE
/// [`AsULE`]: ule::AsULE
/// [`ZeroMapKV`]: maps::ZeroMapKV
///
/// # Example
///
/// ```rust
/// use zerovec::ZeroVec;
///
/// #[zerovec::make_ule(DateULE)]
/// #[derive(
///     Copy,
///     Clone,
///     PartialEq,
///     Eq,
///     Ord,
///     PartialOrd,
///     serde::Serialize,
///     serde::Deserialize,
/// )]
/// struct Date {
///     y: u64,
///     m: u8,
///     d: u8,
/// }
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Dates<'a> {
///     #[serde(borrow)]
///     dates: ZeroVec<'a, Date>,
/// }
///
/// let dates = Dates {
///     dates: ZeroVec::alloc_from_slice(&[
///         Date {
///             y: 1985,
///             m: 9,
///             d: 3,
///         },
///         Date {
///             y: 1970,
///             m: 2,
///             d: 20,
///         },
///         Date {
///             y: 1990,
///             m: 6,
///             d: 13,
///         },
///     ]),
/// };
///
/// let bincode_bytes =
///     bincode::serialize(&dates).expect("Serialization should be successful");
///
/// // Will deserialize without allocations
/// let deserialized: Dates = bincode::deserialize(&bincode_bytes)
///     .expect("Deserialization should be successful");
///
/// assert_eq!(deserialized.dates.get(1).unwrap().y, 1970);
/// assert_eq!(deserialized.dates.get(2).unwrap().d, 13);
/// ```
#[cfg(feature = "derive")]
pub use zerovec_derive::make_ule;

/// Generate a corresponding [`VarULE`] type and the relevant [`EncodeAsVarULE`]/[`zerofrom::ZeroFrom`]
/// implementations for this type
///
/// This can be attached to structs containing only [`AsULE`] types with the last fields being
/// [`Cow<'a, str>`](alloc::borrow::Cow), [`ZeroSlice`], or [`VarZeroSlice`]. If there is more than one such field, it will be represented
/// using [`MultiFieldsULE`](crate::ule::MultiFieldsULE) and getters will be generated. Other VarULE fields will be detected if they are
/// tagged with `#[zerovec::varule(NameOfVarULETy)]`.
///
/// The type must be [`PartialEq`] and [`Eq`].
///
/// [`EncodeAsVarULE`] and [`zerofrom::ZeroFrom`] are useful for avoiding the need to deal with
/// the [`VarULE`] type directly. In particular, it is recommended to use [`zerofrom::ZeroFrom`]
/// to convert the [`VarULE`] type back to this type in a cheap, zero-copy way (see the example below
/// for more details).
///
/// `#[make_varule]` will automatically derive the following traits on the [`VarULE`] type:
///
/// - [`Ord`] and [`PartialOrd`]
/// - [`ZeroMapKV`]
/// - [`alloc::borrow::ToOwned`]
///
/// To disable one of the automatic derives, use `#[zerovec::skip_derive(...)]` like so: `#[zerovec::skip_derive(ZeroMapKV)]`.
/// `Ord` and `PartialOrd` are implemented as a unit and can only be disabled as a group with `#[zerovec::skip_derive(Ord)]`.
///
/// The following traits are available to derive, but not automatic:
///
/// - [`Debug`]
/// - [`Serialize`](serde::Serialize)
/// - [`Deserialize`](serde::Deserialize)
///
/// To enable one of these additional derives, use `#[zerovec::derive(...)]` like so: `#[zerovec::derive(Debug)]`.
///
/// In most cases these derives will defer to the impl of the same trait on the current type, so such impls must exist.
///
/// This implementation will also by default autogenerate [`Ord`] and [`PartialOrd`] on the [`VarULE`] type based on
/// the implementation on `Self`. You can opt out of this with `#[zerovec::skip_derive(Ord)]`
///
/// Note that this implementation will autogenerate [`EncodeAsVarULE`] impls for _both_ `Self` and `&Self`
/// for convenience. This allows for a little more flexibility encoding slices.
///
/// In case there are multiple [`VarULE`] (i.e., variable-sized) fields, this macro will produce private fields that
/// appropriately pack the data together, with the packing format by default being [`crate::vecs::Index16`], but can be
/// overridden with `#[zerovec::format(zerovec::vecs::Index8)]`.
///
/// [`EncodeAsVarULE`]: ule::EncodeAsVarULE
/// [`VarULE`]: ule::VarULE
/// [`ULE`]: ule::ULE
/// [`AsULE`]: ule::AsULE
/// [`ZeroMapKV`]: maps::ZeroMapKV
///
/// # Example
///
/// ```rust
/// use std::borrow::Cow;
/// use zerofrom::ZeroFrom;
/// use zerovec::ule::encode_varule_to_box;
/// use zerovec::{VarZeroVec, ZeroMap, ZeroVec};
///
/// // custom fixed-size ULE type for ZeroVec
/// #[zerovec::make_ule(DateULE)]
/// #[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
/// struct Date {
///     y: u64,
///     m: u8,
///     d: u8,
/// }
///
/// // custom variable sized VarULE type for VarZeroVec
/// #[zerovec::make_varule(PersonULE)]
/// #[zerovec::derive(Serialize, Deserialize)]
/// #[derive(Clone, PartialEq, Eq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
/// struct Person<'a> {
///     birthday: Date,
///     favorite_character: char,
///     #[serde(borrow)]
///     name: Cow<'a, str>,
/// }
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Data<'a> {
///     // note: VarZeroVec always must reference the ULE type directly
///     #[serde(borrow)]
///     important_people: VarZeroVec<'a, PersonULE>,
/// }
///
/// let person1 = Person {
///     birthday: Date {
///         y: 1990,
///         m: 9,
///         d: 7,
///     },
///     favorite_character: 'π',
///     name: Cow::from("Kate"),
/// };
/// let person2 = Person {
///     birthday: Date {
///         y: 1960,
///         m: 5,
///         d: 25,
///     },
///     favorite_character: '冇',
///     name: Cow::from("Jesse"),
/// };
///
/// let important_people = VarZeroVec::from(&[person1, person2]);
/// let data = Data { important_people };
///
/// let bincode_bytes = bincode::serialize(&data).expect("Serialization should be successful");
///
/// // Will deserialize without allocations
/// let deserialized: Data =
///     bincode::deserialize(&bincode_bytes).expect("Deserialization should be successful");
///
/// assert_eq!(&deserialized.important_people.get(1).unwrap().name, "Jesse");
/// assert_eq!(&deserialized.important_people.get(0).unwrap().name, "Kate");
///
/// // Since VarZeroVec produces PersonULE types, it's convenient to use ZeroFrom
/// // to recoup Person values in a zero-copy way
/// let person_converted: Person =
///     ZeroFrom::zero_from(deserialized.important_people.get(1).unwrap());
/// assert_eq!(person_converted.name, "Jesse");
/// assert_eq!(person_converted.birthday.y, 1960);
/// ```
#[cfg(feature = "derive")]
pub use zerovec_derive::make_varule;

#[cfg(test)]
// Expected sizes are based on a 64-bit architecture
#[cfg(target_pointer_width = "64")]
mod tests {
    use super::*;
    use core::mem::size_of;

    /// Checks that the size of the type is one of the given sizes.
    /// The size might differ across Rust versions or channels.
    macro_rules! check_size_of {
        ($sizes:pat, $type:path) => {
            assert!(
                matches!(size_of::<$type>(), $sizes),
                concat!(stringify!($type), " is of size {}"),
                size_of::<$type>()
            );
        };
    }

    #[test]
    fn check_sizes() {
        check_size_of!(24, ZeroVec<u8>);
        check_size_of!(24, ZeroVec<u32>);
        check_size_of!(32 | 24, VarZeroVec<[u8]>);
        check_size_of!(32 | 24, VarZeroVec<str>);
        check_size_of!(48, ZeroMap<u32, u32>);
        check_size_of!(56 | 48, ZeroMap<u32, str>);
        check_size_of!(56 | 48, ZeroMap<str, u32>);
        check_size_of!(64 | 48, ZeroMap<str, str>);
        check_size_of!(120 | 96, ZeroMap2d<str, str, str>);

        check_size_of!(24, Option<ZeroVec<u8>>);
        check_size_of!(32 | 24, Option<VarZeroVec<str>>);
        check_size_of!(64 | 56 | 48, Option<ZeroMap<str, str>>);
        check_size_of!(120 | 104 | 96, Option<ZeroMap2d<str, str, str>>);
    }
}
