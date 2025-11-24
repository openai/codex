/*
 * Copyright 2018 The Starlark in Rust Authors.
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

use std::hash::Hash;
use std::hash::Hasher;

use starlark_map::StarlarkHasher;

/// Utility to compute hash of an unordered collection (e.g. unordered set).
#[derive(Default)]
pub(crate) struct UnorderedHasher {
    state: u64,
    count: u64,
}

impl UnorderedHasher {
    pub(crate) fn new() -> UnorderedHasher {
        UnorderedHasher::default()
    }
}

impl UnorderedHasher {
    pub(crate) fn write_hash(&mut self, value: u64) {
        self.state = self.state.wrapping_add(value);
        self.count = self.count.wrapping_add(1);
    }

    pub(crate) fn _write<H: Hash>(&mut self, value: H) {
        let mut hasher = StarlarkHasher::new();
        value.hash(&mut hasher);
        self.write_hash(hasher.finish());
    }

    pub(crate) fn finish(self) -> u64 {
        let mut hasher = StarlarkHasher::new();
        self.state.hash(&mut hasher);
        self.count.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::values::structs::unordered_hasher::UnorderedHasher;

    #[test]
    fn test_unordered_hasher() {
        let mut h = UnorderedHasher::new();
        h.write_hash(10);
        h.write_hash(20);
        let h0 = h.finish();

        let mut h = UnorderedHasher::new();
        h.write_hash(20);
        h.write_hash(10);
        let h1 = h.finish();

        assert_eq!(h0, h1);
    }
}
