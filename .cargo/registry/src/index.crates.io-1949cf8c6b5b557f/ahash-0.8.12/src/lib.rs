//! AHash is a high performance keyed hash function.
//!
//! It quickly provides a high quality hash where the result is not predictable without knowing the Key.
//! AHash works with `HashMap` to hash keys, but without allowing for the possibility that an malicious user can
//! induce a collision.
//!
//! # How aHash works
//!
//! When it is available aHash uses the hardware AES instructions to provide a keyed hash function.
//! When it is not, aHash falls back on a slightly slower alternative algorithm.
//!
//! Because aHash does not have a fixed standard for its output, it is able to improve over time.
//! But this also means that different computers or computers using different versions of ahash may observe different
//! hash values for the same input.
#![cfg_attr(
    all(
        feature = "std",
        any(feature = "compile-time-rng", feature = "runtime-rng", feature = "no-rng")
    ),
    doc = r##"
# Basic Usage
AHash provides an implementation of the [Hasher] trait.
To construct a HashMap using aHash as its hasher do the following:
```
use ahash::{AHasher, RandomState};
use std::collections::HashMap;

let mut map: HashMap<i32, i32, RandomState> = HashMap::default();
map.insert(12, 34);
```

### Randomness

The above requires a source of randomness to generate keys for the hashmap. By default this obtained from the OS.
It is also possible to have randomness supplied via the `compile-time-rng` flag, or manually.

### If randomness is not available

[AHasher::default()] can be used to hash using fixed keys. This works with
[BuildHasherDefault](std::hash::BuildHasherDefault). For example:

```
use std::hash::BuildHasherDefault;
use std::collections::HashMap;
use ahash::AHasher;

let mut m: HashMap<_, _, BuildHasherDefault<AHasher>> = HashMap::default();
 # m.insert(12, 34);
```
It is also possible to instantiate [RandomState] directly:

```
use ahash::HashMap;
use ahash::RandomState;

let mut m = HashMap::with_hasher(RandomState::with_seed(42));
 # m.insert(1, 2);
```
Or for uses besides a hashhmap:
```
use std::hash::BuildHasher;
use ahash::RandomState;

let hash_builder = RandomState::with_seed(42);
let hash = hash_builder.hash_one("Some Data");
```
There are several constructors for [RandomState] with different ways to supply seeds.

# Convenience wrappers

For convenience, both new-type wrappers and type aliases are provided.

The new type wrappers are called called `AHashMap` and `AHashSet`.
```
use ahash::AHashMap;

let mut map: AHashMap<i32, i32> = AHashMap::new();
map.insert(12, 34);
```
This avoids the need to type "RandomState". (For convenience `From`, `Into`, and `Deref` are provided).

# Aliases

For even less typing and better interop with existing libraries (such as rayon) which require a `std::collection::HashMap` ,
the type aliases [HashMap], [HashSet] are provided.

```
use ahash::{HashMap, HashMapExt};

let mut map: HashMap<i32, i32> = HashMap::new();
map.insert(12, 34);
```
Note the import of [HashMapExt]. This is needed for the constructor.

"##
)]
#![deny(clippy::correctness, clippy::complexity, clippy::perf)]
#![allow(clippy::pedantic, clippy::cast_lossless, clippy::unreadable_literal)]
#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![cfg_attr(specialize, feature(min_specialization))]
#![cfg_attr(feature = "nightly-arm-aes", feature(stdarch_arm_neon_intrinsics))]

#[macro_use]
mod convert;

mod fallback_hash;

