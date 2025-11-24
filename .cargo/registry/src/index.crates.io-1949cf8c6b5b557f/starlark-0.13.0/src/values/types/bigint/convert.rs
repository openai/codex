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

use num_bigint::BigInt;

use crate::typing::Ty;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::int::int_or_big::StarlarkInt;
use crate::values::types::int::int_or_big::StarlarkIntRef;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;

impl StarlarkTypeRepr for u32 {
    type Canonical = <i32 as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        i32::starlark_type_repr()
    }
}

impl<'v> AllocValue<'v> for u32 {
    #[inline]
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl AllocFrozenValue for u32 {
    #[inline]
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl StarlarkTypeRepr for u64 {
    type Canonical = <i32 as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        i32::starlark_type_repr()
    }
}

impl<'v> AllocValue<'v> for u64 {
    #[inline]
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl AllocFrozenValue for u64 {
    #[inline]
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl StarlarkTypeRepr for i64 {
    type Canonical = <i32 as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        i32::starlark_type_repr()
    }
}

impl<'v> AllocValue<'v> for i64 {
    #[inline]
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl AllocFrozenValue for i64 {
    #[inline]
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl StarlarkTypeRepr for usize {
    type Canonical = <i32 as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        i32::starlark_type_repr()
    }
}

impl<'v> AllocValue<'v> for usize {
    #[inline]
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl AllocFrozenValue for usize {
    #[inline]
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl StarlarkTypeRepr for isize {
    type Canonical = <i32 as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        i32::starlark_type_repr()
    }
}

impl<'v> AllocValue<'v> for isize {
    #[inline]
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl AllocFrozenValue for isize {
    #[inline]
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl StarlarkTypeRepr for BigInt {
    type Canonical = <i32 as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        i32::starlark_type_repr()
    }
}

impl<'v> AllocValue<'v> for BigInt {
    #[inline]
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl AllocFrozenValue for BigInt {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl<'v> UnpackValue<'v> for u32 {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<u32>> {
        value.unpack_integer()
    }
}

impl<'v> UnpackValue<'v> for u64 {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<u64>> {
        value.unpack_integer()
    }
}

impl<'v> UnpackValue<'v> for i64 {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<i64>> {
        value.unpack_integer()
    }
}

impl<'v> UnpackValue<'v> for usize {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<usize>> {
        value.unpack_integer()
    }
}

impl<'v> UnpackValue<'v> for isize {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<isize>> {
        value.unpack_integer()
    }
}

impl<'v> UnpackValue<'v> for BigInt {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<BigInt>> {
        let Some(int) = StarlarkIntRef::unpack_value_opt(value) else {
            return Ok(None);
        };
        Ok(match int {
            StarlarkIntRef::Small(x) => Some(BigInt::from(x.to_i32())),
            StarlarkIntRef::Big(x) => Some(x.get().to_owned()),
        })
    }
}

#[cfg(test)]
mod tests {
    use starlark_derive::starlark_module;

    use crate as starlark;
    use crate::assert::Assert;
    use crate::environment::GlobalsBuilder;
    use crate::values::none::NoneType;

    #[test]
    fn test_unpack_int_error() {
        #[starlark_module]
        fn module(globals: &mut GlobalsBuilder) {
            fn takes_i32(#[starlark(require=pos)] _i: i32) -> starlark::Result<NoneType> {
                Ok(NoneType)
            }

            fn takes_i64(#[starlark(require=pos)] _i: i64) -> starlark::Result<NoneType> {
                Ok(NoneType)
            }
        }

        let mut a = Assert::new();
        a.globals_add(module);
        a.fails(
            "takes_i32(1 << 100)",
            &[
                "Integer value is too big to fit in i32: 1267650600228229401496703205376",
                "Error unpacking value for parameter `_i`",
            ],
        );
        a.fails(
            "takes_i64(1 << 100)",
            &[
                "Integer value is too big to fit in i64: 1267650600228229401496703205376",
                "Error unpacking value for parameter `_i`",
            ],
        );
    }
}
