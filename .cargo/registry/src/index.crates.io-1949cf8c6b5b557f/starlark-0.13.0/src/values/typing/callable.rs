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

pub(crate) mod param;

use std::convert::Infallible;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::sync::atomic::AtomicPtr;

use allocative::Allocative;
use dupe::Dupe;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;
use starlark_derive::ProvidesStaticType;

use crate as starlark;
use crate::private::Private;
use crate::typing::callable::TyCallable;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::list::UnpackList;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::typing::callable::param::StarlarkCallableParamAny;
use crate::values::typing::callable::param::StarlarkCallableParamSpec;
use crate::values::typing::TypeCompiled;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::UnpackValue;
use crate::values::Value;

#[derive(
    Debug,
    derive_more::Display,
    Allocative,
    ProvidesStaticType,
    NoSerialize
)]
#[display("{}", Self::TYPE)]
pub(crate) struct TypingCallable;

#[starlark_value(type = "typing.Callable")]
impl<'v> StarlarkValue<'v> for TypingCallable {
    fn eval_type(&self) -> Option<Ty> {
        Some(StarlarkCallable::<StarlarkCallableParamAny, FrozenValue>::starlark_type_repr())
    }

    fn at2(
        &self,
        param_types: Value<'v>,
        ret: Value<'v>,
        heap: &'v Heap,
        _private: Private,
    ) -> crate::Result<Value<'v>> {
        let param_types = UnpackList::<Value>::unpack_value_err(param_types)?;
        let ret = TypeCompiled::new(ret, heap)?.as_ty().dupe();
        let param_types: Vec<Ty> = param_types
            .items
            .into_iter()
            .map(|p| Ok(TypeCompiled::new(p, heap)?.as_ty().dupe()))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(heap.alloc_simple(TypingCallableAt2 {
            callable: TyCallable::new(ParamSpec::pos_only(param_types, []), ret),
        }))
    }
}

impl AllocFrozenValue for TypingCallable {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        static CALLABLE: AValueRepr<AValueImpl<'static, AValueBasic<TypingCallable>>> =
            alloc_static(TypingCallable);

        FrozenValue::new_repr(&CALLABLE)
    }
}

#[derive(
    Allocative,
    Debug,
    ProvidesStaticType,
    NoSerialize,
    derive_more::Display
)]
#[display("{}", callable)]
pub(crate) struct TypingCallableAt2 {
    callable: TyCallable,
}

#[starlark_value(type = "typing.Callable")]
impl<'v> StarlarkValue<'v> for TypingCallableAt2 {
    fn eval_type(&self) -> Option<Ty> {
        Some(Ty::basic(TyBasic::Callable(self.callable.dupe())))
    }
}

/// Marker for a callable value. Can be used in function signatures
/// for better documentation and type checking.
#[derive(Allocative)]
#[allocative(bound = "")]
pub struct StarlarkCallable<
    'v,
    P: StarlarkCallableParamSpec = StarlarkCallableParamAny,
    R: StarlarkTypeRepr = FrozenValue,
>(pub Value<'v>, PhantomData<PhantomData<AtomicPtr<(P, R)>>>);

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Copy for StarlarkCallable<'v, P, R> {}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Clone for StarlarkCallable<'v, P, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Dupe for StarlarkCallable<'v, P, R> {}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Debug for StarlarkCallable<'v, P, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StarlarkCallable").field(&self.0).finish()
    }
}

