/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::any;
use std::cmp::Ordering;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;

/// Hashed string, which is a key while descending into a tree (e.g. type name or field name).
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Key {
    hash: u64,
    s: &'static str,
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.s.cmp(other.s)
    }
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl Deref for Key {
    type Target = str;

    fn deref(&self) -> &str {
        self.s
    }
}

impl Key {
    /// Must be identical to `allocative_derive::hash`.
    const fn hash(s: &str) -> u64 {
        let mut hash = 0xcbf29ce484222325;
        let mut i = 0;
        while i < s.as_bytes().len() {
            let b = s.as_bytes()[i];
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
            i += 1;
        }
        hash
    }

    /// Compute hash.
    pub const fn new(s: &'static str) -> Key {
        let hash = Self::hash(s);
        Key::new_unchecked(hash, s)
    }

    pub const fn new_unchecked(hash: u64, s: &'static str) -> Key {
        Key { hash, s }
    }

    pub fn for_type_name<T: ?Sized>() -> Key {
        // Compute hash at compile time.
        #[cfg(rust_nightly)]
        return Key {
            hash: AllocativeKeyForType::<T>::KEY.hash,
            s: AllocativeKeyForType::<T>::KEY.s,
        };
        // Hope optimizer folds this to constant (it doesn't for long type names).
        #[cfg(not(rust_nightly))]
        return Key::new(any::type_name::<T>());
    }
}

#[cfg(rust_nightly)]
struct AllocativeKeyForType<T: ?Sized>(std::marker::PhantomData<fn(&T)>);

#[cfg(rust_nightly)]
impl<T: ?Sized> AllocativeKeyForType<T> {
    /// Force compute it at compile time. Const fn does not guarantee that.
    pub const KEY: Key = Key::new(any::type_name::<T>());
}
