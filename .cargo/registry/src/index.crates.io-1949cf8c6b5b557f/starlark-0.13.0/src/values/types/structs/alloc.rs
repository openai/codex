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
use crate::values::alloc_value::AllocFrozenStringValue;
use crate::values::alloc_value::AllocStringValue;
use crate::values::structs::value::FrozenStruct;
use crate::values::structs::value::Struct;
use crate::values::structs::StructRef;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;

/// Utility to allocate a struct on a heap.
///
/// # Panics
///
/// Panics if:
/// * keys are not strings
/// * keys are not unique
///
/// # Example
///
/// ```
/// use starlark::values::structs::AllocStruct;
///
/// # use starlark::values::{FrozenHeap, Heap};
/// # fn alloc(heap: &Heap, frozen_heap: &FrozenHeap) {
/// let s = heap.alloc(AllocStruct([("a", 1), ("b", 2)]));
/// let fs = frozen_heap.alloc(AllocStruct([("a", 1), ("b", 2)]));
/// # }
pub struct AllocStruct<S>(pub S);

impl AllocStruct<iter::Empty<(String, String)>> {
    /// Allocate an empty struct.
    pub const EMPTY: AllocStruct<iter::Empty<(String, String)>> = AllocStruct(iter::empty());
}

impl<K, V, S> StarlarkTypeRepr for AllocStruct<S>
where
    S: IntoIterator<Item = (K, V)>,
    V: StarlarkTypeRepr,
{
    type Canonical = <StructRef<'static> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Struct::starlark_type_repr()
    }
}

impl<'v, K, V, S> AllocValue<'v> for AllocStruct<S>
where
    S: IntoIterator<Item = (K, V)>,
    K: AllocStringValue<'v>,
    V: AllocValue<'v>,
{
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        let iter = self.0.into_iter();
        let mut fields = SmallMap::with_capacity(iter.size_hint().0);
        for (k, v) in iter {
            let k = k.alloc_string_value(heap);
            let v = v.alloc_value(heap);
            let prev = fields.insert(k, v);
            assert!(prev.is_none(), "non-unique key: {}", k);
        }
        heap.alloc(Struct::new(fields))
    }
}

impl<K, V, S> AllocFrozenValue for AllocStruct<S>
where
    S: IntoIterator<Item = (K, V)>,
    K: AllocFrozenStringValue,
    V: AllocFrozenValue,
{
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        let iter = self.0.into_iter();
        let mut fields = SmallMap::with_capacity(iter.size_hint().0);
        for (k, v) in iter {
            let k = k.alloc_frozen_string_value(heap);
            let v = v.alloc_frozen_value(heap);
            let prev = fields.insert(k, v);
            assert!(prev.is_none(), "non-unique key: {}", k);
        }
        heap.alloc(FrozenStruct::new(fields))
    }
}
