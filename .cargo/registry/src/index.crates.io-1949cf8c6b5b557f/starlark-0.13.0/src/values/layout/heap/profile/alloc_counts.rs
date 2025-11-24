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

use std::iter::Sum;
use std::ops::Add;
use std::ops::AddAssign;

use allocative::Allocative;
use dupe::Dupe;

/// Allocations counters.
#[derive(Default, Copy, Clone, Dupe, Debug, Allocative)]
pub(crate) struct AllocCounts {
    pub(crate) bytes: usize,
    pub(crate) count: usize,
}

impl AllocCounts {
    #[cfg(test)]
    pub(crate) fn normalize_for_golden_tests(&mut self) {
        // Value sizes depend on compiler version, so normalize them.
        self.bytes = self.count * 8;
    }
}

impl AddAssign for AllocCounts {
    fn add_assign(&mut self, other: AllocCounts) {
        self.bytes += other.bytes;
        self.count += other.count;
    }
}

impl Add for AllocCounts {
    type Output = AllocCounts;

    fn add(self, other: AllocCounts) -> AllocCounts {
        AllocCounts {
            bytes: self.bytes + other.bytes,
            count: self.count + other.count,
        }
    }
}

impl<'a> Sum<&'a AllocCounts> for AllocCounts {
    fn sum<I>(iter: I) -> AllocCounts
    where
        I: Iterator<Item = &'a AllocCounts>,
    {
        iter.fold(AllocCounts::default(), |acc, x| acc + *x)
    }
}
