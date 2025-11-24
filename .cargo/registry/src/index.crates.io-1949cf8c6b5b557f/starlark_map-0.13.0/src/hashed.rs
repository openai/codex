/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;

use allocative::Allocative;
use dupe::Dupe;
use equivalent::Equivalent;

use crate::hash_value::StarlarkHashValue;

/// A key and its hash.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Dupe, Allocative)]
pub struct Hashed<K> {
    hash: StarlarkHashValue,
    key: K,
}

// We deliberately know that this is a hash and value, so our Eq/Hash are fine
#[allow(clippy::derived_hash_with_manual_eq)]
impl<K> Hash for Hashed<K> {
    fn hash<S: Hasher>(&self, state: &mut S) {
        // Only hash the hash, not the key.
        self.hash.hash(state)
    }
}

impl<K> Deref for Hashed<K> {
    type Target = K;

    fn deref(&self) -> &Self::Target {
        &self.key
    }
}

impl<K: Display> Display for Hashed<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.key.fmt(f)
    }
}

impl<K: PartialOrd> PartialOrd for Hashed<K> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.key.partial_cmp(&other.key)
    }
}

impl<K: Ord> Ord for Hashed<K> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

impl<K> Hashed<K> {
    /// Create a new [`Hashed`] value using the [`Hash`] of the key.
    pub fn new(key: K) -> Self
    where
        K: Hash,
    {
        Hashed::new_unchecked(StarlarkHashValue::new(&key), key)
    }

    /// Directly create a new [`Hashed`] using a given hash value.
    /// If the hash does not correspond to the key, its will cause issues.
    pub fn new_unchecked(hash: StarlarkHashValue, key: K) -> Self {
        Hashed { hash, key }
    }

    /// Get the underlying key.
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Get the underlying key, as mutable.
    pub fn key_mut(&mut self) -> &mut K {
        &mut self.key
    }

    /// Get the underlying key taking ownership.
    pub fn into_key(self) -> K {
        self.key
    }

    /// Get the underlying hash.
    pub fn hash(&self) -> StarlarkHashValue {
        self.hash
    }

    /// Convert `Hashed<K>` to `Hashed<&K>`.
    pub fn as_ref(&self) -> Hashed<&K> {
        Hashed::new_unchecked(self.hash, &self.key)
    }
}

impl<'a, K> Hashed<&'a K> {
    /// Make `Hashed<K>` from `Hashed<&K>`.
    pub fn copied(self) -> Hashed<K>
    where
        K: Copy,
    {
        Hashed::new_unchecked(self.hash, *self.key)
    }

    /// Make `Hashed<K>` from `Hashed<&K>`, where `K` is `Clone`.
    pub fn cloned(self) -> Hashed<K>
    where
        K: Clone,
    {
        Hashed::new_unchecked(self.hash, self.key.clone())
    }
}

impl<'a, K: ?Sized> Hashed<&'a K> {
    /// Make `Hashed<K>` from `Hashed<&K>`, where `T` is the owned form of `K`.
    pub fn owned<T>(self) -> Hashed<T>
    where
        K: Equivalent<T>,
        K: ToOwned<Owned = T>,
    {
        Hashed::new_unchecked(self.hash, self.key.to_owned())
    }
}
