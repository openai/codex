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
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;
use starlark_derive::ProvidesStaticType;

use crate as starlark;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::AllocFrozenValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::StarlarkValue;

#[derive(
    Allocative,
    NoSerialize,
    Debug,
    derive_more::Display,
    ProvidesStaticType
)]
#[display("Ellipsis")]
pub(crate) struct Ellipsis;

pub(crate) static VALUE_ELLIPSIS: AValueRepr<AValueImpl<'static, AValueBasic<Ellipsis>>> =
    alloc_static(Ellipsis);

#[starlark_value(type = "ellipsis")]
impl<'v> StarlarkValue<'v> for Ellipsis {}

impl Ellipsis {
    pub(crate) fn new_value() -> FrozenValue {
        FrozenValue::new_repr(&VALUE_ELLIPSIS)
    }
}

impl AllocFrozenValue for Ellipsis {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        Ellipsis::new_value()
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_ellipsis() {
        assert::pass("...");
    }
}
