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

//! An optimised string HashMap which goes even faster when the keys can
//! be pre-hashed or otherwise precomputed.
//!
//! The two bottlenecks in our use of these hash tables are computing the hashes and comparing
//! the resulting keys for equality. We precompute the hashes. We also use `[usize]` to do faster
//! comparison when possible. We use the Starlark SmallHash hashes, promoted by IdHasher,
//! so we can reuse a SmallMap hash.
//!
//! Benchmarks on which the `[usize]` choice was made (mac/linux, all in ns):
//!                           8 bytes       32 bytes      64 bytes
//! slice equality (memcmp)   3.5/3.8       3.5/ 3.0      4.5/ 4.7
//! usize equality loop       1.0/1.4       2.7/ 3.5      3.5/ 6.0
//! u8 equality loop          3.4/5.7      13.7/19.7     22.6/44.8
//!
//! Measuring some sample strings, the P50 = 21 bytes, P75 = 27, P95 = 35,
//! so we can reasonably expect to hit the smaller cases most often.

use std::fmt;
use std::fmt::Debug;
use std::mem;

use allocative::Allocative;
use hashbrown::HashTable;
use starlark_derive::Trace;
use starlark_map::Hashed;

use crate as starlark;
use crate::collections::symbol::symbol::Symbol;
use crate::values::StringValue;

// We use a RawTable (the thing that underlies HashMap) so we can look up efficiently
// and easily by Symbol and str, without being limited by `Borrow` traits.
#[derive(Clone, Trace, Allocative)]
pub(crate) struct SymbolMap<T>(HashTable<(Symbol, T)>);

impl<T: Debug> Debug for SymbolMap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.iter().map(|x| (&x.0, &x.1)))
            .finish()
    }
}

impl<T> SymbolMap<T> {
    pub(crate) fn new() -> Self {
        SymbolMap::with_capacity(0)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        SymbolMap(HashTable::with_capacity(capacity))
    }

    pub(crate) fn insert(&mut self, key: &str, value: T) -> Option<T> {
        let s = Symbol::new(key);
        if let Some((_, item)) = self.0.find_mut(s.hash(), |x| s == x.0) {
            Some(mem::replace(item, value))
        } else {
            // This insert doesn't remove old values, so do that manually first
            self.0.insert_unique(s.hash(), (s, value), |x| x.0.hash());
            None
        }
    }

    #[inline]
    pub(crate) fn get(&self, key: &Symbol) -> Option<&T> {
        self.0.find(key.hash(), |x| key == &x.0).map(|x| &x.1)
    }

    pub(crate) fn get_str(&self, key: &str) -> Option<&T> {
        self.get_hashed_str(Hashed::new(key))
    }

    pub(crate) fn get_hashed_str(&self, key: Hashed<&str>) -> Option<&T> {
        self.0
            .find(key.hash().promote(), |x| x.0.as_str() == *key.key())
            .map(|x| &x.1)
    }

    pub(crate) fn get_hashed_string_value(&self, key: Hashed<StringValue>) -> Option<&T> {
        self.0
            .find(key.hash().promote(), |x| {
                x.0.as_aligned_padded_str() == key.key().as_aligned_padded_str()
            })
            .map(|x| &x.1)
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a (Symbol, T)> + 'a {
        self.0.iter()
    }

    pub(crate) fn keys<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a Symbol> + 'a {
        self.iter().map(|x| &x.0)
    }

    pub(crate) fn values<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a T> + 'a {
        self.iter().map(|x| &x.1)
    }
}
