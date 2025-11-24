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

//! Bindings to/from Rust tuple types.

use either::Either;

use crate::typing::Ty;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::tuple::value::Tuple;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;

impl<'v, T1: AllocValue<'v>> AllocValue<'v> for (T1,) {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_tuple(&[self.0.alloc_value(heap)])
    }
}

impl<'v, T1: AllocValue<'v>, T2: AllocValue<'v>> AllocValue<'v> for (T1, T2) {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_tuple(&[self.0.alloc_value(heap), self.1.alloc_value(heap)])
    }
}

impl<'v, T1: AllocValue<'v>, T2: AllocValue<'v>, T3: AllocValue<'v>> AllocValue<'v>
    for (T1, T2, T3)
{
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_tuple(&[
            self.0.alloc_value(heap),
            self.1.alloc_value(heap),
            self.2.alloc_value(heap),
        ])
    }
}

impl<T1: AllocFrozenValue> AllocFrozenValue for (T1,) {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_tuple(&[self.0.alloc_frozen_value(heap)])
    }
}

impl<T1: AllocFrozenValue, T2: AllocFrozenValue> AllocFrozenValue for (T1, T2) {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_tuple(&[
            self.0.alloc_frozen_value(heap),
            self.1.alloc_frozen_value(heap),
        ])
    }
}

impl<T1: AllocFrozenValue, T2: AllocFrozenValue, T3: AllocFrozenValue> AllocFrozenValue
    for (T1, T2, T3)
{
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_tuple(&[
            self.0.alloc_frozen_value(heap),
            self.1.alloc_frozen_value(heap),
            self.2.alloc_frozen_value(heap),
        ])
    }
}

impl<T1: StarlarkTypeRepr, T2: StarlarkTypeRepr> StarlarkTypeRepr for (T1, T2) {
    type Canonical = (T1::Canonical, T2::Canonical);

    fn starlark_type_repr() -> Ty {
        Ty::tuple2(T1::starlark_type_repr(), T2::starlark_type_repr())
    }
}

impl<T1: StarlarkTypeRepr> StarlarkTypeRepr for (T1,) {
    type Canonical = (T1::Canonical,);

    fn starlark_type_repr() -> Ty {
        Ty::tuple(vec![T1::starlark_type_repr()])
    }
}

impl<T1: StarlarkTypeRepr, T2: StarlarkTypeRepr, T3: StarlarkTypeRepr> StarlarkTypeRepr
    for (T1, T2, T3)
{
    type Canonical = (T1::Canonical, T2::Canonical, T3::Canonical);

    fn starlark_type_repr() -> Ty {
        Ty::tuple(vec![
            T1::starlark_type_repr(),
            T2::starlark_type_repr(),
            T3::starlark_type_repr(),
        ])
    }
}

impl<'v, T1: UnpackValue<'v>, T2: UnpackValue<'v>> UnpackValue<'v> for (T1, T2) {
    type Error = Either<T1::Error, T2::Error>;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(t) = Tuple::from_value(value) else {
            return Ok(None);
        };
        let [a, b] = t.content() else {
            return Ok(None);
        };
        let [a, b] = [*a, *b];
        let Some(a) = T1::unpack_value_impl(a).map_err(Either::Left)? else {
            return Ok(None);
        };
        let Some(b) = T2::unpack_value_impl(b).map_err(Either::Right)? else {
            return Ok(None);
        };
        Ok(Some((a, b)))
    }
}

impl<'v, T1: UnpackValue<'v>, T2: UnpackValue<'v>, T3: UnpackValue<'v>> UnpackValue<'v>
    for (T1, T2, T3)
{
    type Error = Either<T1::Error, Either<T2::Error, T3::Error>>;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(t) = Tuple::from_value(value) else {
            return Ok(None);
        };
        let [a, b, c] = t.content() else {
            return Ok(None);
        };
        let [a, b, c] = [*a, *b, *c];
        let Some(a) = T1::unpack_value_impl(a).map_err(Either::Left)? else {
            return Ok(None);
        };
        let Some(b) = T2::unpack_value_impl(b)
            .map_err(Either::Left)
            .map_err(Either::Right)?
        else {
            return Ok(None);
        };
        let Some(c) = T3::unpack_value_impl(c)
            .map_err(Either::Right)
            .map_err(Either::Right)?
        else {
            return Ok(None);
        };
        Ok(Some((a, b, c)))
    }
}
