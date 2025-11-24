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

//! Function types, including native functions and `object.member` functions.

use allocative::Allocative;
use derivative::Derivative;
use derive_more::Display;
use dupe::Dupe;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::Coerce;
use crate::docs::DocItem;
use crate::docs::DocMember;
use crate::docs::DocProperty;
use crate::docs::DocString;
use crate::docs::DocStringKind;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::private::Private;
use crate::starlark_complex_value;
use crate::starlark_simple_value;
use crate::typing::arc_ty::ArcTy;
use crate::typing::tuple::TyTuple;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::ellipsis::Ellipsis;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLifetimeless;
use crate::values::ValueLike;

#[derive(Debug, thiserror::Error)]
enum FunctionError {
    #[error("`tuple[]` is implemented only for `tuple[T, ...]`")]
    TupleOnlyEllipsis,
}

/// Return value of `type(any function)`.
pub const FUNCTION_TYPE: &str = "function";

/// Marker trait for function types.
pub(crate) enum StarlarkFunction {}

impl StarlarkTypeRepr for StarlarkFunction {
    type Canonical = Self;

    fn starlark_type_repr() -> Ty {
        Ty::any_callable()
    }
}

#[derive(Debug, Allocative, Clone, Copy, Dupe)]
#[doc(hidden)]
pub enum SpecialBuiltinFunction {
    List,
    Dict,
    Tuple,
    Set,
}

/// A native function that can be evaluated.
///
/// This trait is implemented by generated code and rarely needed to be implemented manually.
pub trait NativeFunc: Send + Sync + 'static {
    /// Invoke the function.
    fn invoke<'v>(
        &self,
        eval: &mut Evaluator<'v, '_, '_>,
        args: &Arguments<'v, '_>,
    ) -> crate::Result<Value<'v>>;
}

/// Native method implementation.
///
/// This trait is implemented by generated code and rarely needed to be implemented manually.
pub trait NativeMeth: Send + Sync + 'static {
    /// Invoke the method.
    fn invoke<'v>(
        &self,
        eval: &mut Evaluator<'v, '_, '_>,
        this: Value<'v>,
        args: &Arguments<'v, '_>,
    ) -> crate::Result<Value<'v>>;
}

/// A native function that can be evaluated.
pub trait NativeAttr:
    for<'v> Fn(Value<'v>, &'v Heap) -> crate::Result<Value<'v>> + Send + Sync + 'static
{
}

impl<T> NativeAttr for T where
    T: for<'v> Fn(Value<'v>, &'v Heap) -> crate::Result<Value<'v>> + Send + Sync + 'static
{
}

/// Starlark representation of native (Rust) functions.
///
/// Almost always created with [`#[starlark_module]`](macro@crate::starlark_module).
#[derive(Derivative, ProvidesStaticType, Display, NoSerialize, Allocative)]
#[derivative(Debug)]
#[display("{}", name)]
pub(crate) struct NativeFunction {
    #[derivative(Debug = "ignore")]
    #[allocative(skip)]
    pub(crate) function: Box<dyn NativeFunc>,
    pub(crate) name: String,
    /// `.type` attribute and a type when this function is used in type expression.
    pub(crate) as_type: Option<Ty>,
    pub(crate) ty: Ty,
    /// Safe to evaluate speculatively.
    pub(crate) speculative_exec_safe: bool,
    #[derivative(Debug = "ignore")]
    pub(crate) docs: DocItem,
    pub(crate) special_builtin_function: Option<SpecialBuiltinFunction>,
}

impl AllocFrozenValue for NativeFunction {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_simple(self)
    }
}

impl<'v> AllocValue<'v> for NativeFunction {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_simple(self)
    }
}

