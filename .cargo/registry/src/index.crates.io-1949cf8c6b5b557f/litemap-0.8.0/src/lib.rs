// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! # `litemap`
//!
//! `litemap` is a crate providing [`LiteMap`], a highly simplistic "flat" key-value map
//! based off of a single sorted vector.
//!
//! The main goal of this crate is to provide a map that is good enough for small
//! sizes, and does not carry the binary size impact of [`HashMap`](std::collections::HashMap)
//! or [`BTreeMap`](alloc::collections::BTreeMap).
//!
//! If binary size is not a concern, [`std::collections::BTreeMap`] may be a better choice
//! for your use case. It behaves very similarly to [`LiteMap`] for less than 12 elements,
//! and upgrades itself gracefully for larger inputs.
//!
//! ## Performance characteristics
//!
//! [`LiteMap`] is a data structure with similar characteristics as [`std::collections::BTreeMap`] but
//! with slightly different trade-offs as it's implemented on top of a flat storage (e.g. Vec).
//!
//! * [`LiteMap`] iteration is generally faster than `BTreeMap` because of the flat storage.
//! * [`LiteMap`] can be pre-allocated whereas `BTreeMap` can't.
//! * [`LiteMap`] has a smaller memory footprint than `BTreeMap` for small collections (< 20 items).
//! * Lookup is `O(log(n))` like `BTreeMap`.
//! * Insertion is generally `O(n)`, but optimized to `O(1)` if the new item sorts greater than the current items. In `BTreeMap` it's `O(log(n))`.
//! * Deletion is `O(n)` whereas `BTreeMap` is `O(log(n))`.
//! * Bulk operations like `from_iter`, `extend` and deserialization have an optimized `O(n)` path
//!   for inputs that are ordered and `O(n*log(n))` complexity otherwise.
//!
//! ## Pluggable Backends
//!
//! By default, [`LiteMap`] is backed by a [`Vec`]; however, it can be backed by any appropriate
//! random-access data store, giving that data store a map-like interface. See the [`store`]
//! module for more details.
//!
//! ## Const construction
//!
//! [`LiteMap`] supports const construction from any store that is const-constructible, such as a
//! static slice, via [`LiteMap::from_sorted_store_unchecked()`]. This also makes [`LiteMap`]
//! suitable for use with [`databake`]. See [`impl Bake for LiteMap`] for more details.
//!
//! [`impl Bake for LiteMap`]: ./struct.LiteMap.html#impl-Bake-for-LiteMap<K,+V,+S>
//! [`Vec`]: alloc::vec::Vec

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

// for intra doc links
#[cfg(doc)]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "databake")]
#[path = "databake.rs"] // to not conflict with `databake` as used in the docs
mod databake_impls;
mod map;
#[cfg(feature = "serde")]
mod serde;
#[cfg(feature = "serde")]
mod serde_helpers;
pub mod store;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use map::{Entry, LiteMap, OccupiedEntry, VacantEntry};
