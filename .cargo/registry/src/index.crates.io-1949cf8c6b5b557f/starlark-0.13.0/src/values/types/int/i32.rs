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

use std::any;

use crate::typing::Ty;
use crate::values::int::pointer_i32::PointerI32;
use crate::values::layout::value::IntegerTooBigError;
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

impl<'v> AllocValue<'v> for i32 {
    #[inline]
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc(StarlarkInt::from(self))
    }
}
impl AllocFrozenValue for i32 {
    #[inline]
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc(StarlarkInt::from(self))
    }
}

impl StarlarkTypeRepr for i32 {
    type Canonical = <StarlarkInt as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        PointerI32::starlark_type_repr()
    }
}

impl UnpackValue<'_> for i32 {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value) -> crate::Result<Option<Self>> {
        // Note this does not use `Value::unpack_integer()`
        // because we unlike other call sites,
        // we know that `i32` is `InlineInt` on 64-bit platforms and never `BigInt`,
        // so this is faster.
        if let Some(v) = value.unpack_i32() {
            Ok(Some(v))
        } else {
            if let Some(int) = StarlarkIntRef::unpack(value) {
                Err(crate::Error::new_value(IntegerTooBigError {
                    value: int.to_string(),
                    integer_type: any::type_name::<Self>(),
                }))
            } else {
                Ok(None)
            }
        }
    }
}
