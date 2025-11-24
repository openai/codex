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

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::hash::Hash;
use std::hash::Hasher;
use std::marker::PhantomData;

use allocative::Allocative;
use dupe::Dupe;
use starlark_syntax::codemap::Span;

use crate::typing::error::TypingError;
use crate::typing::error::TypingNoContextError;
use crate::typing::ty::TypeRenderConfig;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::typing::TypingBinOp;
use crate::typing::TypingOracleCtx;
use crate::typing::TypingUnOp;
use crate::values::bool::StarlarkBool;
use crate::values::dict::value::FrozenDict;
use crate::values::float::StarlarkFloat;
use crate::values::list::value::FrozenList;
use crate::values::none::NoneType;
use crate::values::set::value::FrozenSet;
use crate::values::starlark_type_id::StarlarkTypeId;
use crate::values::string::str_type::StarlarkStr;
use crate::values::traits::StarlarkValueVTable;
use crate::values::traits::StarlarkValueVTableGet;
use crate::values::tuple::value::Tuple;
use crate::values::types::bigint::StarlarkBigInt;
use crate::values::typing::type_compiled::alloc::TypeMatcherAlloc;
use crate::values::typing::type_compiled::matchers::StarlarkTypeIdMatcher;
use crate::values::StarlarkValue;

#[derive(Debug, thiserror::Error)]
enum TyStarlarkValueError {
    #[error("Type `{0}` is not callable")]
    NotCallable(TyStarlarkValue),
}

// This is a bit suboptimal for binary size:
// we have two vtable instances for each type: this one, and the one within `AValue` vtable.
struct TyStarlarkValueVTable {
    type_name: &'static str,
    // TODO(nga): put these into generated `StarlarkValueVTable`.
    vtable: StarlarkValueVTable,
    starlark_type_id: StarlarkTypeId,
    /// `starlark_type_id` is `TypeId` of `T::Canonical`.
    /// This is `TypeId` of `T::Canonical::Canonical`.
    starlark_type_id_check: StarlarkTypeId,
}

struct TyStarlarkValueVTableGet<'v, T: StarlarkValue<'v>>(PhantomData<&'v T>);

impl<'v, T: StarlarkValue<'v>> TyStarlarkValueVTableGet<'v, T> {
    const VTABLE: TyStarlarkValueVTable = TyStarlarkValueVTable {
        type_name: T::TYPE,
        vtable: StarlarkValueVTableGet::<T>::VTABLE,
        starlark_type_id: StarlarkTypeId::of_canonical::<T>(),
        starlark_type_id_check: StarlarkTypeId::of_canonical::<T::Canonical>(),
    };
}

/// Type implementation where typing is handled by the `StarlarkValue` trait implementation.
#[derive(Allocative, Clone, Copy, Dupe)]
pub struct TyStarlarkValue {
    #[allocative(skip)]
    vtable: &'static TyStarlarkValueVTable,
}

impl Debug for TyStarlarkValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("TyStarlarkValue")
            .field("type_name", &self.vtable.type_name)
            .finish_non_exhaustive()
    }
}

impl Display for TyStarlarkValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt_with_config(f, &TypeRenderConfig::Default)
    }
}

impl PartialEq for TyStarlarkValue {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.vtable.starlark_type_id == other.vtable.starlark_type_id
    }
}

impl Eq for TyStarlarkValue {}

impl Hash for TyStarlarkValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash type name because type id is not stable.
        // TODO(nga): store hash in vtable.
        self.vtable.type_name.hash(state);
    }
}

impl PartialOrd for TyStarlarkValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TyStarlarkValue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.vtable.type_name.cmp(other.vtable.type_name)
    }
}

impl TyStarlarkValue {
    /// Create a type instance from an implementation of `StarlarkValue`.
    pub const fn new<'v, T: StarlarkValue<'v>>() -> TyStarlarkValue {
        TyStarlarkValue {
            vtable: &TyStarlarkValueVTableGet::<T::Canonical>::VTABLE,
        }
    }

    #[inline]
    pub(crate) fn starlark_type_id(self) -> StarlarkTypeId {
        self.vtable.starlark_type_id
    }

    // Cannot have this check in constructor where it belongs because `new` is `const`.
    #[inline]
    fn self_check(self) {
        debug_assert_eq!(
            self.vtable.starlark_type_id, self.vtable.starlark_type_id_check,
            "`Canonical` for `{}` is not canonical",
            self.vtable.type_name
        );
    }

