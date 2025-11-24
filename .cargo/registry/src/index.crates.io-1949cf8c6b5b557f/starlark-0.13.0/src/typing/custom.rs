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

use std::any;
use std::any::Any;
use std::any::TypeId;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use cmp_any::OrdAny;
use cmp_any::PartialEqAny;
use dupe::Dupe;
use starlark_map::StarlarkHasher;

use crate::codemap::Span;
use crate::typing::call_args::TyCallArgs;
use crate::typing::callable::TyCallable;
use crate::typing::error::InternalError;
use crate::typing::error::TypingNoContextError;
use crate::typing::error::TypingNoContextOrInternalError;
use crate::typing::error::TypingOrInternalError;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::typing::TyFunction;
use crate::typing::TypingBinOp;
use crate::typing::TypingOracleCtx;
use crate::values::typing::type_compiled::alloc::TypeMatcherAlloc;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::typing::type_compiled::factory::TypeCompiledFactory;
use crate::values::typing::type_compiled::matcher::TypeMatcherBox;
use crate::values::typing::type_compiled::matcher::TypeMatcherBoxAlloc;
use crate::values::Value;

/// Custom type implementation. [`Display`] must implement the representation of the type.
pub trait TyCustomImpl: Debug + Display + Hash + Ord + Allocative + Send + Sync + 'static {
    fn as_name(&self) -> Option<&str>;
    fn validate_call(
        &self,
        span: Span,
        _args: &TyCallArgs,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingOrInternalError> {
        Err(oracle.msg_error(span, format!("Value of type `{}` is not callable", self)))
    }
    /// Must override if implementing `validate_call`.
    fn as_callable(&self) -> Option<TyCallable> {
        None
    }
    fn as_function(&self) -> Option<&TyFunction> {
        None
    }
    fn bin_op(
        &self,
        bin_op: TypingBinOp,
        rhs: &TyBasic,
        ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        let _unused = (bin_op, rhs, ctx);
        Err(TypingNoContextOrInternalError::Typing)
    }
    fn iter_item(&self) -> Result<Ty, TypingNoContextError> {
        Err(TypingNoContextError)
    }
    fn index(
        &self,
        item: &TyBasic,
        ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        let _unused = (item, ctx);
        Err(TypingNoContextOrInternalError::Typing)
    }
    fn attribute(&self, attr: &str) -> Result<Ty, TypingNoContextError>;
    fn union2(x: Arc<Self>, other: Arc<Self>) -> Result<Arc<Self>, (Arc<Self>, Arc<Self>)> {
        if x == other { Ok(x) } else { Err((x, other)) }
    }
    fn intersects(x: &Self, y: &Self) -> bool {
        let _ignore = (x, y);
        true
    }
    /// Additional types that this type intersects with.
    fn intersects_with(&self, _other: &TyBasic) -> bool {
        false
    }

    /// Create runtime type matcher for values.
    fn matcher<T: TypeMatcherAlloc>(&self, factory: T) -> T::Result;
}

