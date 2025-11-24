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

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::hash::Hash;
use std::hash::Hasher;

use allocative::Allocative;
use dupe::Dupe;
use starlark_derive::starlark_module;
use starlark_derive::starlark_value;
use starlark_map::StarlarkHasher;
use starlark_syntax::slice_vec_ext::SliceExt;
use starlark_syntax::slice_vec_ext::VecExt;
use thiserror::Error;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::Coerce;
use crate::environment::Methods;
use crate::environment::MethodsBuilder;
use crate::environment::MethodsStatic;
use crate::private::Private;
use crate::typing::Ty;
use crate::values::dict::DictRef;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::list::ListRef;
use crate::values::none::NoneType;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::tuple::value::Tuple;
use crate::values::typing::type_compiled::factory::TypeCompiledFactory;
use crate::values::typing::type_compiled::matcher::TypeMatcher;
use crate::values::typing::type_compiled::matchers::IsAny;
use crate::values::AllocValue;
use crate::values::Demand;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::Trace;
use crate::values::Value;
use crate::values::ValueLifetimeless;
use crate::values::ValueLike;

#[derive(Debug, Error)]
enum TypingError {
    /// The value does not have the specified type
    #[error("Value `{0}` of type `{1}` does not match the type annotation `{2}` for {3}")]
    TypeAnnotationMismatch(String, String, String, String),
    /// The given type annotation does not represent a type
    #[error("Type `{0}` is not a valid type annotation")]
    InvalidTypeAnnotation(String),
    #[error("`{{A: B}}` cannot be used as type, perhaps you meant `dict[A, B]`")]
    Dict,
    #[error("`[X]` cannot be used as type, perhaps you meant `list[X]`")]
    List,
    /// The given type annotation does not exist, but the user might have forgotten quotes around
    /// it
    #[error(r#"Found `{0}` instead of a valid type annotation. Perhaps you meant `"{1}"`?"#)]
    PerhapsYouMeant(String, String),
    #[error("Value of type `{1}` does not match type `{2}`: {0}")]
    ValueDoesNotMatchType(String, &'static str, String),
    #[error("String literals are not allowed in type expressions: `{0}`")]
    StringLiteralNotAllowed(String),
}

pub(crate) trait TypeCompiledDyn: Debug + Allocative + Send + Sync + 'static {
    fn as_ty_dyn(&self) -> &Ty;
    fn is_runtime_wildcard_dyn(&self) -> bool;
    fn to_frozen_dyn(&self, heap: &FrozenHeap) -> TypeCompiled<FrozenValue>;
}

// TODO(nga): derive.
unsafe impl<'v> ProvidesStaticType<'v> for &'v dyn TypeCompiledDyn {
    type StaticType = &'static dyn TypeCompiledDyn;
}

impl<T> TypeCompiledDyn for TypeCompiledImplAsStarlarkValue<T>
where
    T: TypeMatcher,
{
    fn as_ty_dyn(&self) -> &Ty {
        &self.ty
    }
    fn is_runtime_wildcard_dyn(&self) -> bool {
        self.type_compiled_impl.is_wildcard()
    }
    fn to_frozen_dyn(&self, heap: &FrozenHeap) -> TypeCompiled<FrozenValue> {
        TypeCompiled(heap.alloc_simple::<TypeCompiledImplAsStarlarkValue<T>>(Self::clone(self)))
    }
}

#[derive(
    Clone,
    Eq,
    PartialEq,
    Debug,
    Allocative,
    ProvidesStaticType,
    NoSerialize
)]
pub struct TypeCompiledImplAsStarlarkValue<T: 'static> {
    type_compiled_impl: T,
    ty: Ty,
}

impl<T> TypeCompiledImplAsStarlarkValue<T>
where
    TypeCompiledImplAsStarlarkValue<T>: StarlarkValue<'static>,
{
    pub(crate) const fn alloc_static(
        imp: T,
        ty: Ty,
    ) -> AValueRepr<AValueImpl<'static, AValueBasic<TypeCompiledImplAsStarlarkValue<T>>>> {
        alloc_static(TypeCompiledImplAsStarlarkValue {
            type_compiled_impl: imp,
            ty,
        })
    }
}

