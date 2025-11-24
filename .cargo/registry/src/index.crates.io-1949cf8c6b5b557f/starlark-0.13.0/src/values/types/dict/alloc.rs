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

use starlark_map::small_map::SmallMap;

use crate::typing::Ty;
use crate::values::dict::value::FrozenDictData;
use crate::values::dict::Dict;
use crate::values::layout::value::ValueLike;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::dict::dict_type::DictType;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;

/// Utility to allocate a dict from iterator.
///
/// Iterator must be a list of pairs (key, value).
/// Duplicate keys are allowed, last key wins.
///
/// # Panics
///
/// Panics if a key is not hashable.
///
/// # Example
///
/// ```
/// use starlark::values::dict::AllocDict;
///
/// # use starlark::values::{FrozenHeap, Heap};
/// # fn alloc(heap: &Heap, frozen_heap: &FrozenHeap) {
/// let l = heap.alloc(AllocDict([("a", 1), ("b", 2), ("c", 3)]));
/// let ls = frozen_heap.alloc(AllocDict([("a", 1), ("b", 2), ("c", 3)]));
/// # }
/// ```
pub struct AllocDict<D>(pub D);

impl AllocDict<iter::Empty<(FrozenValue, FrozenValue)>> {
    /// Allocate an empty dict.
    pub const EMPTY: AllocDict<iter::Empty<(FrozenValue, FrozenValue)>> = AllocDict(iter::empty());
}

impl<D, K, V> StarlarkTypeRepr for AllocDict<D>
where
    D: IntoIterator<Item = (K, V)>,
    K: StarlarkTypeRepr,
    V: StarlarkTypeRepr,
{
    type Canonical = DictType<K::Canonical, V::Canonical>;

    fn starlark_type_repr() -> Ty {
        DictType::<K, V>::starlark_type_repr()
    }
}

impl<'v, D, K, V> AllocValue<'v> for AllocDict<D>
where
    D: IntoIterator<Item = (K, V)>,
    K: AllocValue<'v>,
    V: AllocValue<'v>,
{
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        let iter = self.0.into_iter();
        let mut map = SmallMap::with_capacity(iter.size_hint().0);
        for (k, v) in iter {
            map.insert_hashed(
                k.alloc_value(heap).get_hashed().unwrap(),
                v.alloc_value(heap),
            );
        }
        heap.alloc(Dict::new(map))
    }
}

impl<D, K, V> AllocFrozenValue for AllocDict<D>
where
    D: IntoIterator<Item = (K, V)>,
    K: AllocFrozenValue,
    V: AllocFrozenValue,
{
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        let iter = self.0.into_iter();
        let mut map = SmallMap::with_capacity(iter.size_hint().0);
        for (k, v) in iter {
            map.insert_hashed(
                k.alloc_frozen_value(heap).get_hashed().unwrap(),
                v.alloc_frozen_value(heap),
            );
        }
        heap.alloc(FrozenDictData { content: map })
    }
}
