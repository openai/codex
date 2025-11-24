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

//! Local slot id as value.
//!
//! To be able to propagate the local slot number through parameter binding machinery.

use std::array;

use allocative::Allocative;
use once_cell::sync::Lazy;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;

use crate as starlark;
use crate::eval::runtime::slots::LocalSlotId;
use crate::starlark_simple_value;
use crate::values::FrozenHeap;
use crate::values::FrozenHeapRef;
use crate::values::FrozenValueTyped;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;

/// Local slot id as `FrozenValue`. This object only using during compilation
/// and never appears in the executed program.
#[derive(
    derive_more::Display,
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
#[display("{:?}", self)]
pub(crate) struct LocalAsValue {
    #[allocative(skip)]
    pub(crate) local: LocalSlotId,
}

#[starlark_value(type = "LocalAsValue")]
impl<'v> StarlarkValue<'v> for LocalAsValue {}

starlark_simple_value!(LocalAsValue);

/// Create a value which represents a reference to local slot id during optimization.
pub(crate) fn local_as_value(
    local: LocalSlotId,
) -> Option<FrozenValueTyped<'static, LocalAsValue>> {
    // 100 is practically enough.
    static LOCALS: Lazy<(
        FrozenHeapRef,
        [FrozenValueTyped<'static, LocalAsValue>; 100],
    )> = Lazy::new(|| {
        let heap = FrozenHeap::new();
        let locals = array::from_fn(|i| {
            heap.alloc_simple_typed(LocalAsValue {
                local: LocalSlotId(i as u32),
            })
        });
        (heap.into_ref(), locals)
    });
    LOCALS.1.get(local.0 as usize).copied()
}