#[doc(hidden)]
#[derive(Hash, Eq, PartialEq, Debug, Clone, Allocative)]
pub struct DummyTypeMatcher;

impl TypeMatcher for DummyTypeMatcher {
    fn matches(&self, _value: Value) -> bool {
        unreachable!()
    }
}

#[starlark_value(type = "type")]
impl<'v, T: 'static> StarlarkValue<'v> for TypeCompiledImplAsStarlarkValue<T>
where
    T: TypeMatcher,
{
    type Canonical = TypeCompiledImplAsStarlarkValue<DummyTypeMatcher>;

    fn type_matches_value(&self, value: Value<'v>, _private: Private) -> bool {
        self.type_compiled_impl.matches(value)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_ref_static::<dyn TypeCompiledDyn>(self);
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        Hash::hash(&self.ty, hasher);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> crate::Result<bool> {
        let Some(other) = other.downcast_ref::<Self>() else {
            return Ok(false);
        };
        Ok(self.ty == other.ty)
    }

    fn eval_type(&self) -> Option<Ty> {
        // `TypeCompiled::new` handles this type explicitly,
        // but implement this function to make proc-macro generate `bit_or`.
        // Also safer to be explicit here.
        Some(self.ty.clone())
    }

    fn get_methods() -> Option<&'static Methods>
    where
        Self: Sized,
    {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(type_compiled_methods)
    }
}

impl<T: TypeMatcher> Display for TypeCompiledImplAsStarlarkValue<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ty)
    }
}

#[starlark_module]
fn type_compiled_methods(methods: &mut MethodsBuilder) {
    /// True iff the value matches this type.
    fn matches<'v>(this: Value<'v>, value: Value<'v>) -> anyhow::Result<bool> {
        Ok(this.get_ref().type_matches_value(value))
    }

    /// Error if the value does not match this type.
    fn check_matches<'v>(this: Value<'v>, value: Value<'v>) -> anyhow::Result<NoneType> {
        if !this.get_ref().type_matches_value(value) {
            return Err(TypingError::ValueDoesNotMatchType(
                value.to_repr(),
                value.get_type(),
                TypeCompiled(this).to_string(),
            )
            .into());
        }
        Ok(NoneType)
    }
}

/// Wrapper for a [`Value`] that acts like a runtime type matcher.
#[derive(
    Debug,
    Allocative,
    Freeze,
    Trace,
    Clone,
    Copy,
    Dupe,
    Coerce,
    ProvidesStaticType
)]
#[repr(transparent)]
pub struct TypeCompiled<V: ValueLifetimeless>(
    /// `V` is `TypeCompiledImplAsStarlarkValue`.
    V,
);

impl<'v, V: ValueLike<'v>> Display for TypeCompiled<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.downcast() {
            Ok(t) => Display::fmt(&t.as_ty_dyn(), f),
            Err(_) => {
                // This is unreachable, but we should not panic in `Display`.
                Display::fmt(&self.0, f)
            }
        }
    }
}

impl<V: ValueLifetimeless> StarlarkTypeRepr for TypeCompiled<V> {
    type Canonical = TypeCompiledImplAsStarlarkValue<DummyTypeMatcher>;

    fn starlark_type_repr() -> Ty {
        TypeCompiledImplAsStarlarkValue::<DummyTypeMatcher>::starlark_type_repr()
    }
}

impl<'v, V: ValueLike<'v>> AllocValue<'v> for TypeCompiled<V> {
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        self.0.to_value()
    }
}

impl<'v, V: ValueLike<'v>> TypeCompiled<V> {
    pub(crate) fn unchecked_new(value: V) -> Self {
        TypeCompiled(value)
    }

