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

use std::collections::HashMap;

use allocative::Allocative;
use starlark_map::small_map::SmallMap;

use crate::values::layout::heap::profile::alloc_counts::AllocCounts;

#[derive(Debug, Default, Clone, Allocative)]
/// Information about the data stored on a heap. Accessible through
/// the function `allocated_summary` available on [`Heap`](crate::values::Heap)
/// and [`FrozenHeap`](crate::values::FrozenHeap)
pub struct HeapSummary {
    /// For each type, give the (number of entries, size of all entries).
    /// The size may be approximate as it includes information from
    /// the approximate [`memory_size`](StarlarkValue::memory_size) function.
    pub(crate) summary: SmallMap<&'static str, AllocCounts>,
}

impl HeapSummary {
    /// (Count, total size) by type.
    pub fn summary(&self) -> HashMap<String, (usize, usize)> {
        self.summary
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (v.count, v.bytes)))
            .collect()
    }

    pub(crate) fn total(&self) -> AllocCounts {
        self.summary.values().sum()
    }

    /// Total number of bytes allocated.
    pub fn total_allocated_bytes(&self) -> usize {
        self.total().bytes
    }

    pub(crate) fn add(&mut self, t: &'static str, s: AllocCounts) {
        *self.summary.entry(t).or_default() += s;
    }

    pub(crate) fn merge<'a>(heaps: impl IntoIterator<Item = &'a HeapSummary>) -> HeapSummary {
        let mut summary = SmallMap::new();
        for heap in heaps {
            for (k, v) in heap.summary.iter() {
                *summary.entry(*k).or_default() += *v;
            }
        }
        HeapSummary { summary }
    }

    #[cfg(test)]
    pub(crate) fn normalize_for_golden_tests(&mut self) {
        for v in self.summary.values_mut() {
            v.normalize_for_golden_tests();
        }
    }
}