pub(crate) trait TyCustomDyn: Debug + Display + Allocative + Send + Sync + 'static {
    fn eq_token(&self) -> PartialEqAny;
    fn hash_code(&self) -> u64;
    fn cmp_token(&self) -> (OrdAny, &'static str);
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    fn as_any(&self) -> &dyn Any;

    fn as_name_dyn(&self) -> Option<&str>;
    fn validate_call_dyn(
        &self,
        span: Span,
        args: &TyCallArgs,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingOrInternalError>;
    fn is_intersects_with_dyn(&self, other: &TyBasic) -> bool;
    fn as_callable_dyn(&self) -> Option<TyCallable>;
    fn as_function_dyn(&self) -> Option<&TyFunction>;
    fn iter_item_dyn(&self) -> Result<Ty, TypingNoContextError>;
    fn index_dyn(
        &self,
        index: &TyBasic,
        ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError>;
    fn attribute_dyn(&self, attr: &str) -> Result<Ty, TypingNoContextError>;
    fn bin_op_dyn(
        &self,
        bin_op: TypingBinOp,
        rhs: &TyBasic,
        ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError>;
    fn union2_dyn(
        self: Arc<Self>,
        other: Arc<dyn TyCustomDyn>,
    ) -> Result<Arc<dyn TyCustomDyn>, (Arc<dyn TyCustomDyn>, Arc<dyn TyCustomDyn>)>;
    fn intersects_dyn(&self, other: &dyn TyCustomDyn) -> bool;

    fn matcher_with_type_compiled_factory_dyn<'v>(
        &self,
        type_compiled_factory: TypeCompiledFactory<'_, 'v>,
    ) -> TypeCompiled<Value<'v>>;

    fn matcher_box_dyn(&self) -> TypeMatcherBox;
}

impl<T: TyCustomImpl> TyCustomDyn for T {
    fn eq_token(&self) -> PartialEqAny {
        PartialEqAny::new(self)
    }

    fn hash_code(&self) -> u64 {
        let mut hasher = StarlarkHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    fn cmp_token(&self) -> (OrdAny, &'static str) {
        (OrdAny::new(self), any::type_name::<Self>())
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_name_dyn(&self) -> Option<&str> {
        self.as_name()
    }

    fn validate_call_dyn(
        &self,
        span: Span,
        args: &TyCallArgs,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingOrInternalError> {
        self.validate_call(span, args, oracle)
    }

    fn as_callable_dyn(&self) -> Option<TyCallable> {
        self.as_callable()
    }

    fn is_intersects_with_dyn(&self, other: &TyBasic) -> bool {
        self.intersects_with(other)
    }

    fn as_function_dyn(&self) -> Option<&TyFunction> {
        self.as_function()
    }

    fn attribute_dyn(&self, attr: &str) -> Result<Ty, TypingNoContextError> {
        self.attribute(attr)
    }

    fn iter_item_dyn(&self) -> Result<Ty, TypingNoContextError> {
        self.iter_item()
    }

    fn index_dyn(
        &self,
        index: &TyBasic,
        ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        self.index(index, ctx)
    }

    fn bin_op_dyn(
        &self,
        bin_op: TypingBinOp,
        rhs: &TyBasic,
        ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        self.bin_op(bin_op, rhs, ctx)
    }

    fn union2_dyn(
        self: Arc<Self>,
        other: Arc<dyn TyCustomDyn>,
    ) -> Result<Arc<dyn TyCustomDyn>, (Arc<dyn TyCustomDyn>, Arc<dyn TyCustomDyn>)> {
        if TypeId::of::<Self>() == other.eq_token().type_id() {
            let other: Arc<Self> = Arc::downcast(other.into_any()).unwrap();
            T::union2(self, other)
                .map::<Arc<dyn TyCustomDyn>, _>(|x| x)
                .map_err::<(Arc<dyn TyCustomDyn>, Arc<dyn TyCustomDyn>), _>(|(x, y)| (x, y))
        } else {
            Err((self, other))
        }
    }

    fn intersects_dyn(&self, other: &dyn TyCustomDyn) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            T::intersects(self, other)
        } else {
            false
        }
    }

    fn matcher_with_type_compiled_factory_dyn<'v>(
        &self,
        type_compiled_factory: TypeCompiledFactory<'_, 'v>,
    ) -> TypeCompiled<Value<'v>> {
        self.matcher(type_compiled_factory)
    }

    fn matcher_box_dyn(&self) -> TypeMatcherBox {
        self.matcher(TypeMatcherBoxAlloc)
    }
}

#[derive(Debug, derive_more::Display, Allocative, Clone, Dupe)]
pub struct TyCustom(pub(crate) Arc<dyn TyCustomDyn>);

impl TyCustom {
    pub(crate) fn new<T: TyCustomImpl>(ty: T) -> Self {
        Self(Arc::new(ty))
    }

    pub(crate) fn as_name(&self) -> Option<&str> {
        self.0.as_name_dyn()
    }

    pub(crate) fn union2(x: TyCustom, y: TyCustom) -> Result<TyCustom, (TyCustom, TyCustom)> {
        x.0.union2_dyn(y.0)
            .map(TyCustom)
            .map_err(|(x, y)| (TyCustom(x), TyCustom(y)))
    }

    pub(crate) fn intersects(x: &TyCustom, y: &TyCustom) -> bool {
        x.0.intersects_dyn(&*y.0)
    }

    pub(crate) fn intersects_with(
        &self,
        other: &TyBasic,
        ctx: TypingOracleCtx,
    ) -> Result<bool, InternalError> {
        if self.0.is_intersects_with_dyn(other) {
            return Ok(true);
        }
        match other {
            TyBasic::Custom(other) => Ok(Self::intersects(self, other)),
            TyBasic::Callable(c) => {
                if let Some(this) = self.0.as_callable_dyn() {
                    ctx.callables_intersect(&this, c)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    pub(crate) fn matcher_with_type_compiled_factory<'v>(
        &self,
        type_compiled_factory: TypeCompiledFactory<'_, 'v>,
    ) -> TypeCompiled<Value<'v>> {
        self.0
            .matcher_with_type_compiled_factory_dyn(type_compiled_factory)
    }

    pub(crate) fn matcher_with_box(&self) -> TypeMatcherBox {
        self.0.matcher_box_dyn()
    }
}

impl PartialEq for TyCustom {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_token() == other.0.eq_token()
    }
}

impl Eq for TyCustom {}

impl Hash for TyCustom {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash_code().hash(state)
    }
}

impl PartialOrd for TyCustom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TyCustom {
    fn cmp(&self, other: &Self) -> Ordering {
        let (a_cmp, a_type_name) = self.0.cmp_token();
        let (b_cmp, b_type_name) = other.0.cmp_token();

        // Type ids are comparable, but we want comparison independent of hashing.
        if OrdAny::type_id(&a_cmp) != OrdAny::type_id(&b_cmp) {
            let type_name_cmp = a_type_name.cmp(b_type_name);
            if type_name_cmp != Ordering::Equal {
                return type_name_cmp;
            }

            // This is unreachable: if the type names are the same,
            // the type ids should be the same.
        }

        a_cmp.cmp(&b_cmp)
    }
}
