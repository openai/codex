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

#![cfg(test)]

use allocative::Allocative;
use starlark_derive::starlark_value;
use starlark_derive::Freeze;
use starlark_derive::NoSerialize;
use starlark_derive::Trace;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FreezeResult;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLifetimeless;
use crate::values::ValueLike;

#[derive(
    Trace,
    Freeze,
    Debug,
    derive_more::Display,
    Allocative,
    ProvidesStaticType,
    NoSerialize
)]
#[display("TestComplexValue<{}>", _0)]
pub(crate) struct TestComplexValue<V: ValueLifetimeless>(pub(crate) V);

#[starlark_value(type = "TestComplexValue")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for TestComplexValue<V> where
    Self: ProvidesStaticType<'v>
{
}

impl<'v> AllocValue<'v> for TestComplexValue<Value<'v>> {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl AllocFrozenValue for TestComplexValue<FrozenValue> {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_simple(self)
    }
}

/// There's no anyhow API to print error without rust backtrace
/// ([issue](https://github.com/dtolnay/anyhow/issues/300)).
pub(crate) fn trim_rust_backtrace(error: &str) -> &str {
    match error.find("\nStack backtrace:") {
        Some(pos) => error[..pos].trim_end(),
        None => error.trim_end(),
    }
}