    fn downcast(self) -> anyhow::Result<&'v dyn TypeCompiledDyn> {
        self.to_value()
            .0
            .request_value::<&dyn TypeCompiledDyn>()
            .ok_or_else(|| anyhow::anyhow!("Not TypeCompiledImpl (internal error)"))
    }

    /// Check if given value matches this type.
    pub fn matches(&self, value: Value<'v>) -> bool {
        self.0.to_value().get_ref().type_matches_value(value)
    }

    /// Get the typechecker type for this runtime type.
    pub fn as_ty(&self) -> &'v Ty {
        self.downcast().unwrap().as_ty_dyn()
    }

    /// True if `TypeCompiled` matches any type at runtime.
    /// However, compile-time/lint typechecker may still check the type.
    pub(crate) fn is_runtime_wildcard(self) -> bool {
        self.downcast().unwrap().is_runtime_wildcard_dyn()
    }

    #[cold]
    #[inline(never)]
    fn check_type_error(self, value: Value<'v>, arg_name: Option<&str>) -> crate::Result<()> {
        Err(crate::Error::new_other(
            TypingError::TypeAnnotationMismatch(
                value.to_str(),
                value.get_type().to_owned(),
                self.to_string(),
                match arg_name {
                    None => "return type".to_owned(),
                    Some(x) => format!("argument `{}`", x),
                },
            ),
        ))
    }

    pub(crate) fn check_type(self, value: Value<'v>, arg_name: Option<&str>) -> crate::Result<()> {
        if self.matches(value) {
            Ok(())
        } else {
            self.check_type_error(value, arg_name)
        }
    }

    pub(crate) fn to_value(self) -> TypeCompiled<Value<'v>> {
        TypeCompiled(self.0.to_value())
    }

    pub(crate) fn to_inner(self) -> V {
        self.0
    }

    pub(crate) fn write_hash(self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        self.to_value().0.write_hash(hasher)
    }

    // Dead code, but may become useful in the future.
    pub(crate) fn _equals(self, other: Self) -> crate::Result<bool> {
        self.to_value().0.equals(other.to_value().0)
    }
}

impl<'v, V: ValueLike<'v>> Hash for TypeCompiled<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.0.to_value().get_hash() {
            Ok(h) => h.hash(state),
            Err(_) => {
                // Unreachable, but we should not panic in `Hash`.
            }
        }
    }
}

impl<'v, V: ValueLike<'v>> PartialEq for TypeCompiled<V> {
    #[allow(clippy::manual_unwrap_or)]
    fn eq(&self, other: &Self) -> bool {
        self.0
            .to_value()
            .equals(other.0.to_value())
            .unwrap_or_default()
    }
}

impl<'v, V: ValueLike<'v>> Eq for TypeCompiled<V> {}

impl<'v, V: ValueLike<'v>> TypeCompiled<V> {
    /// Reallocate the type in a frozen heap.
    pub fn to_frozen(self, heap: &FrozenHeap) -> TypeCompiled<FrozenValue> {
        if let Some(v) = self.0.to_value().unpack_frozen() {
            TypeCompiled(v)
        } else {
            self.to_value().downcast().unwrap().to_frozen_dyn(heap)
        }
    }
}