/// Define the function type
#[starlark_value(type = FUNCTION_TYPE)]
impl<'v> StarlarkValue<'v> for NativeFunction {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        self.function.invoke(eval, args).map_err(Into::into)
    }

    fn get_attr(&self, attribute: &str, heap: &'v Heap) -> Option<Value<'v>> {
        if let Some(s) = self.as_type.as_ref().and_then(|t| t.as_name()) {
            if attribute == "type" {
                return Some(heap.alloc(s));
            }
        }
        None
    }

    fn eval_type(&self) -> Option<Ty> {
        self.as_type.clone()
    }

    fn has_attr(&self, _attribute: &str, _heap: &'v Heap) -> bool {
        // TODO(nga): implement properly.
        false
    }

    fn dir_attr(&self) -> Vec<String> {
        if self.as_type.is_some() {
            vec!["type".to_owned()]
        } else {
            Vec::new()
        }
    }

    fn documentation(&self) -> DocItem {
        self.docs.clone()
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(self.ty.dupe())
    }

    fn at(&self, index: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match &self.special_builtin_function {
            Some(SpecialBuiltinFunction::List) => {
                let index = TypeCompiled::new(index, heap)?;
                Ok(TypeCompiled::type_list_of(index, heap).to_inner())
            }
            Some(SpecialBuiltinFunction::Set) => {
                let index = TypeCompiled::new(index, heap)?;
                Ok(TypeCompiled::type_set_of(index, heap).to_inner())
            }
            _ => ValueError::unsupported(self, "[]"),
        }
    }

    fn at2(
        &self,
        index0: Value<'v>,
        index1: Value<'v>,
        heap: &'v Heap,
        _private: Private,
    ) -> crate::Result<Value<'v>> {
        match &self.special_builtin_function {
            Some(SpecialBuiltinFunction::Dict) => {
                let index0 = TypeCompiled::new(index0, heap)?;
                let index1 = TypeCompiled::new(index1, heap)?;
                Ok(TypeCompiled::type_dict_of(index0, index1, heap).to_inner())
            }
            Some(SpecialBuiltinFunction::Tuple) => {
                let item = TypeCompiled::new(index0, heap)?;
                if index1.downcast_ref::<Ellipsis>().is_some() {
                    Ok(TypeCompiled::from_ty(
                        &Ty::basic(TyBasic::Tuple(TyTuple::Of(ArcTy::new(
                            item.as_ty().clone(),
                        )))),
                        heap,
                    )
                    .to_inner())
                } else {
                    Err(crate::Error::new_other(FunctionError::TupleOnlyEllipsis))
                }
            }
            _ => ValueError::unsupported(self, "[,]"),
        }
    }
}

#[derive(Derivative, Display, NoSerialize, ProvidesStaticType, Allocative)]
#[derivative(Debug)]
#[display("{}", name)]
pub(crate) struct NativeMethod {
    #[derivative(Debug = "ignore")]
    #[allocative(skip)]
    pub(crate) function: FrozenRef<'static, dyn NativeMeth>,
    pub(crate) name: String,
    pub(crate) ty: Ty,
    /// Safe to evaluate speculatively.
    pub(crate) speculative_exec_safe: bool,
    #[derivative(Debug = "ignore")]
    pub(crate) docs: DocItem,
}

starlark_simple_value!(NativeMethod);

#[starlark_value(type = "native_method")]
impl<'v> StarlarkValue<'v> for NativeMethod {
    fn documentation(&self) -> DocItem {
        self.docs.clone()
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(self.ty.clone())
    }
}

/// Used by the `#[starlark(attribute)]` tag of [`#[starlark_module]`](macro@starlark_module)
/// to define a function that pretends to be an attribute.
#[derive(Derivative, Display, NoSerialize, ProvidesStaticType, Allocative)]
#[display("Attribute")]
#[derivative(Debug)]
pub(crate) struct NativeAttribute {
    /// Safe to evaluate speculatively.
    pub(crate) speculative_exec_safe: bool,
    pub(crate) docstring: Option<String>,
    pub(crate) typ: Ty,
}

starlark_simple_value!(NativeAttribute);

impl NativeAttribute {
    #[inline]
    pub(crate) fn invoke_method_impl<'v>(
        function: &dyn NativeAttr,
        this: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        let method = function(this, eval.heap())?;
        method.invoke(args, eval)
    }
}

#[starlark_value(type = "attribute")]
impl<'v> StarlarkValue<'v> for NativeAttribute {
    fn documentation(&self) -> DocItem {
        let ds = self
            .docstring
            .as_ref()
            .and_then(|ds| DocString::from_docstring(DocStringKind::Rust, ds));
        let typ = self.typ.clone();
        DocItem::Member(DocMember::Property(DocProperty { docs: ds, typ }))
    }
}

/// A wrapper for a method with a self object already bound.
#[derive(
    Clone,
    Debug,
    Trace,
    Coerce,
    Display,
    Freeze,
    NoSerialize,
    ProvidesStaticType,
    Allocative
)]
#[repr(C)]
#[display("{}", method)]
pub(crate) struct BoundMethodGen<V: ValueLifetimeless> {
    pub(crate) method: FrozenValueTyped<'static, NativeMethod>,
    pub(crate) this: V,
}

starlark_complex_value!(pub(crate) BoundMethod);

impl<'v, V: ValueLike<'v>> BoundMethodGen<V> {
    /// Create a new [`BoundMethod`]. Given the expression `object.function`,
    /// the first argument would be `object`, and the second would be `getattr(object, "function")`.
    pub(crate) fn new(this: V, method: FrozenValueTyped<'static, NativeMethod>) -> Self {
        BoundMethodGen { method, this }
    }
}

#[starlark_value(type = FUNCTION_TYPE)]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for BoundMethodGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        self.method
            .function
            .invoke(eval, self.this.to_value(), args)
    }

    fn documentation(&self) -> DocItem {
        self.method.documentation()
    }
}
