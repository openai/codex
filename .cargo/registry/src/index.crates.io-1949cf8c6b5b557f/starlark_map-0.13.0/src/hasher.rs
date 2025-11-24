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

use std::hash::BuildHasher;
use std::hash::Hasher;

use dupe::Dupe;
use fxhash::FxHasher64;

use crate::hash_value::StarlarkHashValue;

/// A hasher used by Starlark implementation.
///
/// Starlark relies on stable hashing, and this is the hasher.
#[derive(Default)]
pub struct StarlarkHasher(
    // TODO(nga): `FxHasher64` is endian-dependent, this is not right.
    FxHasher64,
);

impl StarlarkHasher {
    /// Creates a new hasher.
    #[inline]
    pub fn new() -> StarlarkHasher {
        StarlarkHasher::default()
    }

    /// Finish the hash computation and return the result.
    #[inline]
    pub fn finish_small(&self) -> StarlarkHashValue {
        // NOTE: Here we throw away half the key material we are given,
        // taking only the lower 32 bits.
        // Not a problem because `DefaultHasher` produces well-swizzled bits.
        StarlarkHashValue::new_unchecked(self.finish() as u32)
    }
}

impl Hasher for StarlarkHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes)
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.0.write_u8(i)
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.0.write_u16(i)
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.0.write_u32(i)
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0.write_u64(i)
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.0.write_u128(i)
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.0.write_usize(i)
    }
}

/// [`BuildHasher`] implementation which produces [`StarlarkHasher`].
#[derive(Default, Debug, Clone, Copy, Dupe)]
pub struct StarlarkHasherBuilder;

impl BuildHasher for StarlarkHasherBuilder {
    type Hasher = StarlarkHasher;

    #[inline]
    fn build_hasher(&self) -> StarlarkHasher {
        StarlarkHasher::default()
    }
}