// These functions are small, but are deliberately out-of-line so we get better
// information in profiling about the origin of these closures
impl<'v> TypeCompiled<Value<'v>> {
    pub(crate) fn alloc(
        type_compiled_impl: impl TypeMatcher,
        ty: Ty,
        heap: &'v Heap,
    ) -> TypeCompiled<Value<'v>> {
        TypeCompiled(heap.alloc_simple(TypeCompiledImplAsStarlarkValue {
            type_compiled_impl,
            ty,
        }))
    }

    pub(crate) fn type_list_of(
        t: TypeCompiled<Value<'v>>,
        heap: &'v Heap,
    ) -> TypeCompiled<Value<'v>> {
        TypeCompiledFactory::alloc_ty(&Ty::list(t.as_ty().clone()), heap)
    }

    pub(crate) fn type_set_of(
        t: TypeCompiled<Value<'v>>,
        heap: &'v Heap,
    ) -> TypeCompiled<Value<'v>> {
        TypeCompiledFactory::alloc_ty(&Ty::set(t.as_ty().clone()), heap)
    }

    pub(crate) fn type_any_of_two(
        t0: TypeCompiled<Value<'v>>,
        t1: TypeCompiled<Value<'v>>,
        heap: &'v Heap,
    ) -> TypeCompiled<Value<'v>> {
        let ty = Ty::union2(t0.as_ty().clone(), t1.as_ty().clone());
        TypeCompiledFactory::alloc_ty(&ty, heap)
    }

    pub(crate) fn type_any_of(
        ts: Vec<TypeCompiled<Value<'v>>>,
        heap: &'v Heap,
    ) -> TypeCompiled<Value<'v>> {
        let ty = Ty::unions(ts.into_map(|t| t.as_ty().clone()));
        TypeCompiledFactory::alloc_ty(&ty, heap)
    }

    pub(crate) fn type_dict_of(
        kt: TypeCompiled<Value<'v>>,
        vt: TypeCompiled<Value<'v>>,
        heap: &'v Heap,
    ) -> TypeCompiled<Value<'v>> {
        let ty = Ty::dict(kt.as_ty().clone(), vt.as_ty().clone());
        TypeCompiledFactory::alloc_ty(&ty, heap)
    }

    /// Parse `[t1, t2, ...]` as type.
    fn from_list(t: &ListRef<'v>, heap: &'v Heap) -> anyhow::Result<TypeCompiled<Value<'v>>> {
        match t.content() {
            [] | [_] => Err(TypingError::List.into()),
            ts @ [_, _, ..] => {
                // A union type, can match any
                let ts = ts.try_map(|t| TypeCompiled::new(*t, heap))?;
                Ok(TypeCompiled::type_any_of(ts, heap))
            }
        }
    }

    pub(crate) fn from_ty(ty: &Ty, heap: &'v Heap) -> Self {
        TypeCompiledFactory::alloc_ty(ty, heap)
    }

    /// Evaluate type annotation at runtime.
    pub fn new(ty: Value<'v>, heap: &'v Heap) -> anyhow::Result<Self> {
        if let Some(s) = StringValue::new(ty) {
            return Err(TypingError::StringLiteralNotAllowed(s.to_string()).into());
        } else if ty.is_none() {
            Ok(TypeCompiledFactory::alloc_ty(&Ty::none(), heap))
        } else if let Some(t) = Tuple::from_value(ty) {
            let elems = t
                .content()
                .try_map(|t| anyhow::Ok(TypeCompiled::new(*t, heap)?.as_ty().clone()))?;
            Ok(TypeCompiled::from_ty(&Ty::tuple(elems), heap))
        } else if let Some(t) = ListRef::from_value(ty) {
            TypeCompiled::from_list(t, heap)
        } else if ty.request_value::<&dyn TypeCompiledDyn>().is_some() {
            // This branch is optimization: `TypeCompiledAsStarlarkValue` implements `eval_type`,
            // but this branch avoids copying the type.
            Ok(TypeCompiled(ty))
        } else if let Some(ty) = ty.get_ref().eval_type() {
            Ok(TypeCompiled::from_ty(&ty, heap))
        } else {
            Err(invalid_type_annotation(ty, heap).into())
        }
    }
}

impl TypeCompiled<FrozenValue> {
    /// Evaluate type annotation at runtime.
    pub(crate) fn new_frozen(ty: FrozenValue, frozen_heap: &FrozenHeap) -> anyhow::Result<Self> {
        // TODO(nga): trip to a heap is not free.
        let heap = Heap::new();
        let ty = TypeCompiled::new(ty.to_value(), &heap)?;
        Ok(ty.to_frozen(frozen_heap))
    }

    /// `typing.Any`.
    pub fn any() -> TypeCompiled<FrozenValue> {
        static ANYTHING: AValueRepr<
            AValueImpl<'static, AValueBasic<TypeCompiledImplAsStarlarkValue<IsAny>>>,
        > = TypeCompiledImplAsStarlarkValue::alloc_static(IsAny, Ty::any());

        TypeCompiled::unchecked_new(FrozenValue::new_repr(&ANYTHING))
    }
}

fn invalid_type_annotation<'v>(ty: Value<'v>, heap: &'v Heap) -> TypingError {
    if DictRef::from_value(ty).is_some() {
        TypingError::Dict
    } else if ListRef::from_value(ty).is_some() {
        TypingError::List
    } else if let Some(name) = ty
        .get_attr("type", heap)
        .ok()
        .flatten()
        .and_then(|v| v.unpack_str())
    {
        TypingError::PerhapsYouMeant(ty.to_str(), name.into())
    } else {
        TypingError::InvalidTypeAnnotation(ty.to_str())
    }
}
