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

use std::collections::BTreeMap;
use std::hash::Hash;

use either::Either;

use crate::collections::SmallMap;
use crate::typing::Ty;
use crate::values::dict::AllocDict;
use crate::values::dict::DictRef;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::dict::dict_type::DictType;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;

// SmallMap

impl<'v, K: AllocValue<'v>, V: AllocValue<'v>> AllocValue<'v> for SmallMap<K, V> {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        AllocDict(self).alloc_value(heap)
    }
}

impl<K: AllocFrozenValue, V: AllocFrozenValue> AllocFrozenValue for SmallMap<K, V> {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        AllocDict(self).alloc_frozen_value(heap)
    }
}

impl<'a, 'v, K: 'a + StarlarkTypeRepr, V: 'a + StarlarkTypeRepr> AllocValue<'v>
    for &'a SmallMap<K, V>
where
    &'a K: AllocValue<'v>,
    &'a V: AllocValue<'v>,
{
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        AllocDict(self).alloc_value(heap)
    }
}

impl<'a, K: 'a + StarlarkTypeRepr, V: 'a + StarlarkTypeRepr> AllocFrozenValue for &'a SmallMap<K, V>
where
    &'a K: AllocFrozenValue,
    &'a V: AllocFrozenValue,
{
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        AllocDict(self).alloc_frozen_value(heap)
    }
}

impl<'a, K: StarlarkTypeRepr, V: StarlarkTypeRepr> StarlarkTypeRepr for &'a SmallMap<K, V> {
    type Canonical = <SmallMap<K, V> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        DictType::<K, V>::starlark_type_repr()
    }
}

impl<K: StarlarkTypeRepr, V: StarlarkTypeRepr> StarlarkTypeRepr for SmallMap<K, V> {
    type Canonical = <DictType<K, V> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        DictType::<K, V>::starlark_type_repr()
    }
}

impl<'v, K: UnpackValue<'v> + Hash + Eq, V: UnpackValue<'v>> UnpackValue<'v> for SmallMap<K, V> {
    type Error = Either<K::Error, V::Error>;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(dict) = DictRef::from_value(value) else {
            return Ok(None);
        };
        let it = dict.iter();
        let mut r = SmallMap::with_capacity(it.len());
        for (k, v) in it {
            let Some(k) = K::unpack_value_impl(k).map_err(Either::Left)? else {
                return Ok(None);
            };
            let Some(v) = V::unpack_value_impl(v).map_err(Either::Right)? else {
                return Ok(None);
            };
            // TODO(nga): return error if keys are not unique.
            r.insert(k, v);
        }
        Ok(Some(r))
    }
}

// BTreeMap

impl<'v, K: AllocValue<'v>, V: AllocValue<'v>> AllocValue<'v> for BTreeMap<K, V> {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        AllocDict(self).alloc_value(heap)
    }
}

impl<K: AllocFrozenValue, V: AllocFrozenValue> AllocFrozenValue for BTreeMap<K, V> {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        AllocDict(self).alloc_frozen_value(heap)
    }
}

impl<'a, 'v, K: 'a + StarlarkTypeRepr, V: 'a + StarlarkTypeRepr> AllocValue<'v>
    for &'a BTreeMap<K, V>
where
    &'a K: AllocValue<'v>,
    &'a V: AllocValue<'v>,
{
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        AllocDict(self).alloc_value(heap)
    }
}

impl<'a, K: 'a + StarlarkTypeRepr, V: 'a + StarlarkTypeRepr> AllocFrozenValue for &'a BTreeMap<K, V>
where
    &'a K: AllocFrozenValue,
    &'a V: AllocFrozenValue,
{
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        AllocDict(self).alloc_frozen_value(heap)
    }
}

impl<'a, K: StarlarkTypeRepr, V: StarlarkTypeRepr> StarlarkTypeRepr for &'a BTreeMap<K, V> {
    type Canonical = <BTreeMap<K, V> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        DictType::<K, V>::starlark_type_repr()
    }
}

impl<K: StarlarkTypeRepr, V: StarlarkTypeRepr> StarlarkTypeRepr for BTreeMap<K, V> {
    type Canonical = <DictType<K, V> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        DictType::<K, V>::starlark_type_repr()
    }
}

impl<'v, K: UnpackValue<'v> + Ord, V: UnpackValue<'v>> UnpackValue<'v> for BTreeMap<K, V> {
    type Error = Either<K::Error, V::Error>;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(dict) = DictRef::from_value(value) else {
            return Ok(None);
        };
        let mut r = BTreeMap::new();
        for (k, v) in dict.iter() {
            let Some(k) = K::unpack_value_impl(k).map_err(Either::Left)? else {
                return Ok(None);
            };
            let Some(v) = V::unpack_value_impl(v).map_err(Either::Right)? else {
                return Ok(None);
            };
            // TODO(nga): return error if keys are not unique.
            r.insert(k, v);
        }
        Ok(Some(r))
    }
}
