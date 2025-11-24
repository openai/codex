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
use starlark_derive::NoSerialize;
use starlark_derive::ProvidesStaticType;

use crate as starlark;
use crate::typing::Ty;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::starlark_value;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::StarlarkValue;

#[derive(
    Debug,
    derive_more::Display,
    Allocative,
    ProvidesStaticType,
    NoSerialize
)]
#[display("{}", Self::TYPE)]
pub(crate) struct TypingNever;

#[starlark_value(type = "typing.Never")]
impl<'v> StarlarkValue<'v> for TypingNever {
    fn eval_type(&self) -> Option<Ty> {
        Some(Ty::never())
    }
}

impl AllocFrozenValue for TypingNever {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        static NEVER: AValueRepr<AValueImpl<'static, AValueBasic<TypingNever>>> =
            alloc_static(TypingNever);

        FrozenValue::new_repr(&NEVER)
    }
}

/// Never type, can be used as native function return type.
pub enum StarlarkNever {}

impl StarlarkTypeRepr for StarlarkNever {
    type Canonical = Self;

    fn starlark_type_repr() -> Ty {
        Ty::never()
    }
}

impl<'v> AllocValue<'v> for StarlarkNever {
    fn alloc_value(self, _heap: &'v crate::values::Heap) -> crate::values::Value<'v> {
        match self {}
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_never_runtime() {
        assert::is_true("not isinstance(1, typing.Never)");
    }

    #[test]
    fn test_never_compile_time() {
        assert::pass(
            r#"
def f() -> typing.Never:
    return fail()
"#,
        );
    }
}
