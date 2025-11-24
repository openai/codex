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

//! Implementations of alloc and unpack traits for string.

use std::convert::Infallible;

use crate::typing::Ty;
use crate::values::alloc_value::AllocFrozenStringValue;
use crate::values::alloc_value::AllocStringValue;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::UnpackValue;
use crate::values::Value;

impl AllocFrozenValue for String {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        self.alloc_frozen_string_value(heap).to_frozen_value()
    }
}

impl AllocFrozenStringValue for String {
    fn alloc_frozen_string_value(self, heap: &FrozenHeap) -> FrozenStringValue {
        heap.alloc_str(self.as_str())
    }
}

impl<'a> AllocFrozenValue for &'a str {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        self.alloc_frozen_string_value(heap).to_frozen_value()
    }
}

impl<'a> AllocFrozenStringValue for &'a str {
    fn alloc_frozen_string_value(self, heap: &FrozenHeap) -> FrozenStringValue {
        heap.alloc_str(self)
    }
}

impl<'v> AllocValue<'v> for String {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        self.alloc_string_value(heap).to_value()
    }
}

impl<'v> AllocStringValue<'v> for String {
    fn alloc_string_value(self, heap: &'v Heap) -> StringValue<'v> {
        heap.alloc_str(self.as_str())
    }
}

impl StarlarkTypeRepr for char {
    type Canonical = <String as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        String::starlark_type_repr()
    }
}

impl<'v> AllocValue<'v> for char {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        self.alloc_string_value(heap).to_value()
    }
}

impl<'v> AllocStringValue<'v> for char {
    fn alloc_string_value(self, heap: &'v Heap) -> StringValue<'v> {
        heap.alloc_char(self)
    }
}

impl StarlarkTypeRepr for &'_ String {
    type Canonical = <String as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        String::starlark_type_repr()
    }
}

impl<'v> AllocValue<'v> for &'_ String {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        self.alloc_string_value(heap).to_value()
    }
}

impl<'v> AllocStringValue<'v> for &'_ String {
    fn alloc_string_value(self, heap: &'v Heap) -> StringValue<'v> {
        heap.alloc_str(self.as_str())
    }
}

impl<'v> AllocValue<'v> for &'_ str {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        self.alloc_string_value(heap).to_value()
    }
}

impl<'v> AllocStringValue<'v> for &'_ str {
    fn alloc_string_value(self, heap: &'v Heap) -> StringValue<'v> {
        heap.alloc_str(self)
    }
}

impl<'v> UnpackValue<'v> for &'v str {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(value.unpack_str())
    }
}

impl<'v> UnpackValue<'v> for String {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(value.unpack_str().map(ToOwned::to_owned))
    }
}
