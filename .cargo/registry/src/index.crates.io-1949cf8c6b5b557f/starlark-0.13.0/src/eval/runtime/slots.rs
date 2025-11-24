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

use allocative::Allocative;
use dupe::Dupe;
use starlark_derive::Trace;
use starlark_derive::VisitSpanMut;

use crate as starlark;
use crate::eval::bc::stack_ptr::BcSlot;
use crate::values::Freeze;
use crate::values::FreezeResult;

/// Not captured.
#[derive(
    Clone,
    Copy,
    Dupe,
    Debug,
    PartialEq,
    Eq,
    Trace,
    Freeze,
    VisitSpanMut,
    Allocative
)]
pub(crate) struct LocalSlotId(pub(crate) u32);

impl LocalSlotId {
    /// Each local slot is a valid BC slot.
    /// When it is:
    /// * known to be initialized
    /// * or used for writing
    /// * but not captured
    #[inline]
    pub(crate) fn to_bc_slot(self) -> BcSlot {
        BcSlot(self.0)
    }

    #[inline]
    pub(crate) fn to_captured_or_not(self) -> LocalSlotIdCapturedOrNot {
        LocalSlotIdCapturedOrNot(self.0)
    }
}

/// Captured local slot id.
///
/// E. g. in code:
///
/// ```python
/// def f():
///   x = 1
///   return lambda: x
/// ```
///
/// `x` slots (in both `f` and `lambda`) are captured.
#[derive(Clone, Copy, Dupe, Debug, PartialEq, Eq, Trace, VisitSpanMut)]
pub(crate) struct LocalCapturedSlotId(pub(crate) u32);

impl LocalCapturedSlotId {
    #[inline]
    pub(crate) fn to_bc_slot(self) -> BcSlot {
        BcSlot(self.0)
    }
}

/// Local slot id, when we don't know if it is captured or not.
///
/// This is used only during AST analysis.
#[derive(Clone, Copy, Dupe, Debug, PartialEq, Eq, Trace)]
pub(crate) struct LocalSlotIdCapturedOrNot(pub(crate) u32);
