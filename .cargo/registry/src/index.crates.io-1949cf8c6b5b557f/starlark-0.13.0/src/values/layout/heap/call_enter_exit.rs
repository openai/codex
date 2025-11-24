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

//! Marker objects to track allocations.

use std::fmt::Debug;

use allocative::Allocative;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::eval::runtime::profile::instant::ProfilerInstant;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Value;

/// A type which is either drop or non-drop.
pub(crate) trait MaybeDrop: Debug + Sync + Send + Allocative + 'static {}

/// Type which has `Drop`.
#[derive(ProvidesStaticType, Debug, Trace, Allocative)]
pub(crate) struct NeedsDrop;
impl Drop for NeedsDrop {
    fn drop(&mut self) {
        // Just make this type `Drop`.
        // Note `mem::needs_drop()` would return `true` for this type,
        // even if `drop` is optimized away: https://rust.godbolt.org/z/1cxKoMzdM
    }
}

/// Type which doesn't have `Drop`.
#[derive(ProvidesStaticType, Debug, Trace, Allocative)]
pub(crate) struct NoDrop;

impl MaybeDrop for NeedsDrop {}
impl MaybeDrop for NoDrop {}

#[derive(
    Trace,
    Debug,
    derive_more::Display,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
#[display("CallEnter")]
pub(crate) struct CallEnter<'v, D: MaybeDrop + 'static> {
    pub(crate) function: Value<'v>,
    pub(crate) time: ProfilerInstant,
    pub(crate) maybe_drop: D,
}

#[starlark_value(type = "call_enter")]
impl<'v, D: MaybeDrop + Trace<'v> + 'v> StarlarkValue<'v> for CallEnter<'v, D> {
    type Canonical = Self;
}

#[derive(
    Debug,
    derive_more::Display,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
#[display("CallExit")]
pub(crate) struct CallExit<D: MaybeDrop + 'static> {
    pub(crate) time: ProfilerInstant,
    pub(crate) maybe_drop: D,
}

#[starlark_value(type = "call_exit")]
impl<'v, D: MaybeDrop> StarlarkValue<'v> for CallExit<D> {
    type Canonical = Self;
}