// TODO(nga): implement `#[trace(bound = "")]`.
unsafe impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Trace<'v>
    for StarlarkCallable<'v, P, R>
{
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.0.trace(tracer);
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> StarlarkCallable<'v, P, R> {
    /// Wrap the value.
    pub fn unchecked_new(value: Value<'v>) -> Self {
        StarlarkCallable(value, PhantomData)
    }

    /// Convert to `FrozenValue` version.
    pub fn unpack_frozen(self) -> Option<FrozenStarlarkCallable<P, R>> {
        self.0
            .unpack_frozen()
            .map(FrozenStarlarkCallable::unchecked_new)
    }

    /// Erase parameter and return types.
    pub fn erase(self) -> StarlarkCallable<'v> {
        StarlarkCallable::unchecked_new(self.0)
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> StarlarkTypeRepr
    for StarlarkCallable<'v, P, R>
{
    type Canonical = Self;

    fn starlark_type_repr() -> Ty {
        // TODO(nga): implement the same machinery for `typing.Callable`.
        Ty::callable(P::params(), R::starlark_type_repr())
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> UnpackValue<'v>
    for StarlarkCallable<'v, P, R>
{
    type Error = Infallible;

    #[inline]
    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        if value.vtable().starlark_value.HAS_invoke {
            Ok(Some(StarlarkCallable::unchecked_new(value)))
        } else {
            Ok(None)
        }
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> AllocValue<'v>
    for StarlarkCallable<'v, P, R>
{
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        self.0
    }
}

/// Marker for a callable value.
#[derive(Allocative)]
#[allocative(bound = "")]
pub struct FrozenStarlarkCallable<
    P: StarlarkCallableParamSpec = StarlarkCallableParamAny,
    R: StarlarkTypeRepr = FrozenValue,
>(pub FrozenValue, PhantomData<AtomicPtr<(P, R)>>);

impl<P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Debug for FrozenStarlarkCallable<P, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FrozenStarlarkCallable")
            .field(&self.0)
            .finish()
    }
}

fn _assert_sync_send() {
    fn _assert<T: Sync + Send>() {}
    // `Value` is not `Sync` nor `Send`, but `FrozenStarlarkCallable` should be.
    _assert::<FrozenStarlarkCallable<(Value,), Value>>();
}

impl<P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Copy for FrozenStarlarkCallable<P, R> {}

impl<P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Clone for FrozenStarlarkCallable<P, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Dupe for FrozenStarlarkCallable<P, R> {}

unsafe impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Trace<'v>
    for FrozenStarlarkCallable<P, R>
{
    fn trace(&mut self, tracer: &Tracer<'v>) {
        // TODO: implement `#[trace(bound = "")]`.
        self.0.trace(tracer);
    }
}

impl<P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> FrozenStarlarkCallable<P, R> {
    /// Wrap the value.
    pub fn unchecked_new(value: FrozenValue) -> Self {
        FrozenStarlarkCallable(value, PhantomData)
    }

    /// Erase parameter and return types.
    pub fn erase(self) -> FrozenStarlarkCallable {
        FrozenStarlarkCallable::unchecked_new(self.0)
    }
}

impl<P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> StarlarkTypeRepr
    for FrozenStarlarkCallable<P, R>
{
    type Canonical = <StarlarkCallable<'static, P, R> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        StarlarkCallable::<P, R>::starlark_type_repr()
    }
}

impl<P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> AllocFrozenValue
    for FrozenStarlarkCallable<P, R>
{
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        self.0
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Freeze for StarlarkCallable<'v, P, R> {
    type Frozen = FrozenStarlarkCallable<P, R>;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenStarlarkCallable::unchecked_new(
            self.0.freeze(freezer)?,
        ))
    }
}

impl<P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> FrozenStarlarkCallable<P, R> {
    /// Convert to `Value`-version.
    #[inline]
    pub fn to_callable<'v>(self) -> StarlarkCallable<'v, P, R> {
        StarlarkCallable::<P, R>::unchecked_new(self.0.to_value())
    }
}

/// More strict version of [`StarlarkCallable`].
///
/// This checks not only that the value is callable,
/// but also that it is a callable with the correct signature.
///
/// The implementation uses starlark-rust typechecker with all its limitations.
/// For example, if there are optional parameters in both value-def and this signature,
/// signature matching is ignored at the time of writing.
///
/// Unpacking with this type is expensive:
/// usually it is OK to use it for code executed once at top-level scope (like `rule()`),
/// but not for code executed many times (like `partial()`).
#[derive(Allocative)]
#[allocative(bound = "")]
pub struct StarlarkCallableChecked<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr>(
    pub Value<'v>,
    PhantomData<AtomicPtr<(P, R)>>,
);

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Clone
    for StarlarkCallableChecked<'v, P, R>
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Copy
    for StarlarkCallableChecked<'v, P, R>
{
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Dupe
    for StarlarkCallableChecked<'v, P, R>
{
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Debug
    for StarlarkCallableChecked<'v, P, R>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StarlarkCallableChecked")
            .field(&self.0)
            .finish()
    }
}