cfg_if::cfg_if! {
    if #[cfg(any(
            all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "aes", not(miri)),
            all(feature = "nightly-arm-aes", target_arch = "aarch64", target_feature = "aes", not(miri)),
            all(feature = "nightly-arm-aes", target_arch = "arm", target_feature = "aes", not(miri)),
        ))] {
        mod aes_hash;
        pub use crate::aes_hash::AHasher;
    } else {
        pub use crate::fallback_hash::AHasher;
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        mod hash_map;
        mod hash_set;

        pub use crate::hash_map::AHashMap;
        pub use crate::hash_set::AHashSet;

        /// [Hasher]: std::hash::Hasher
        /// [HashMap]: std::collections::HashMap
        /// Type alias for [HashMap]<K, V, ahash::RandomState>
        pub type HashMap<K, V> = std::collections::HashMap<K, V, crate::RandomState>;

        /// Type alias for [HashSet]<K, ahash::RandomState>
        pub type HashSet<K> = std::collections::HashSet<K, crate::RandomState>;
    }
}

#[cfg(test)]
mod hash_quality_test;

mod operations;
pub mod random_state;
mod specialize;

pub use crate::random_state::RandomState;

use core::hash::BuildHasher;

#[cfg(feature = "std")]
/// A convenience trait that can be used together with the type aliases defined to
/// get access to the `new()` and `with_capacity()` methods for the HashMap type alias.
pub trait HashMapExt {
    /// Constructs a new HashMap
    fn new() -> Self;
    /// Constructs a new HashMap with a given initial capacity
    fn with_capacity(capacity: usize) -> Self;
}

#[cfg(feature = "std")]
/// A convenience trait that can be used together with the type aliases defined to
/// get access to the `new()` and `with_capacity()` methods for the HashSet type aliases.
pub trait HashSetExt {
    /// Constructs a new HashSet
    fn new() -> Self;
    /// Constructs a new HashSet with a given initial capacity
    fn with_capacity(capacity: usize) -> Self;
}

#[cfg(feature = "std")]
impl<K, V, S> HashMapExt for std::collections::HashMap<K, V, S>
where
    S: BuildHasher + Default,
{
    fn new() -> Self {
        std::collections::HashMap::with_hasher(S::default())
    }

    fn with_capacity(capacity: usize) -> Self {
        std::collections::HashMap::with_capacity_and_hasher(capacity, S::default())
    }
}

#[cfg(feature = "std")]
impl<K, S> HashSetExt for std::collections::HashSet<K, S>
where
    S: BuildHasher + Default,
{
    fn new() -> Self {
        std::collections::HashSet::with_hasher(S::default())
    }

    fn with_capacity(capacity: usize) -> Self {
        std::collections::HashSet::with_capacity_and_hasher(capacity, S::default())
    }
}

/// Provides a default [Hasher] with fixed keys.
/// This is typically used in conjunction with [BuildHasherDefault] to create
/// [AHasher]s in order to hash the keys of the map.
///
/// Generally it is preferable to use [RandomState] instead, so that different
/// hashmaps will have different keys. However if fixed keys are desirable this
/// may be used instead.
///
/// # Example
/// ```
/// use std::hash::BuildHasherDefault;
/// use ahash::{AHasher, RandomState};
/// use std::collections::HashMap;
///
/// let mut map: HashMap<i32, i32, BuildHasherDefault<AHasher>> = HashMap::default();
/// map.insert(12, 34);
/// ```
///
/// [BuildHasherDefault]: std::hash::BuildHasherDefault
/// [Hasher]: std::hash::Hasher
/// [HashMap]: std::collections::HashMap
impl Default for AHasher {
    /// Constructs a new [AHasher] with fixed keys.
    /// If `std` is enabled these will be generated upon first invocation.
    /// Otherwise if the `compile-time-rng`feature is enabled these will be generated at compile time.
    /// If neither of these features are available, hardcoded constants will be used.
    ///
    /// Because the values are fixed, different hashers will all hash elements the same way.
    /// This could make hash values predictable, if DOS attacks are a concern. If this behaviour is
    /// not required, it may be preferable to use [RandomState] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use ahash::AHasher;
    /// use std::hash::Hasher;
    ///
    /// let mut hasher_1 = AHasher::default();
    /// let mut hasher_2 = AHasher::default();
    ///
    /// hasher_1.write_u32(1234);
    /// hasher_2.write_u32(1234);
    ///
    /// assert_eq!(hasher_1.finish(), hasher_2.finish());
    /// ```
    #[inline]
    fn default() -> AHasher {
        RandomState::with_fixed_keys().build_hasher()
    }
}

