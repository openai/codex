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

use crate::typing::custom::TyCustom;
use crate::typing::Ty;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::typing::type_compiled::alloc::TypeMatcherAlloc;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::typing::type_compiled::compiled::TypeCompiledImplAsStarlarkValue;
use crate::values::typing::type_compiled::matcher::TypeMatcher;
use crate::values::typing::type_compiled::matchers::IsAny;
use crate::values::typing::type_compiled::matchers::IsBool;
use crate::values::typing::type_compiled::matchers::IsInt;
use crate::values::typing::type_compiled::matchers::IsNone;
use crate::values::typing::type_compiled::matchers::IsStr;
use crate::values::typing::type_compiled::type_matcher_factory::TypeMatcherFactory;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;

/// Allocate a `Ty` with a `TypeMatcher` in starlark heap as `TypeCompiled`.
pub struct TypeCompiledFactory<'a, 'v> {
    heap: &'v Heap,
    ty: &'a Ty,
}

impl<'a, 'v> TypeMatcherAlloc for TypeCompiledFactory<'a, 'v> {
    type Result = TypeCompiled<Value<'v>>;

    fn alloc<T: TypeMatcher>(self, matcher: T) -> Self::Result {
        TypeCompiled::alloc(matcher, self.ty.clone(), self.heap)
    }

    fn custom(self, custom: &TyCustom) -> Self::Result {
        custom.matcher_with_type_compiled_factory(self)
    }

    fn from_type_matcher_factory(self, factory: &TypeMatcherFactory) -> Self::Result {
        factory.factory.type_compiled(self)
    }

    fn any(self) -> TypeCompiled<Value<'v>> {
        if self.ty == &Ty::any() {
            TypeCompiled::any().to_value()
        } else {
            self.alloc(IsAny)
        }
    }

    fn none(self) -> TypeCompiled<Value<'v>> {
        if self.ty == &Ty::none() {
            static IS_NONE: AValueRepr<
                AValueImpl<'static, AValueBasic<TypeCompiledImplAsStarlarkValue<IsNone>>>,
            > = TypeCompiledImplAsStarlarkValue::alloc_static(IsNone, Ty::none());

            TypeCompiled::unchecked_new(FrozenValue::new_repr(&IS_NONE).to_value())
        } else {
            self.alloc(IsNone)
        }
    }

    fn bool(self) -> TypeCompiled<Value<'v>> {
        if self.ty == &Ty::bool() {
            static IS_BOOL: AValueRepr<
                AValueImpl<'static, AValueBasic<TypeCompiledImplAsStarlarkValue<IsBool>>>,
            > = TypeCompiledImplAsStarlarkValue::alloc_static(IsBool, Ty::bool());

            TypeCompiled::unchecked_new(FrozenValue::new_repr(&IS_BOOL).to_value())
        } else {
            self.alloc(IsBool)
        }
    }

    fn int(self) -> TypeCompiled<Value<'v>> {
        if self.ty == &Ty::int() {
            static IS_INT: AValueRepr<
                AValueImpl<AValueBasic<TypeCompiledImplAsStarlarkValue<IsInt>>>,
            > = TypeCompiledImplAsStarlarkValue::alloc_static(IsInt, Ty::int());

            TypeCompiled::unchecked_new(FrozenValue::new_repr(&IS_INT).to_value())
        } else {
            self.alloc(IsInt)
        }
    }

    fn str(self) -> TypeCompiled<Value<'v>> {
        if self.ty == &Ty::string() {
            static IS_STRING: AValueRepr<
                AValueImpl<'static, AValueBasic<TypeCompiledImplAsStarlarkValue<IsStr>>>,
            > = TypeCompiledImplAsStarlarkValue::alloc_static(IsStr, Ty::string());

            TypeCompiled::unchecked_new(FrozenValue::new_repr(&IS_STRING).to_value())
        } else {
            self.alloc(IsStr)
        }
    }
}

impl<'a, 'v> TypeCompiledFactory<'a, 'v> {
    pub(crate) fn alloc_ty(ty: &'a Ty, heap: &'v Heap) -> TypeCompiled<Value<'v>> {
        TypeCompiledFactory { heap, ty }.ty(ty)
    }
}
