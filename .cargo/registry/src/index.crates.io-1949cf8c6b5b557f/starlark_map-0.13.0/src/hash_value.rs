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
use std::hash::Hash;

use allocative::Allocative;
use dupe::Dupe;

use crate::hasher::StarlarkHasher;
use crate::mix_u32::mix_u32;

/// A hash value.
#[derive(Clone, Copy, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct StarlarkHashValue(u32);

impl StarlarkHashValue {
    /// Create a new [`StarlarkHashValue`] using the [`Hash`] trait
    /// for given key.
    #[inline]
    pub fn new<K: Hash + ?Sized>(key: &K) -> Self {
        let mut hasher = StarlarkHasher::default();
        key.hash(&mut hasher);
        hasher.finish_small()
    }

    /// Directly create a new [`StarlarkHashValue`] using a hash.
    /// The expectation is that the key will be well-swizzled,
    /// or there may be many hash collisions.
    #[inline]
    pub const fn new_unchecked(hash: u32) -> Self {
        StarlarkHashValue(hash)
    }

    /// Hash 64-bit integer.
    ///
    /// Input can also be a non-well swizzled hash to create better hash.
    #[inline]
    pub const fn hash_64(h: u64) -> Self {
        // `fmix64` function from MurMur3 hash (which is in public domain).
        // https://github.com/aappleby/smhasher/blob/61a0530f28277f2e850bfc39600ce61d02b518de/src/MurmurHash3.cpp#L81

        let h = h ^ (h >> 33);
        let h = h.wrapping_mul(0xff51afd7ed558ccd);
        let h = h ^ (h >> 33);
        let h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        let h = h ^ (h >> 33);

        StarlarkHashValue::new_unchecked(h as u32)
    }

    /// Get the integer hash value.
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Make u64 hash from this hash.
    ///
    /// The resulting hash should be good enough to be used in hashbrown hashtable.
    #[inline]
    pub fn promote(self) -> u64 {
        mix_u32(self.0)
    }
}
