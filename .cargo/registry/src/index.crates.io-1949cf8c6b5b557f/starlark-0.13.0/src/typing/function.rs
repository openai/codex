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

use std::fmt::Debug;
use std::hash::Hash;

use allocative::Allocative;
use dupe::Dupe;

use crate::codemap::Span;
use crate::typing::call_args::TyCallArgs;
use crate::typing::callable::TyCallable;
use crate::typing::custom::TyCustomImpl;
use crate::typing::error::TypingNoContextError;
use crate::typing::error::TypingNoContextOrInternalError;
use crate::typing::error::TypingOrInternalError;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::typing::TypingBinOp;
use crate::typing::TypingOracleCtx;
use crate::values::typing::type_compiled::alloc::TypeMatcherAlloc;

/// Custom function typechecker.
pub trait TyCustomFunctionImpl:
    Debug + Eq + Ord + Hash + Allocative + Send + Sync + 'static
{
    fn is_type(&self) -> bool {
        false
    }

    fn validate_call(
        &self,
        span: Span,
        args: &TyCallArgs,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingOrInternalError>;

    fn as_callable(&self) -> TyCallable;

    fn as_function(&self) -> Option<&TyFunction> {
        None
    }
}

#[derive(
    Allocative,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Debug,
    derive_more::Display
)]
#[display(
    "def({}) -> {}",
    self.0.as_callable().params(),
    self.0.as_callable().result(),
)]
pub struct TyCustomFunction<F: TyCustomFunctionImpl>(pub F);

impl<F: TyCustomFunctionImpl> TyCustomImpl for TyCustomFunction<F> {
    fn as_name(&self) -> Option<&str> {
        Some("function")
    }

    fn validate_call(
        &self,
        span: Span,
        args: &TyCallArgs,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingOrInternalError> {
        self.0.validate_call(span, args, oracle)
    }

    fn as_callable(&self) -> Option<TyCallable> {
        Some(self.0.as_callable())
    }

    fn as_function(&self) -> Option<&TyFunction> {
        self.0.as_function()
    }

    fn bin_op(
        &self,
        bin_op: TypingBinOp,
        _rhs: &TyBasic,
        _ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        match bin_op {
            // `str | list`.
            TypingBinOp::BitOr if self.0.is_type() => Ok(Ty::basic(TyBasic::Type)),
            _ => Err(TypingNoContextOrInternalError::Typing),
        }
    }

    fn index(
        &self,
        _item: &TyBasic,
        _ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        // TODO(nga): this is hack for `enum` (type) which pretends to be a function.
        //   Should be a custom type.
        Ok(Ty::any())
    }

    fn attribute(&self, _attr: &str) -> Result<Ty, TypingNoContextError> {
        Err(TypingNoContextError)
    }

    fn matcher<T: TypeMatcherAlloc>(&self, factory: T) -> T::Result {
        factory.callable()
    }
}

/// A function.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Allocative)]
pub struct TyFunction {
    /// The `.type` property of the function, often `""`.
    pub(crate) type_attr: Option<Ty>,
    pub(crate) callable: TyCallable,
}

impl TyFunction {
    /// Constructor.
    pub fn new_with_type_attr(params: ParamSpec, result: Ty, type_attr: Ty) -> Self {
        // TODO(nga): validate params are in correct order.
        TyFunction {
            type_attr: Some(type_attr),
            callable: TyCallable::new(params, result),
        }
    }

    /// Constructor.
    pub fn new(params: ParamSpec, result: Ty) -> Self {
        TyFunction {
            type_attr: None,
            callable: TyCallable::new(params, result),
        }
    }

    /// Callable signature of the function.
    pub fn callable(&self) -> &TyCallable {
        &self.callable
    }
}

impl TyCustomFunctionImpl for TyFunction {
    fn is_type(&self) -> bool {
        self.type_attr.is_some()
    }

    fn validate_call(
        &self,
        span: Span,
        args: &TyCallArgs,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingOrInternalError> {
        oracle.validate_fn_call(span, &self.callable, args)
    }

    fn as_callable(&self) -> TyCallable {
        self.callable.dupe()
    }

    fn as_function(&self) -> Option<&TyFunction> {
        Some(self)
    }
}
