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

use starlark_syntax::slice_vec_ext::SliceExt;

use crate::typing::custom::TyCustom;
use crate::typing::starlark_value::TyStarlarkValue;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::values::typing::type_compiled::matcher::TypeMatcher;
use crate::values::typing::type_compiled::matcher::TypeMatcherBoxAlloc;
use crate::values::typing::type_compiled::matchers::IsAny;
use crate::values::typing::type_compiled::matchers::IsAnyOf;
use crate::values::typing::type_compiled::matchers::IsAnyOfTwo;
use crate::values::typing::type_compiled::matchers::IsBool;
use crate::values::typing::type_compiled::matchers::IsCallable;
use crate::values::typing::type_compiled::matchers::IsDict;
use crate::values::typing::type_compiled::matchers::IsDictOf;
use crate::values::typing::type_compiled::matchers::IsInt;
use crate::values::typing::type_compiled::matchers::IsIterable;
use crate::values::typing::type_compiled::matchers::IsList;
use crate::values::typing::type_compiled::matchers::IsListOf;
use crate::values::typing::type_compiled::matchers::IsNever;
use crate::values::typing::type_compiled::matchers::IsNone;
use crate::values::typing::type_compiled::matchers::IsSet;
use crate::values::typing::type_compiled::matchers::IsSetOf;
use crate::values::typing::type_compiled::matchers::IsStr;
use crate::values::typing::type_compiled::matchers::IsType;
use crate::values::typing::type_compiled::matchers::StarlarkTypeIdMatcher;
use crate::values::typing::type_compiled::type_matcher_factory::TypeMatcherFactory;

/// Allocate runtime type matcher, either in starlark heap or in malloc.
pub trait TypeMatcherAlloc: Sized {
    type Result;

    fn alloc<T: TypeMatcher>(self, matcher: T) -> Self::Result;

    fn custom(self, custom: &TyCustom) -> Self::Result;

    fn from_type_matcher_factory(self, factory: &TypeMatcherFactory) -> Self::Result;

    // Now the utilities.

    fn unreachable_cannot_appear_in_type_expr(self) -> ! {
        // TODO(nga): replace panic with error.
        unreachable!("type cannot appear in type expressions")
    }

    /// `typing.Any`.
    fn any(self) -> Self::Result {
        self.alloc(IsAny)
    }

    /// `typing.Never`.
    fn never(self) -> Self::Result {
        self.alloc(IsNever)
    }

    /// `None`.
    fn none(self) -> Self::Result {
        self.alloc(IsNone)
    }

    /// `bool`.
    fn bool(self) -> Self::Result {
        self.alloc(IsBool)
    }

    /// `int`.
    fn int(self) -> Self::Result {
        self.alloc(IsInt)
    }

    /// `str`.
    fn str(self) -> Self::Result {
        self.alloc(IsStr)
    }

    fn ty(self, ty: &Ty) -> Self::Result {
        match ty.iter_union() {
            [] => self.never(),
            [ty] => self.ty_basic(ty),
            [ty0, ty1] => self.any_of_two_basic(ty0, ty1),
            tys => {
                let matchers = tys.map(|t| TypeMatcherBoxAlloc.ty_basic(t));
                if matchers.iter().any(|m| m.is_wildcard()) {
                    self.any()
                } else {
                    self.alloc(IsAnyOf(matchers))
                }
            }
        }
    }

    /// `A | B`.
    fn any_of_two_matcher(self, m0: impl TypeMatcher, m1: impl TypeMatcher) -> Self::Result {
        if m0.is_wildcard() {
            self.alloc(m1)
        } else if m1.is_wildcard() {
            self.alloc(m0)
        } else {
            self.alloc(IsAnyOfTwo(m0, m1))
        }
    }

    /// `A | B`.
    fn any_of_two_basic(self, ty0: &TyBasic, ty1: &TyBasic) -> Self::Result {
        if ty0 == &TyBasic::Any {
            self.ty_basic(ty1)
        } else if ty1 == &TyBasic::Any {
            self.ty_basic(ty0)
        } else if ty0 == &TyBasic::none() {
            self.none_or_basic(ty1)
        } else if ty1 == &TyBasic::none() {
            self.none_or_basic(ty0)
        } else {
            let m0 = TypeMatcherBoxAlloc.ty_basic(ty0);
            let m1 = TypeMatcherBoxAlloc.ty_basic(ty1);
            self.any_of_two_matcher(m0, m1)
        }
    }

    fn ty_basic(self, ty: &TyBasic) -> Self::Result {
        match ty {
            TyBasic::Any => self.any(),
            TyBasic::StarlarkValue(x) => x.matcher(self),
            TyBasic::List(item) => self.list_of(item),
            TyBasic::Tuple(tuple) => tuple.matcher(self),
            TyBasic::Dict(k, v) => self.dict_of(k, v),
            TyBasic::Iter(_item) => self.alloc(IsIterable),
            TyBasic::Callable(_c) => self.alloc(IsCallable),
            TyBasic::Type => self.alloc(IsType),
            TyBasic::Custom(custom) => self.custom(custom),
            TyBasic::Set(item) => self.set_of(item),
        }
    }

    fn callable(self) -> Self::Result {
        self.alloc(IsCallable)
    }

