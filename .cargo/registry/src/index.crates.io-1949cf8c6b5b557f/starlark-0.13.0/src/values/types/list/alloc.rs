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

use std::iter;

use crate::typing::Ty;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;

/// Utility to allocate a list from iterator.
///
/// # Example
///
/// ```
/// use starlark::values::list::AllocList;
///
/// # use starlark::values::{FrozenHeap, Heap};
/// # fn alloc(heap: &Heap, frozen_heap: &FrozenHeap) {
/// let l = heap.alloc(AllocList([1, 2, 3]));
/// let ls = frozen_heap.alloc(AllocList([1, 2, 3]));
/// # }
/// ```
pub struct AllocList<L>(pub L);

impl AllocList<iter::Empty<FrozenValue>> {
    /// Allocate an empty list.
    pub const EMPTY: AllocList<iter::Empty<FrozenValue>> = AllocList(iter::empty());
}

impl<L> StarlarkTypeRepr for AllocList<L>
where
    L: IntoIterator,
    L::Item: StarlarkTypeRepr,
{
    type Canonical = <Vec<L::Item> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Vec::<L::Item>::starlark_type_repr()
    }
}

impl<'v, L> AllocValue<'v> for AllocList<L>
where
    L: IntoIterator,
    L::Item: AllocValue<'v>,
{
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_list_iter(self.0.into_iter().map(|x| heap.alloc(x)))
    }
}

impl<L> AllocFrozenValue for AllocList<L>
where
    L: IntoIterator,
    L::Item: AllocFrozenValue,
{
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_list_iter(self.0.into_iter().map(|x| heap.alloc(x)))
    }
}
