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
use std::fmt::Display;

use allocative::Allocative;
use dupe::Dupe;

use crate::typing::arc_ty::ArcTy;
use crate::typing::callable::TyCallable;
use crate::typing::custom::TyCustom;
use crate::typing::custom::TyCustomImpl;
use crate::typing::starlark_value::TyStarlarkValue;
use crate::typing::tuple::TyTuple;
use crate::typing::ty::TypeRenderConfig;
use crate::typing::Ty;
use crate::typing::TyFunction;
use crate::values::none::NoneType;
use crate::values::string::str_type::StarlarkStr;
use crate::values::typing::any::TypingAny;
use crate::values::StarlarkValue;

/// Type that is not a union.
#[derive(Eq, PartialEq, Hash, Clone, Dupe, Debug, Ord, PartialOrd, Allocative)]
pub enum TyBasic {
    /// Type that contain anything
    Any,
    /// Type is handled by `StarlarkValue` trait implementation.
    StarlarkValue(TyStarlarkValue),
    /// Iter is a type that supports iteration, only used as arguments to primitive functions.
    /// The inner type is applicable for each iteration element.
    Iter(ArcTy),
    /// `typing.Callable`.
    Callable(TyCallable),
    /// `type`.
    Type,
    /// A list.
    List(ArcTy),
    /// A tuple. May be empty, to indicate the empty tuple.
    Tuple(TyTuple),
    /// A dictionary, with key and value types
    Dict(ArcTy, ArcTy),
    /// Custom type.
    Custom(TyCustom),
    /// A set.
    Set(ArcTy),
}

impl TyBasic {
    pub(crate) const fn none() -> TyBasic {
        TyBasic::starlark_value::<NoneType>()
    }

    pub(crate) const fn starlark_value<'v, T: StarlarkValue<'v>>() -> TyBasic {
        TyBasic::StarlarkValue(TyStarlarkValue::new::<T>())
    }

    pub(crate) const fn string() -> Self {
        TyBasic::starlark_value::<StarlarkStr>()
    }

    pub(crate) const fn int() -> Self {
        TyBasic::StarlarkValue(TyStarlarkValue::int())
    }

    pub(crate) const fn float() -> Self {
        TyBasic::StarlarkValue(TyStarlarkValue::float())
    }

    /// Create a list type.
    pub(crate) fn list(element: Ty) -> Self {
        TyBasic::List(ArcTy::new(element))
    }

    /// `list[typing.Any]`.
    pub(crate) fn any_list() -> Self {
        TyBasic::List(ArcTy::any())
    }

    /// `dict[typing.Any, typing.Any]`.
    pub(crate) fn any_dict() -> Self {
        Self::dict(Ty::any(), Ty::any())
    }

    pub(crate) fn any_set() -> Self {
        TyBasic::Set(ArcTy::any())
    }

    /// Create a iterable type.
    pub(crate) fn iter(item: Ty) -> Self {
        TyBasic::Iter(ArcTy::new(item))
    }

    /// Create a dictionary type.
    pub(crate) fn dict(key: Ty, value: Ty) -> Self {
        TyBasic::Dict(ArcTy::new(key), ArcTy::new(value))
    }

    /// Create a set type.
    pub(crate) fn set(item: Ty) -> Self {
        TyBasic::Set(ArcTy::new(item))
    }

    pub(crate) fn custom(custom: impl TyCustomImpl) -> Self {
        TyBasic::Custom(TyCustom::new(custom))
    }

    /// Turn a type back into a name, potentially erasing some structure.
    /// E.g. the type `[bool]` would return `list`.
    /// Types like [`Ty::any`] will return `None`.
    pub fn as_name(&self) -> Option<&str> {
        match self {
            TyBasic::StarlarkValue(t) => Some(t.as_name()),
            TyBasic::List(_) => Some("list"),
            TyBasic::Tuple(_) => Some("tuple"),
            TyBasic::Dict(..) => Some("dict"),
            TyBasic::Type => Some("type"),
            TyBasic::Custom(c) => c.as_name(),
            TyBasic::Any | TyBasic::Iter(_) | TyBasic::Callable(_) => None,
            TyBasic::Set(_) => Some("set"),
        }
    }

    /// If this type is function, return the function type.
    pub(crate) fn as_function(&self) -> Option<&TyFunction> {
        match self {
            TyBasic::Custom(c) => c.0.as_function_dyn(),
            _ => None,
        }
    }

    /// Type is a tuple, with specified or unspecified member types.
    pub(crate) fn is_tuple(&self) -> bool {
        matches!(self, TyBasic::Tuple(_))
    }

    /// Type is a list, with specified or unspecified member types.
    pub(crate) fn is_list(&self) -> bool {
        self.as_name() == Some("list")
    }

    pub(crate) fn fmt_with_config(
        &self,
        f: &mut fmt::Formatter<'_>,
        config: &TypeRenderConfig,
    ) -> fmt::Result {
        match self {
            TyBasic::Any => write!(f, "{}", TypingAny::TYPE),
            TyBasic::StarlarkValue(x) => x.fmt_with_config(f, config),
            TyBasic::Iter(x) => {
                if x.is_any() {
                    write!(f, "typing.Iterable")
                } else {
                    write!(f, "typing.Iterable[{}]", x.display_with(config))
                }
            }
            TyBasic::Callable(c) => c.fmt_with_config(f, config),
            TyBasic::List(x) => {
                if x.is_any() {
                    write!(f, "list")
                } else {
                    write!(f, "list[{}]", x.display_with(config))
                }
            }
            TyBasic::Tuple(tuple) => tuple.fmt_with_config(f, config),
            TyBasic::Dict(k, v) => {
                if k.is_any() && v.is_any() {
                    write!(f, "dict")
                } else {
                    write!(
                        f,
                        "dict[{}, {}]",
                        k.display_with(config),
                        v.display_with(config)
                    )
                }
            }
            TyBasic::Type => write!(f, "type"),
            TyBasic::Custom(c) => Display::fmt(c, f),
            TyBasic::Set(x) => write!(f, "set[{}]", x.display_with(config)),
        }
    }
}

impl Display for TyBasic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_config(f, &TypeRenderConfig::Default)
    }
}