    pub(crate) fn as_name(self) -> &'static str {
        self.self_check();
        self.vtable.type_name
    }

    pub(crate) const fn int() -> TyStarlarkValue {
        TyStarlarkValue::new::<StarlarkBigInt>()
    }

    pub(crate) const fn float() -> TyStarlarkValue {
        TyStarlarkValue::new::<StarlarkFloat>()
    }

    pub(crate) const fn tuple() -> TyStarlarkValue {
        TyStarlarkValue::new::<Tuple>()
    }

    pub(crate) fn is_str(self) -> bool {
        self.self_check();
        self == TyStarlarkValue::new::<StarlarkStr>()
    }

    pub(crate) fn is_int(self) -> bool {
        self.self_check();
        self == TyStarlarkValue::new::<StarlarkBigInt>()
    }

    pub(crate) fn is_list(self) -> bool {
        self.self_check();
        self == TyStarlarkValue::new::<FrozenList>()
    }

    pub(crate) fn is_dict(self) -> bool {
        self.self_check();
        self == TyStarlarkValue::new::<FrozenDict>()
    }

    pub(crate) fn is_tuple(self) -> bool {
        self.self_check();
        self == TyStarlarkValue::new::<Tuple>()
    }

    #[allow(dead_code)]
    pub(crate) fn is_set(self) -> bool {
        self.self_check();
        self == TyStarlarkValue::new::<FrozenSet>()
    }

    /// Result of applying unary operator to this type.
    pub(crate) fn un_op(self, un_op: TypingUnOp) -> Result<TyStarlarkValue, TypingNoContextError> {
        let has = match un_op {
            TypingUnOp::Plus => self.vtable.vtable.HAS_plus,
            TypingUnOp::Minus => self.vtable.vtable.HAS_minus,
            TypingUnOp::BitNot => self.vtable.vtable.HAS_bit_not,
        };
        if has {
            Ok(self)
        } else {
            Err(TypingNoContextError)
        }
    }

    pub(crate) fn bin_op(self, op: TypingBinOp, rhs: &TyBasic) -> Result<Ty, TypingNoContextError> {
        match (self.vtable.vtable.bin_op_ty)(op, rhs) {
            Some(ty) => Ok(ty),
            None => Err(TypingNoContextError),
        }
    }

    pub(crate) fn rbin_op(
        self,
        op: TypingBinOp,
        lhs: &TyBasic,
    ) -> Result<Ty, TypingNoContextError> {
        match (self.vtable.vtable.rbin_op_ty)(lhs, op) {
            Some(ty) => Ok(ty),
            None => Err(TypingNoContextError),
        }
    }

    pub(crate) fn index(self, _index: &TyBasic) -> Result<Ty, TypingNoContextError> {
        if self.vtable.vtable.HAS_at {
            Ok(Ty::any())
        } else {
            Err(TypingNoContextError)
        }
    }

    /// If this type can be slice, return the result type of slicing.
    pub(crate) fn slice(self) -> Result<Ty, TypingNoContextError> {
        if self.vtable.vtable.HAS_slice {
            // All known implementations of slice return self type.
            Ok(Ty::basic(TyBasic::StarlarkValue(self)))
        } else {
            Err(TypingNoContextError)
        }
    }

    pub(crate) fn is_indexable(self) -> bool {
        self.vtable.vtable.HAS_at
    }

    pub(crate) fn attr_from_methods(self, name: &str) -> Result<Ty, TypingNoContextError> {
        if let Some(methods) = (self.vtable.vtable.get_methods)() {
            if let Some(ty) = methods.get_ty(name) {
                return Ok(ty);
            }
        }
        Err(TypingNoContextError)
    }

    pub(crate) fn attr(self, name: &str) -> Result<Ty, TypingNoContextError> {
        if let Ok(ty) = self.attr_from_methods(name) {
            return Ok(ty);
        }
        if let Some(ty) = (self.vtable.vtable.attr_ty)(name) {
            return Ok(ty);
        }
        Err(TypingNoContextError)
    }

    pub(crate) fn is_callable(self) -> bool {
        self.vtable.vtable.HAS_invoke
    }

    pub(crate) fn validate_call(
        self,
        span: Span,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingError> {
        if self.is_callable() {
            Ok(Ty::any())
        } else {
            Err(oracle.mk_error(span, TyStarlarkValueError::NotCallable(self)))
        }
    }

    #[inline]
    pub(crate) fn is_iterable(vtable: &StarlarkValueVTable) -> bool {
        vtable.HAS_iterate || vtable.HAS_iterate_collect
    }

    /// Instance of this type can be evaluated as a type.
    #[inline]
    pub(crate) fn is_type_from_vtable(vtable: &StarlarkValueVTable) -> bool {
        vtable.HAS_eval_type
    }

    pub(crate) fn is_type(self) -> bool {
        self.self_check();
        Self::is_type_from_vtable(&self.vtable.vtable)
    }

    pub(crate) fn iter_item(self) -> Result<Ty, TypingNoContextError> {
        if Self::is_iterable(&self.vtable.vtable) {
            Ok(Ty::any())
        } else {
            Err(TypingNoContextError)
        }
    }

    /// Convert to runtime type matcher.
    pub(crate) fn matcher<T: TypeMatcherAlloc>(self, matcher: T) -> T::Result {
        self.self_check();

        // First handle special cases that can match faster than default matcher.
        // These are optimizations.
        if self.vtable.starlark_type_id == StarlarkTypeId::of::<StarlarkBigInt>() {
            matcher.int()
        } else if self.vtable.starlark_type_id == StarlarkTypeId::of::<StarlarkBool>() {
            matcher.bool()
        } else if self.vtable.starlark_type_id == StarlarkTypeId::of::<NoneType>() {
            matcher.none()
        } else if self.vtable.starlark_type_id == StarlarkTypeId::of::<StarlarkStr>() {
            matcher.str()
        } else {
            matcher.alloc(StarlarkTypeIdMatcher::new(self))
        }
    }

    pub(crate) fn fmt_with_config(
        &self,
        f: &mut fmt::Formatter<'_>,
        config: &TypeRenderConfig,
    ) -> fmt::Result {
        let type_name = match self.vtable.type_name {
            "string" => "str",
            "NoneType" => "None",
            name => name,
        };
        match config {
            TypeRenderConfig::Default => write!(f, "{}", type_name),
            TypeRenderConfig::LinkedType {
                render_linked_ty_starlark_value,
            } => {
                write!(f, "{}", render_linked_ty_starlark_value(self))
            }
        }
    }
}