unsafe impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> Trace<'v>
    for StarlarkCallableChecked<'v, P, R>
{
    fn trace(&mut self, tracer: &Tracer<'v>) {
        let StarlarkCallableChecked(value, phantom) = self;
        value.trace(tracer);
        phantom.trace(tracer);
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> AllocValue<'v>
    for StarlarkCallableChecked<'v, P, R>
{
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        self.0
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> StarlarkCallableChecked<'v, P, R> {
    /// Convert to [`StarlarkCallable`].
    pub fn to_unchecked(self) -> StarlarkCallable<'v, P, R> {
        StarlarkCallable::unchecked_new(self.0)
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> StarlarkTypeRepr
    for StarlarkCallableChecked<'v, P, R>
{
    type Canonical = <StarlarkCallable<'v, P, R> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        <Self::Canonical as StarlarkTypeRepr>::starlark_type_repr()
    }
}

impl<'v, P: StarlarkCallableParamSpec, R: StarlarkTypeRepr> UnpackValue<'v>
    for StarlarkCallableChecked<'v, P, R>
{
    /// Only internal error is possible.
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        // Check it is a callable first.
        if StarlarkCallable::<P, R>::unpack_value_opt(value).is_none() {
            return Ok(None);
        }

        // We need generic statics to cache this.
        let ty = Ty::callable(P::params(), R::starlark_type_repr());

        match Ty::of_value(value).check_intersects(&ty)? {
            true => Ok(Some(StarlarkCallableChecked(value, PhantomData))),
            false => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use starlark_derive::starlark_module;

    use crate as starlark;
    use crate::assert;
    use crate::assert::Assert;
    use crate::environment::GlobalsBuilder;
    use crate::values::none::NoneType;
    use crate::values::typing::StarlarkCallable;
    use crate::values::typing::StarlarkCallableChecked;

    #[test]
    fn test_callable_runtime() {
        assert::is_true("isinstance(lambda: None, typing.Callable)");
        assert::is_true("isinstance(len, typing.Callable)");
        assert::is_true("Rec = record(); isinstance(Rec, typing.Callable)");
        assert::is_false("isinstance(37, typing.Callable)");
    }

    #[test]
    fn test_callable_pass_compile_time() {
        assert::pass(
            r#"
Rec = record()

def foo(x: typing.Callable):
    pass

def bar():
    foo(len)
    foo(lambda x: 1)
    foo(Rec)
"#,
        );
    }

    #[test]
    fn test_callable_fail_compile_time() {
        assert::fail(
            r#"
def foo(x: typing.Callable):
    pass

def bar():
    foo(1)
"#,
            "Expected type",
        );
    }

    #[starlark_module]
    fn my_module(globals: &mut GlobalsBuilder) {
        fn accept_f(
            #[starlark(require=pos)] _x: StarlarkCallable<(String,), i32>,
        ) -> anyhow::Result<NoneType> {
            Ok(NoneType)
        }
    }

    #[test]
    fn test_native_callable_pass() {
        let mut a = Assert::new();
        a.globals_add(my_module);
        a.pass(
            r#"
def f(x: str) -> int:
    return len(x)

def test():
    accept_f(f)
"#,
        );
    }

    #[test]
    fn test_native_callable_fail_compile_time_wrong_param_type() {
        let mut a = Assert::new();
        a.globals_add(my_module);
        a.fail(
            r#"
def f(x: list) -> int:
    return 1

def test():
    accept_f(f)
"#,
            "Expected type `typing.Callable[[str], int]` but got",
        );
    }

    #[test]
    fn test_native_callable_fail_compile_time_wrong_param_count() {
        let mut a = Assert::new();
        a.globals_add(my_module);
        a.fail(
            r#"
def f() -> int:
    return 1

def test():
    accept_f(f)
"#,
            "Expected type `typing.Callable[[str], int]` but got",
        );
    }

    #[test]
    fn test_typing_callable_pass() {
        let a = Assert::new();
        a.pass(
            r#"
def accept_f(x: typing.Callable[[str], int]) -> None:
    pass

def f(x: str) -> int:
    return len(x)

def test():
    accept_f(f)
"#,
        );
    }

    #[test]
    fn test_typing_callable_fail_compile_time_wrong_param_type() {
        let a = Assert::new();
        a.fail(
            r#"
def accept_f(x: typing.Callable[[str], int]) -> None:
    pass

def f(x: list) -> int:
    return 1

def test():
    accept_f(f)
"#,
            "Expected type `typing.Callable[[str], int]` but got",
        );
    }

    #[test]
    fn test_typing_callable_fail_compile_time_wrong_param_count() {
        let a = Assert::new();
        a.fail(
            r#"
def accept_f(x: typing.Callable[[str], int]) -> None:
    pass

def f() -> int:
    return 1

def test():
    accept_f(f)
"#,
            "Expected type `typing.Callable[[str], int]` but got",
        );
    }

    #[test]
    fn test_callable_checked_runtime() {
        #[starlark_module]
        fn module(globals: &mut GlobalsBuilder) {
            fn accept_f(
                #[starlark(require=pos)] _f: StarlarkCallableChecked<(), NoneType>,
            ) -> anyhow::Result<NoneType> {
                Ok(NoneType)
            }

            fn good() -> anyhow::Result<NoneType> {
                Ok(NoneType)
            }

            fn bad() -> anyhow::Result<i32> {
                Ok(10)
            }
        }

        let mut a = Assert::new();
        a.globals_add(module);

        a.pass("accept_f(good)");

        a.fail(
            r#"
def test():
    x = noop(bad) # Hide the type from static typechecker.
    accept_f(x)

test()
        "#,
            "Type of parameter `_f` doesn't match",
        );
    }
}
