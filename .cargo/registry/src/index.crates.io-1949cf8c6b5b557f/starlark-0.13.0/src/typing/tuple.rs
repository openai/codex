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
use std::fmt::Formatter;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

use crate::typing::arc_ty::ArcTy;
use crate::typing::error::InternalError;
use crate::typing::starlark_value::TyStarlarkValue;
use crate::typing::ty::TypeRenderConfig;
use crate::typing::Ty;
use crate::typing::TypingOracleCtx;
use crate::values::typing::type_compiled::alloc::TypeMatcherAlloc;
use crate::values::typing::type_compiled::matcher::TypeMatcherBoxAlloc;
use crate::values::typing::type_compiled::matchers::IsTupleElems;
use crate::values::typing::type_compiled::matchers::IsTupleElems0;
use crate::values::typing::type_compiled::matchers::IsTupleElems1;
use crate::values::typing::type_compiled::matchers::IsTupleElems2;
use crate::values::typing::type_compiled::matchers::IsTupleOf;
use crate::values::typing::type_compiled::matchers::StarlarkTypeIdMatcher;

#[derive(Eq, PartialEq, Hash, Clone, Dupe, Debug, Ord, PartialOrd, Allocative)]
pub enum TyTuple {
    /// `tuple[T0, T1, T2]`.
    Elems(Arc<[Ty]>),
    /// `tuple[T, ...]`.
    Of(ArcTy),
}

impl TyTuple {
    pub(crate) fn get(&self, i: usize) -> Option<&Ty> {
        match self {
            TyTuple::Elems(elems) => elems.get(i),
            TyTuple::Of(t) => Some(t),
        }
    }

    pub(crate) fn item_ty(&self) -> Ty {
        match self {
            TyTuple::Elems(elems) => Ty::unions(elems.to_vec()),
            TyTuple::Of(t) => (**t).clone(),
        }
    }

    pub(crate) fn intersects(
        this: &TyTuple,
        other: &TyTuple,
        ctx: &TypingOracleCtx,
    ) -> Result<bool, InternalError> {
        match (this, other) {
            (TyTuple::Elems(this), TyTuple::Elems(other)) => Ok(this.len() == other.len() && {
                for (x, y) in this.iter().zip(other.iter()) {
                    if !ctx.intersects(x, y)? {
                        return Ok(false);
                    }
                }
                true
            }),
            (TyTuple::Of(this), TyTuple::Of(other)) => ctx.intersects(this, other),
            (TyTuple::Elems(elems), TyTuple::Of(item))
            | (TyTuple::Of(item), TyTuple::Elems(elems)) => {
                // For example `tuple[str, int]` does not intersect with `tuple[str, ...]`.
                for x in elems.iter() {
                    if !ctx.intersects(x, item)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    pub(crate) fn matcher<T: TypeMatcherAlloc>(&self, type_compiled_factory: T) -> T::Result {
        match self {
            TyTuple::Elems(elems) => match &**elems {
                [] => type_compiled_factory.alloc(IsTupleElems0),
                [x0] => type_compiled_factory.alloc(IsTupleElems1(TypeMatcherBoxAlloc.ty(x0))),
                [x0, x1] => {
                    if let (Some(x0), Some(x1)) = (x0.is_starlark_value(), x1.is_starlark_value()) {
                        type_compiled_factory.alloc(IsTupleElems2(
                            StarlarkTypeIdMatcher::new(x0),
                            StarlarkTypeIdMatcher::new(x1),
                        ))
                    } else {
                        type_compiled_factory.alloc(IsTupleElems2(
                            TypeMatcherBoxAlloc.ty(x0),
                            TypeMatcherBoxAlloc.ty(x1),
                        ))
                    }
                }
                xs => {
                    let xs = xs
                        .iter()
                        .map(|x| TypeMatcherBoxAlloc.ty(x))
                        .collect::<Vec<_>>();
                    type_compiled_factory.alloc(IsTupleElems(xs))
                }
            },
            TyTuple::Of(item) if item.is_any() => {
                TyStarlarkValue::tuple().matcher(type_compiled_factory)
            }
            TyTuple::Of(item) => {
                if let Some(item) = item.is_starlark_value() {
                    return type_compiled_factory
                        .alloc(IsTupleOf(StarlarkTypeIdMatcher::new(item)));
                } else {
                    let item = TypeMatcherBoxAlloc.ty(item);
                    type_compiled_factory.alloc(IsTupleOf(item))
                }
            }
        }
    }

    pub(crate) fn fmt_with_config(
        &self,
        f: &mut Formatter<'_>,
        config: &TypeRenderConfig,
    ) -> fmt::Result {
        match self {
            TyTuple::Elems(elems) => match &**elems {
                [x] => write!(f, "({},)", x.display_with(config)),
                xs => display_container::fmt_container(
                    f,
                    "(",
                    ")",
                    xs.iter().map(|x| x.display_with(config)),
                ),
            },
            TyTuple::Of(item) => {
                if item.is_any() {
                    write!(f, "tuple")
                } else {
                    write!(f, "tuple[{}, ...]", item.display_with(config))
                }
            }
        }
    }
}

impl Display for TyTuple {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TyTuple::Elems(elems) => match &**elems {
                [x] => write!(f, "({},)", x),
                xs => display_container::fmt_container(f, "(", ")", xs),
            },
            TyTuple::Of(item) => {
                if item.is_any() {
                    write!(f, "tuple")
                } else {
                    write!(f, "tuple[{}, ...]", item)
                }
            }
        }
    }
}
