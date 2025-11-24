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

use std::marker::PhantomData;

use allocative::Allocative;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;
use starlark_derive::ProvidesStaticType;

use crate as starlark;
use crate::typing::Ty;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::StarlarkValue;

enum NonInstantiable {}

/// `StarlarkTypeRepr` for iterable types.
pub struct StarlarkIter<T: StarlarkTypeRepr>(PhantomData<T>, NonInstantiable);

impl<T: StarlarkTypeRepr> StarlarkTypeRepr for StarlarkIter<T> {
    type Canonical = StarlarkIter<T::Canonical>;

    fn starlark_type_repr() -> Ty {
        Ty::iter(T::starlark_type_repr())
    }
}

#[derive(
    Debug,
    derive_more::Display,
    Allocative,
    ProvidesStaticType,
    NoSerialize
)]
#[display("{}", Self::TYPE)]
pub(crate) struct TypingIterable;

#[starlark_value(type = "typing.Iterable")]
impl<'v> StarlarkValue<'v> for TypingIterable {
    fn eval_type(&self) -> Option<Ty> {
        Some(Ty::iter(Ty::any()))
    }

    // TODO(nga): support `[]`.
}

impl AllocFrozenValue for TypingIterable {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        static ANY: AValueRepr<AValueImpl<'static, AValueBasic<TypingIterable>>> =
            alloc_static(TypingIterable);

        FrozenValue::new_repr(&ANY)
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_iterable_runtime() {
        assert::is_true("isinstance([1, 2, 3], typing.Iterable)");
        assert::is_true("isinstance((1, 2, 3), typing.Iterable)");
        assert::is_true("isinstance(range(10), typing.Iterable)");
        assert::is_false("isinstance('', typing.Iterable)");
        assert::is_false("isinstance(1, typing.Iterable)");
    }

    #[test]
    fn test_iterable_compile_time_pass() {
        assert::pass(
            r#"
def foo(x: typing.Iterable):
    pass

def bar():
    foo([1, 2, 3])
"#,
        );
    }

    #[test]
    fn test_iterable_compile_time_fail() {
        assert::fail(
            r#"
def foo(x: typing.Iterable):
    pass

def bar():
    foo(1)
"#,
            "Expected type `typing.Iterable`",
        );
    }
}