    /// `A | None`.
    fn none_or_starlark_value(self, ty: TyStarlarkValue) -> Self::Result {
        if ty.is_str() {
            self.alloc(IsAnyOfTwo(IsNone, IsStr))
        } else if ty.is_int() {
            self.alloc(IsAnyOfTwo(IsNone, IsInt))
        } else {
            self.alloc(IsAnyOfTwo(IsNone, StarlarkTypeIdMatcher::new(ty)))
        }
    }

    /// `A | None`.
    fn none_or_basic(self, ty: &TyBasic) -> Self::Result {
        match ty {
            TyBasic::Any => self.alloc(IsNone),
            ty if *ty == TyBasic::any_list() => self.alloc(IsAnyOfTwo(IsNone, IsList)),
            TyBasic::StarlarkValue(ty) => self.none_or_starlark_value(*ty),
            ty => self.alloc(IsAnyOfTwo(IsNone, TypeMatcherBoxAlloc.ty_basic(ty))),
        }
    }

    /// `list`.
    fn list(self) -> Self::Result {
        self.alloc(IsList)
    }

    /// `list[Item]`.
    fn list_of_matcher(self, item: impl TypeMatcher) -> Self::Result {
        if item.is_wildcard() {
            self.list()
        } else {
            self.alloc(IsListOf(item))
        }
    }

    /// `list[Item]`.
    fn list_of_starlark_value(self, item: TyStarlarkValue) -> Self::Result {
        if item.is_str() {
            self.list_of_matcher(IsStr)
        } else {
            self.list_of_matcher(StarlarkTypeIdMatcher::new(item))
        }
    }

    /// `list[Item]`.
    fn list_of_basic(self, item: &TyBasic) -> Self::Result {
        match item {
            TyBasic::Any => self.list(),
            TyBasic::StarlarkValue(ty) => self.list_of_starlark_value(*ty),
            ty => self.list_of_matcher(TypeMatcherBoxAlloc.ty_basic(ty)),
        }
    }

    /// `list[Item]`.
    fn list_of(self, item: &Ty) -> Self::Result {
        if item.is_any() {
            self.list()
        } else if let [ty] = item.iter_union() {
            self.list_of_basic(ty)
        } else {
            let matcher = TypeMatcherBoxAlloc.ty(item);
            self.list_of_matcher(matcher)
        }
    }

    /// `dict`.
    fn dict(self) -> Self::Result {
        self.alloc(IsDict)
    }

    /// `dict[Key, Value]`.
    fn dict_of_matcher(self, k: impl TypeMatcher, v: impl TypeMatcher) -> Self::Result {
        match (k.is_wildcard(), v.is_wildcard()) {
            (true, true) => self.dict(),
            (true, false) => self.alloc(IsDictOf(IsAny, v)),
            (false, true) => self.alloc(IsDictOf(k, IsAny)),
            (false, false) => self.alloc(IsDictOf(k, v)),
        }
    }

    /// `dict[Key, Value]`.
    fn dict_of_starlark_value_to_something(self, k: TyStarlarkValue, v: &Ty) -> Self::Result {
        if k.is_str() {
            // Optimize matchers for common types like `dict[str, something]`.
            self.dict_of_matcher(IsStr, TypeMatcherBoxAlloc.ty(v))
        } else {
            self.dict_of_matcher(StarlarkTypeIdMatcher::new(k), TypeMatcherBoxAlloc.ty(v))
        }
    }

    /// `dict[Key, Value]`.
    fn dict_of(self, k: &Ty, v: &Ty) -> Self::Result {
        if k.is_any() && v.is_any() {
            self.dict()
        } else if let Some(k) = k.is_starlark_value() {
            self.dict_of_starlark_value_to_something(k, v)
        } else {
            let k = TypeMatcherBoxAlloc.ty(k);
            let v = TypeMatcherBoxAlloc.ty(v);
            self.dict_of_matcher(k, v)
        }
    }

    /// `set`.
    fn set(self) -> Self::Result {
        self.alloc(IsSet)
    }

    /// `set[Item]`.
    fn set_of_matcher(self, item: impl TypeMatcher) -> Self::Result {
        if item.is_wildcard() {
            self.set()
        } else {
            self.alloc(IsSetOf(item))
        }
    }

    /// `set[Item]`.
    fn set_of_starlark_value(self, item: TyStarlarkValue) -> Self::Result {
        if item.is_str() {
            self.set_of_matcher(IsStr)
        } else {
            self.set_of_matcher(StarlarkTypeIdMatcher::new(item))
        }
    }

    /// `set[Item]`.
    fn set_of_basic(self, item: &TyBasic) -> Self::Result {
        match item {
            TyBasic::Any => self.set(),
            TyBasic::StarlarkValue(ty) => self.set_of_starlark_value(*ty),
            ty => self.set_of_matcher(TypeMatcherBoxAlloc.ty_basic(ty)),
        }
    }

    fn set_of(self, item: &Ty) -> Self::Result {
        if item.is_any() {
            self.set()
        } else if let [ty] = item.iter_union() {
            self.set_of_basic(ty)
        } else {
            let matcher = TypeMatcherBoxAlloc.ty(item);
            self.set_of_matcher(matcher)
        }
    }
}