// #[inline(never)]
// #[doc(hidden)]
// pub fn hash_test(input: &[u8]) -> u64 {
//     let a = RandomState::with_seeds(11, 22, 33, 44);
//     <[u8]>::get_hash(input, &a)
// }

#[cfg(feature = "std")]
#[cfg(test)]
mod test {
    use crate::convert::Convert;
    use crate::specialize::CallHasher;
    use crate::*;
    use core::hash::Hash;
    use core::hash::Hasher;
    use std::collections::HashMap;

    #[test]
    fn test_ahash_alias_map_construction() {
        let mut map = super::HashMap::with_capacity(1234);
        map.insert(1, "test");
    }

    #[test]
    fn test_ahash_alias_set_construction() {
        let mut set = super::HashSet::with_capacity(1234);
        set.insert(1);
    }

    #[test]
    fn test_default_builder() {
        use core::hash::BuildHasherDefault;

        let mut map = HashMap::<u32, u64, BuildHasherDefault<AHasher>>::default();
        map.insert(1, 3);
    }

    #[test]
    fn test_builder() {
        let mut map = HashMap::<u32, u64, RandomState>::default();
        map.insert(1, 3);
    }

    #[test]
    fn test_conversion() {
        let input: &[u8] = b"dddddddd";
        let bytes: u64 = as_array!(input, 8).convert();
        assert_eq!(bytes, 0x6464646464646464);
    }

    #[test]
    fn test_non_zero() {
        let mut hasher1 = AHasher::new_with_keys(0, 0);
        let mut hasher2 = AHasher::new_with_keys(0, 0);
        "foo".hash(&mut hasher1);
        "bar".hash(&mut hasher2);
        assert_ne!(hasher1.finish(), 0);
        assert_ne!(hasher2.finish(), 0);
        assert_ne!(hasher1.finish(), hasher2.finish());

        let mut hasher1 = AHasher::new_with_keys(0, 0);
        let mut hasher2 = AHasher::new_with_keys(0, 0);
        3_u64.hash(&mut hasher1);
        4_u64.hash(&mut hasher2);
        assert_ne!(hasher1.finish(), 0);
        assert_ne!(hasher2.finish(), 0);
        assert_ne!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_non_zero_specialized() {
        let hasher_build = RandomState::with_seeds(0, 0, 0, 0);

        let h1 = str::get_hash("foo", &hasher_build);
        let h2 = str::get_hash("bar", &hasher_build);
        assert_ne!(h1, 0);
        assert_ne!(h2, 0);
        assert_ne!(h1, h2);

        let h1 = u64::get_hash(&3_u64, &hasher_build);
        let h2 = u64::get_hash(&4_u64, &hasher_build);
        assert_ne!(h1, 0);
        assert_ne!(h2, 0);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_ahasher_construction() {
        let _ = AHasher::new_with_keys(1234, 5678);
    }

    #[test]
    fn test_specialize_reference_hash() {
        let hasher_build = RandomState::with_seeds(0, 0, 0, 0);
        let h1 = hasher_build.hash_one(1u64);
        let h2 = hasher_build.hash_one(&1u64);

        assert_eq!(h1, h2);

        let h1 = u64::get_hash(&1_u64, &hasher_build);
        let h2 = <&u64>::get_hash(&&1_u64, &hasher_build);

        assert_eq!(h1, h2);

        let h1 = hasher_build.hash_one(1u128);
        let h2 = hasher_build.hash_one(&1u128);

        assert_eq!(h1, h2);
    }
}
