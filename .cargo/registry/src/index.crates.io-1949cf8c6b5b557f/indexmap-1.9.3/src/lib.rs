// We *mostly* avoid unsafe code, but `map::core::raw` allows it to use `RawTable` buckets.
#![deny(unsafe_code)]
#![warn(rust_2018_idioms)]
#![doc(html_root_url = "https://docs.rs/indexmap/1/")]
#![no_std]

//! [`IndexMap`] is a hash table where the iteration order of the key-value
//! pairs is independent of the hash values of the keys.
//!
//! [`IndexSet`] is a corresponding hash set using the same implementation and
//! with similar properties.
//!
//! [`IndexMap`]: map/struct.IndexMap.html
//! [`IndexSet`]: set/struct.IndexSet.html
//!
//!
//! ### Feature Highlights
//!
//! [`IndexMap`] and [`IndexSet`] are drop-in compatible with the std `HashMap`
//! and `HashSet`, but they also have some features of note:
//!
//! - The ordering semantics (see their documentation for details)
//! - Sorting methods and the [`.pop()`][IndexMap::pop] methods.
//! - The [`Equivalent`] trait, which offers more flexible equality definitions
//!   between borrowed and owned versions of keys.
//! - The [`MutableKeys`][map::MutableKeys] trait, which gives opt-in mutable
//!   access to hash map keys.
//!
//! ### Alternate Hashers
//!
//! [`IndexMap`] and [`IndexSet`] have a default hasher type `S = RandomState`,
//! just like the standard `HashMap` and `HashSet`, which is resistant to
//! HashDoS attacks but not the most performant. Type aliases can make it easier
//! to use alternate hashers:
//!
//! ```
//! use fnv::FnvBuildHasher;
//! use fxhash::FxBuildHasher;
//! use indexmap::{IndexMap, IndexSet};
//!
//! type FnvIndexMap<K, V> = IndexMap<K, V, FnvBuildHasher>;
//! type FnvIndexSet<T> = IndexSet<T, FnvBuildHasher>;
//!
//! type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;
//! type FxIndexSet<T> = IndexSet<T, FxBuildHasher>;
//!
//! let std: IndexSet<i32> = (0..100).collect();
//! let fnv: FnvIndexSet<i32> = (0..100).collect();
//! let fx: FxIndexSet<i32> = (0..100).collect();
//! assert_eq!(std, fnv);
//! assert_eq!(std, fx);
//! ```
//!
//! ### Rust Version
//!
//! This version of indexmap requires Rust 1.56 or later.
//!
//! The indexmap 1.x release series will use a carefully considered version
//! upgrade policy, where in a later 1.x version, we will raise the minimum
//! required Rust version.
//!
//! ## No Standard Library Targets
//!
//! This crate supports being built without `std`, requiring
//! `alloc` instead. This is enabled automatically when it is detected that
//! `std` is not available. There is no crate feature to enable/disable to
//! trigger this. It can be tested by building for a std-less target.
//!
//! - Creating maps and sets using [`new`][IndexMap::new] and
//! [`with_capacity`][IndexMap::with_capacity] is unavailable without `std`.  
//!   Use methods [`IndexMap::default`][def],
//!   [`with_hasher`][IndexMap::with_hasher],
//!   [`with_capacity_and_hasher`][IndexMap::with_capacity_and_hasher] instead.
//!   A no-std compatible hasher will be needed as well, for example
//!   from the crate `twox-hash`.
//! - Macros [`indexmap!`] and [`indexset!`] are unavailable without `std`.
//!
//! [def]: map/struct.IndexMap.html#impl-Default

extern crate alloc;

#[cfg(has_std)]
#[macro_use]
extern crate std;

use alloc::vec::{self, Vec};

mod arbitrary;
#[macro_use]
mod macros;
mod equivalent;
mod mutable_keys;
#[cfg(feature = "serde")]
mod serde;
#[cfg(feature = "serde")]
pub mod serde_seq;
mod util;

pub mod map;
pub mod set;

// Placed after `map` and `set` so new `rayon` methods on the types
// are documented after the "normal" methods.
#[cfg(feature = "rayon")]
mod rayon;

#[cfg(feature = "rustc-rayon")]
mod rustc;

pub use crate::equivalent::Equivalent;
pub use crate::map::IndexMap;
pub use crate::set::IndexSet;

// shared private items

/// Hash value newtype. Not larger than usize, since anything larger
/// isn't used for selecting position anyway.
#[derive(Clone, Copy, Debug, PartialEq)]
struct HashValue(usize);

impl HashValue {
    #[inline(always)]
    fn get(self) -> u64 {
        self.0 as u64
    }
}

#[derive(Copy, Debug)]
struct Bucket<K, V> {
    hash: HashValue,
    key: K,
    value: V,
}

impl<K, V> Clone for Bucket<K, V>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Bucket {
            hash: self.hash,
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }

    fn clone_from(&mut self, other: &Self) {
        self.hash = other.hash;
        self.key.clone_from(&other.key);
        self.value.clone_from(&other.value);
    }
}

impl<K, V> Bucket<K, V> {
    // field accessors -- used for `f` instead of closures in `.map(f)`
    fn key_ref(&self) -> &K {
        &self.key
    }
    fn value_ref(&self) -> &V {
        &self.value
    }
    fn value_mut(&mut self) -> &mut V {
        &mut self.value
    }
    fn key(self) -> K {
        self.key
    }
    fn value(self) -> V {
        self.value
    }
    fn key_value(self) -> (K, V) {
        (self.key, self.value)
    }
    fn refs(&self) -> (&K, &V) {
        (&self.key, &self.value)
    }
    fn ref_mut(&mut self) -> (&K, &mut V) {
        (&self.key, &mut self.value)
    }
    fn muts(&mut self) -> (&mut K, &mut V) {
        (&mut self.key, &mut self.value)
    }
}

trait Entries {
    type Entry;
    fn into_entries(self) -> Vec<Self::Entry>;
    fn as_entries(&self) -> &[Self::Entry];
    fn as_entries_mut(&mut self) -> &mut [Self::Entry];
    fn with_entries<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [Self::Entry]);
}
