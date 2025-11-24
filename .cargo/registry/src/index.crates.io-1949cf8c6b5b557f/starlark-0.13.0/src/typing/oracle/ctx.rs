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

use std::fmt::Display;
use std::iter;

use dupe::Dupe;
use starlark_map::small_map::SmallMap;
use starlark_syntax::syntax::ast::BinOp;

use crate::codemap::CodeMap;
use crate::codemap::Span;
use crate::codemap::Spanned;
use crate::typing::basic::TyBasic;
use crate::typing::call_args::TyCallArgs;
use crate::typing::callable::TyCallable;
use crate::typing::callable_param::ParamIsRequired;
use crate::typing::callable_param::ParamMode;
use crate::typing::error::InternalError;
use crate::typing::error::TypingError;
use crate::typing::error::TypingNoContextError;
use crate::typing::error::TypingNoContextOrInternalError;
use crate::typing::error::TypingOrInternalError;
use crate::typing::starlark_value::TyStarlarkValue;
use crate::typing::tuple::TyTuple;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::typing::TypingBinOp;
use crate::typing::TypingUnOp;
use crate::values::dict::value::MutableDict;
use crate::values::list::value::List;
use crate::values::set::value::MutableSet;
use crate::values::tuple::value::Tuple;

#[derive(Debug, thiserror::Error)]
enum TypingOracleCtxError {
    #[error("Expected type `{require}` but got `{got}`")]
    IncompatibleType { got: String, require: String },
    #[error("Call to a non-callable type `{ty}`")]
    CallToNonCallable { ty: String },
    #[error("Missing required parameter `{name}`")]
    MissingRequiredParameter { name: String },
    #[error("Unexpected parameter named `{name}`")]
    UnexpectedNamedArgument { name: String },
    #[error("Too many positional arguments")]
    TooManyPositionalArguments,
    #[error("Call arguments incompatible, fn type is `{fun}`")]
    CallArgumentsIncompatible { fun: Ty },
    #[error("Type `{ty}` does not have [] operator or [] cannot accept `{index}`")]
    MissingIndexOperator { ty: Ty, index: Ty },
    #[error("Type `{ty}` does not have [::] operator")]
    MissingSliceOperator { ty: Ty },
    #[error("The attribute `{attr}` is not available on the type `{ty}`")]
    AttributeNotAvailable { ty: Ty, attr: String },
    #[error("Type `{ty}` is not iterable")]
    NotIterable { ty: Ty },
    #[error("Unary operator `{un_op}` is not available on the type `{ty}`")]
    UnaryOperatorNotAvailable { ty: Ty, un_op: TypingUnOp },
    #[error("Binary operator `{bin_op}` is not available on the types `{left}` and `{right}`")]
    BinaryOperatorNotAvailable {
        bin_op: TypingBinOp,
        left: Ty,
        right: Ty,
    },
}

/// Oracle reference with utility methods.
///
/// This type is stateless.
#[derive(Clone, Copy, Dupe)]
pub struct TypingOracleCtx<'a> {
    pub(crate) codemap: &'a CodeMap,
}

impl<'a> TypingOracleCtx<'a> {
    pub(crate) fn mk_error(&self, span: Span, err: impl Into<anyhow::Error>) -> TypingError {
        TypingError::new_anyhow(err.into(), span, self.codemap)
    }

    pub(crate) fn mk_error_as_maybe_internal(
        &self,
        span: Span,
        err: impl Into<anyhow::Error>,
    ) -> TypingOrInternalError {
        TypingOrInternalError::Typing(TypingError::new_anyhow(err.into(), span, self.codemap))
    }

    pub(crate) fn msg_error(&self, span: Span, msg: impl Display) -> TypingOrInternalError {
        TypingOrInternalError::Typing(TypingError::msg(msg, span, self.codemap))
    }

    /// If I do `self[i]` what will the resulting type be.
    pub(crate) fn indexed_basic(&self, ty: &TyBasic, i: usize) -> Ty {
        match ty {
            TyBasic::Any => Ty::any(),
            TyBasic::List(x) => x.to_ty(),
            TyBasic::Tuple(xs) => xs.get(i).cloned().unwrap_or(Ty::never()),
            // Not exactly sure what we should do here
            _ => Ty::any(),
        }
    }

    /// If I do `self[i]` what will the resulting type be.
    pub(crate) fn indexed(&self, ty: &Ty, i: usize) -> Ty {
        Ty::unions(
            ty.iter_union()
                .iter()
                .map(|x| self.indexed_basic(x, i))
                .collect(),
        )
    }

    pub(crate) fn validate_type(
        &self,
        got: Spanned<&Ty>,
        require: &Ty,
    ) -> Result<(), TypingOrInternalError> {
        if !self.intersects(got.node, require)? {
            Err(self.mk_error_as_maybe_internal(
                got.span,
                TypingOracleCtxError::IncompatibleType {
                    got: got.to_string(),
                    require: require.to_string(),
                },
            ))
        } else {
            Ok(())
        }
    }

    #[allow(clippy::redundant_pattern_matching)]
    fn validate_args(
        &self,
        params: &ParamSpec,
        args: &TyCallArgs,
        span: Span,
    ) -> Result<(), TypingOrInternalError> {
        // Want to figure out which arguments go in which positions
        let mut param_args: Vec<Vec<Spanned<&Ty>>> = vec![vec![]; params.params().len()];
        // The next index a positional parameter might fill
        let mut param_pos = 0;
        let mut seen_vargs = false;

        let TyCallArgs {
            pos: args_pos,
            named: args_named,
            args: args_args,
            kwargs: args_kwargs,
        } = args;
        for ty in args_pos {
            loop {
                match params.params().get(param_pos) {
                    None => {
                        return Err(self.mk_error_as_maybe_internal(
                            ty.span,
                            TypingOracleCtxError::TooManyPositionalArguments,
                        ));
                    }
                    Some(param) => {
                        let found_index = param_pos;
                        if param.mode != ParamMode::Args {
                            param_pos += 1;
                        }
                        if param.allows_pos() {
                            param_args[found_index].push(ty.as_ref());
                            break;
                        }
                    }
                }
            }
        }
        for arg in args_named {
            let (name, ty) = &arg.node;
            let mut success = false;
            for (i, param) in params.params().iter().enumerate() {
                if param.name() == Some(*name) || param.mode == ParamMode::Kwargs {
                    param_args[i].push(Spanned {
                        span: arg.span,
                        node: ty,
                    });
                    success = true;
                    break;
                }
            }
            if !success {
                return Err(self.mk_error_as_maybe_internal(
                    arg.span,
                    TypingOracleCtxError::UnexpectedNamedArgument {
                        name: (*name).to_owned(),
                    },
                ));
            }
        }
        if let Some(_) = args_args {
            seen_vargs = true;
        }
        if let Some(_) = args_kwargs {
            seen_vargs = true;
        }

        for (param, args) in iter::zip(params.params(), param_args) {
            match param.mode {
                ParamMode::PosOnly(req)
                | ParamMode::PosOrName(_, req)
                | ParamMode::NameOnly(_, req) => match args.as_slice() {
                    [] => {
                        if req == ParamIsRequired::Yes && !seen_vargs {
                            return Err(self.mk_error_as_maybe_internal(
                                span,
                                TypingOracleCtxError::MissingRequiredParameter {
                                    name: param.name_display().to_owned(),
                                },
                            ));
                        }
                    }
                    [arg] => self.validate_type(*arg, &param.ty)?,
                    [_, _, ..] => {
                        return Err(TypingOrInternalError::Internal(InternalError::msg(
                            "Multiple arguments bound to parameter",
                            span,
                            self.codemap,
                        )));
                    }
                },
                ParamMode::Args => {
                    for ty in args {
                        // For an arg, we require the type annotation to be inner value,
                        // rather than the outer (which is always a tuple)
                        self.validate_type(ty, &param.ty)?;
                    }
                }
                ParamMode::Kwargs => {
                    for ty in args {
                        self.validate_type(ty, &param.ty)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_fn_call(
        &self,
        span: Span,
        fun: &TyCallable,
        args: &TyCallArgs,
    ) -> Result<Ty, TypingOrInternalError> {
        self.validate_args(fun.params(), args, span)?;
        Ok(fun.result().dupe())
    }

    #[allow(clippy::collapsible_else_if)]
    fn validate_call_basic(
        &self,
        span: Span,
        fun: &TyBasic,
        args: &TyCallArgs,
    ) -> Result<Ty, TypingOrInternalError> {
        match fun {
            TyBasic::Any => Ok(Ty::any()),
            TyBasic::StarlarkValue(t) => Ok(t.validate_call(span, *self)?),
            TyBasic::List(_) | TyBasic::Dict(..) | TyBasic::Tuple(_) | TyBasic::Set(_) => Err(self
                .mk_error_as_maybe_internal(
                    span,
                    TypingOracleCtxError::CallToNonCallable {
                        ty: fun.to_string(),
                    },
                )),
            TyBasic::Iter(_) | TyBasic::Type => {
                // Unknown type, may be callable.
                Ok(Ty::any())
            }
            TyBasic::Callable(c) => c.validate_call(span, args, *self),
            TyBasic::Custom(t) => t.0.validate_call_dyn(span, args, *self),
        }
    }

    #[allow(clippy::collapsible_else_if)]
    pub(crate) fn validate_call(
        &self,
        span: Span,
        fun: &Ty,
        args: &TyCallArgs,
    ) -> Result<Ty, TypingOrInternalError> {
        if fun.is_any() || fun.is_never() {
            return Ok(fun.dupe());
        }

        let mut successful = Vec::new();
        let mut errors: Vec<TypingError> = Vec::new();
        for variant in fun.iter_union() {
            match self.validate_call_basic(span, variant, args) {
                Ok(ty) => successful.push(ty),
                Err(TypingOrInternalError::Typing(e)) => errors.push(e),
                Err(TypingOrInternalError::Internal(e)) => {
                    return Err(TypingOrInternalError::Internal(e));
                }
            }
        }
        if !successful.is_empty() {
            Ok(Ty::unions(successful))
        } else {
            if errors.len() == 1 {
                Err(TypingOrInternalError::Typing(errors.pop().unwrap()))
            } else {
                Err(self.mk_error_as_maybe_internal(
                    span,
                    TypingOracleCtxError::CallArgumentsIncompatible { fun: fun.dupe() },
                ))
            }
        }
    }

    fn iter_item_basic(&self, ty: &TyBasic) -> Result<Ty, TypingNoContextError> {
        match ty {
            TyBasic::Any => Ok(Ty::any()),
            TyBasic::StarlarkValue(ty) => ty.iter_item(),
            TyBasic::List(item) => Ok((**item).dupe()),
            TyBasic::Dict(k, _v) => Ok((**k).dupe()),
            TyBasic::Tuple(tuple) => Ok(tuple.item_ty()),
            TyBasic::Callable(_) => Ok(Ty::any()),
            TyBasic::Type => Ok(Ty::any()),
            TyBasic::Iter(ty) => Ok(ty.to_ty()),
            TyBasic::Custom(ty) => ty.0.iter_item_dyn(),
            TyBasic::Set(item) => Ok((**item).dupe()),
        }
    }

    /// Item type of an iterable.
    pub(crate) fn iter_item(&self, iter: Spanned<&Ty>) -> Result<Ty, TypingError> {
        match iter.typecheck_union_simple(|basic| self.iter_item_basic(basic)) {
            Ok(ty) => Ok(ty),
            Err(TypingNoContextError) => Err(self.mk_error(
                iter.span,
                TypingOracleCtxError::NotIterable {
                    ty: iter.node.clone(),
                },
            )),
        }
    }

    fn expr_index_ty(
        &self,
        array: &TyBasic,
        index: Spanned<&TyBasic>,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        match array {
            TyBasic::Any | TyBasic::Callable(_) | TyBasic::Iter(_) | TyBasic::Type => Ok(Ty::any()),
            TyBasic::Tuple(tuple) => {
                if !self.intersects_basic(index.node, &TyBasic::int())? {
                    return Err(TypingNoContextOrInternalError::Typing);
                }
                Ok(tuple.item_ty())
            }
            TyBasic::List(item) => {
                if !self.intersects_basic(index.node, &TyBasic::int())? {
                    return Err(TypingNoContextOrInternalError::Typing);
                }
                Ok((**item).dupe())
            }
            TyBasic::Dict(k, v) => {
                if !self.intersects(&Ty::basic(index.node.dupe()), k)? {
                    return Err(TypingNoContextOrInternalError::Typing);
                }
                Ok((**v).dupe())
            }
            TyBasic::Set(item) => {
                if !self.intersects(&Ty::basic(index.node.dupe()), item)? {
                    return Err(TypingNoContextOrInternalError::Typing);
                }
                Ok((**item).dupe())
            }
            TyBasic::StarlarkValue(array) => Ok(array.index(index.node)?),
            TyBasic::Custom(c) => Ok(c.0.index_dyn(index.node, self)?),
        }
    }

    pub(crate) fn expr_index(
        &self,
        span: Span,
        array: Ty,
        index: Spanned<Ty>,
    ) -> Result<Ty, TypingOrInternalError> {
        if array.is_any() || array.is_never() {
            return Ok(array);
        }
        if index.is_never() {
            return Ok(Ty::never());
        }

        let mut good = Vec::new();
        for array in array.iter_union() {
            for index_basic in index.node.iter_union() {
                match self.expr_index_ty(
                    array,
                    Spanned {
                        span: index.span,
                        node: index_basic,
                    },
                ) {
                    Ok(ty) => {
                        good.push(ty);
                    }
                    Err(TypingNoContextOrInternalError::Internal(e)) => {
                        return Err(TypingOrInternalError::Internal(e));
                    }
                    Err(TypingNoContextOrInternalError::Typing) => {}
                }
            }
        }

        if good.is_empty() {
            // TODO: message is wrong: it is possible that there's index operator,
            //   but it does not support parameter type.
            //   But we don't support that.
            Err(self.mk_error_as_maybe_internal(
                span,
                TypingOracleCtxError::MissingIndexOperator {
                    ty: array,
                    index: index.node,
                },
            ))
        } else {
            Ok(Ty::unions(good))
        }
    }

    fn expr_slice_basic(&self, array: &TyBasic) -> Result<Ty, TypingNoContextError> {
        if let TyBasic::StarlarkValue(v) = array {
            v.slice()
        } else if array.is_tuple() || array.is_list() {
            Ok(Ty::basic(array.dupe()))
        } else {
            Err(TypingNoContextError)
        }
    }

    pub(crate) fn expr_slice(&self, span: Span, array: Ty) -> Result<Ty, TypingError> {
        match array.typecheck_union_simple(|basic| self.expr_slice_basic(basic)) {
            Ok(ty) => Ok(ty),
            Err(TypingNoContextError) => Err(self.mk_error(
                span,
                TypingOracleCtxError::MissingSliceOperator { ty: array },
            )),
        }
    }

    fn expr_dot_basic(&self, array: &TyBasic, attr: &str) -> Result<Ty, TypingNoContextError> {
        match array {
            TyBasic::Any | TyBasic::Callable(_) | TyBasic::Iter(_) | TyBasic::Type => Ok(Ty::any()),
            TyBasic::StarlarkValue(s) => s.attr(attr),
            TyBasic::Tuple(_) => Err(TypingNoContextError),
            TyBasic::List(elem) => match attr {
                "pop" => Ok(Ty::function(
                    ParamSpec::pos_only([], [Ty::int()]),
                    (**elem).dupe(),
                )),
                "index" => Ok(Ty::function(
                    ParamSpec::pos_only([(**elem).dupe()], [Ty::int()]),
                    Ty::int(),
                )),
                "remove" => Ok(Ty::function(
                    ParamSpec::pos_only([(**elem).dupe()], []),
                    Ty::none(),
                )),
                attr => TyStarlarkValue::new::<List>().attr(attr),
            },
            TyBasic::Dict(tk, tv) => {
                match attr {
                    "get" => Ok(Ty::union2(
                        Ty::function(
                            ParamSpec::pos_only([tk.to_ty()], []),
                            Ty::union2(tv.to_ty(), Ty::none()),
                        ),
                        // This second signature is a bit too lax, but get with a default is much rarer
                        Ty::function(ParamSpec::pos_only([tk.to_ty(), Ty::any()], []), Ty::any()),
                    )),
                    "keys" => Ok(Ty::function(
                        ParamSpec::empty(),
                        Ty::basic(TyBasic::List(tk.dupe())),
                    )),
                    "values" => Ok(Ty::function(
                        ParamSpec::empty(),
                        Ty::basic(TyBasic::List(tv.dupe())),
                    )),
                    "items" => Ok(Ty::function(
                        ParamSpec::empty(),
                        Ty::list(Ty::tuple(vec![tk.to_ty(), tv.to_ty()])),
                    )),
                    "popitem" => Ok(Ty::function(
                        ParamSpec::empty(),
                        Ty::tuple(vec![tk.to_ty(), tv.to_ty()]),
                    )),
                    attr => TyStarlarkValue::new::<MutableDict>().attr(attr),
                }
            }
            TyBasic::Custom(custom) => custom.0.attribute_dyn(attr),
            //TODO(romanp) add match on attr similar to Dict
            TyBasic::Set(_) => TyStarlarkValue::new::<MutableSet>().attr(attr),
        }
    }

    pub(crate) fn expr_dot(&self, span: Span, array: &Ty, attr: &str) -> Result<Ty, TypingError> {
        match array.typecheck_union_simple(|basic| self.expr_dot_basic(basic, attr)) {
            Ok(x) => Ok(x),
            Err(TypingNoContextError) => Err(self.mk_error(
                span,
                TypingOracleCtxError::AttributeNotAvailable {
                    ty: array.clone(),
                    attr: attr.to_owned(),
                },
            )),
        }
    }

    fn expr_un_op_basic(
        &self,
        ty: &TyBasic,
        un_op: TypingUnOp,
    ) -> Result<Ty, TypingNoContextError> {
        match ty {
            TyBasic::StarlarkValue(ty) => match ty.un_op(un_op) {
                Ok(x) => Ok(Ty::basic(TyBasic::StarlarkValue(x))),
                Err(TypingNoContextError) => Err(TypingNoContextError),
            },
            _ => Err(TypingNoContextError),
        }
    }

    pub(crate) fn expr_un_op(
        &self,
        span: Span,
        ty: Ty,
        un_op: TypingUnOp,
    ) -> Result<Ty, TypingError> {
        match ty.typecheck_union_simple(|basic| self.expr_un_op_basic(basic, un_op)) {
            Ok(ty) => Ok(ty),
            Err(TypingNoContextError) => Err(self.mk_error(
                span,
                TypingOracleCtxError::UnaryOperatorNotAvailable { ty, un_op },
            )),
        }
    }

    fn expr_bin_op_ty_basic_lhs(
        &self,
        lhs: &TyBasic,
        bin_op: TypingBinOp,
        rhs: Spanned<&TyBasic>,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        match lhs {
            TyBasic::Any | TyBasic::Iter(_) | TyBasic::Callable(_) | TyBasic::Type => Ok(Ty::any()),
            TyBasic::StarlarkValue(lhs) => Ok(lhs.bin_op(bin_op, rhs.node)?),
            lhs @ TyBasic::List(elem) => match bin_op {
                TypingBinOp::Less => {
                    if self.intersects_basic(lhs, rhs.node)? {
                        Ok(Ty::bool())
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                TypingBinOp::In => {
                    if self.intersects(elem, &Ty::basic(rhs.node.dupe()))? {
                        Ok(Ty::bool())
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                TypingBinOp::Add => {
                    if self.intersects_basic(rhs.node, &TyBasic::any_list())? {
                        Ok(Ty::list(Ty::union2(
                            elem.to_ty(),
                            self.iter_item_basic(rhs.node)?,
                        )))
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                TypingBinOp::Mul => {
                    if self.intersects_basic(rhs.node, &TyBasic::int())? {
                        Ok(Ty::basic(lhs.dupe()))
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                _ => Ok(TyStarlarkValue::new::<List>().bin_op(bin_op, rhs.node)?),
            },
            TyBasic::Tuple(_) => {
                // TODO(nga): can do better types.
                Ok(TyStarlarkValue::new::<Tuple>().bin_op(bin_op, rhs.node)?)
            }
            TyBasic::Dict(k, v) => match bin_op {
                TypingBinOp::BitOr => {
                    if self.intersects_basic(rhs.node, &TyBasic::any_dict())? {
                        Ok(Ty::union2(
                            Ty::dict(k.to_ty(), v.to_ty()),
                            Ty::basic(rhs.node.dupe()),
                        ))
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                TypingBinOp::In => {
                    if self.intersects(&Ty::basic(rhs.node.dupe()), k)? {
                        Ok(Ty::bool())
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                bin_op => Ok(TyStarlarkValue::new::<MutableDict>().bin_op(bin_op, rhs.node)?),
            },
            TyBasic::Custom(lhs) => Ok(lhs.0.bin_op_dyn(bin_op, rhs.node, self)?),
            TyBasic::Set(elem) => match bin_op {
                TypingBinOp::In => {
                    if self.intersects(&Ty::basic(rhs.node.dupe()), elem)? {
                        Ok(Ty::bool())
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                TypingBinOp::BitXor
                | TypingBinOp::BitAnd
                | TypingBinOp::Sub
                | TypingBinOp::BitOr => {
                    if self.intersects_basic(rhs.node, &TyBasic::any_set())? {
                        Ok(Ty::union2(
                            Ty::set(elem.to_ty()),
                            Ty::basic(rhs.node.dupe()),
                        ))
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                bin_op => Ok(TyStarlarkValue::new::<MutableSet>().bin_op(bin_op, rhs.node)?),
            },
        }
    }

    fn expr_bin_op_ty_basic_rhs(
        &self,
        lhs: &TyBasic,
        bin_op: TypingBinOp,
        rhs: &TyBasic,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        match rhs {
            TyBasic::StarlarkValue(rhs) => Ok(rhs.rbin_op(bin_op, lhs)?),
            rhs @ TyBasic::List(_) => match bin_op {
                TypingBinOp::Mul => {
                    if self.intersects_basic(lhs, &TyBasic::int())? {
                        Ok(Ty::basic(rhs.clone()))
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                _ => Ok(TyStarlarkValue::new::<List>().rbin_op(bin_op, lhs)?),
            },
            TyBasic::Tuple(_) => match bin_op {
                TypingBinOp::Mul => {
                    if self.intersects_basic(lhs, &TyBasic::int())? {
                        Ok(Ty::any_tuple())
                    } else {
                        Err(TypingNoContextOrInternalError::Typing)
                    }
                }
                _ => Ok(TyStarlarkValue::tuple().rbin_op(bin_op, lhs)?),
            },
            _ => Err(TypingNoContextOrInternalError::Typing),
        }
    }

    fn expr_bin_op_ty_basic(
        &self,
        span: Span,
        lhs: Spanned<&TyBasic>,
        bin_op: TypingBinOp,
        rhs: Spanned<&TyBasic>,
    ) -> Result<Ty, TypingOrInternalError> {
        if let TyBasic::Any = lhs.node {
            return Ok(Ty::any());
        }

        if let Ok(r) = self.expr_bin_op_ty_basic_lhs(lhs.node, bin_op, rhs) {
            return Ok(r);
        }
        if let Ok(r) = self.expr_bin_op_ty_basic_rhs(lhs.node, bin_op, rhs.node) {
            return Ok(r);
        }

        Err(self.mk_error_as_maybe_internal(
            span,
            TypingOracleCtxError::BinaryOperatorNotAvailable {
                bin_op,
                left: Ty::basic(lhs.node.clone()),
                right: Ty::basic(rhs.node.clone()),
            },
        ))
    }

    pub(crate) fn expr_bin_op_ty(
        &self,
        span: Span,
        lhs: Spanned<Ty>,
        bin_op: TypingBinOp,
        rhs: Spanned<Ty>,
    ) -> Result<Ty, TypingOrInternalError> {
        if lhs.is_never() || rhs.is_never() {
            // TODO(nga): even if RHS is never, it still can be an error
            //   if LHS does not support bin op.
            match bin_op {
                bin_op if bin_op.always_bool() => return Ok(Ty::bool()),
                _ => return Ok(Ty::never()),
            }
        }

        let mut good = Vec::new();
        for lhs_i in lhs.node.iter_union() {
            for rhs_i in rhs.node.iter_union() {
                let lhs_i = Spanned {
                    span: lhs.span,
                    node: lhs_i,
                };
                let rhs_i = Spanned {
                    span: rhs.span,
                    node: rhs_i,
                };
                match self.expr_bin_op_ty_basic(span, lhs_i, bin_op, rhs_i) {
                    Ok(ty) => good.push(ty),
                    Err(TypingOrInternalError::Internal(e)) => {
                        return Err(TypingOrInternalError::Internal(e));
                    }
                    Err(TypingOrInternalError::Typing(_e)) => {
                        // TODO(nga): keep the error if it is the only one error.
                    }
                }
            }
        }

        if good.is_empty() {
            Err(self.mk_error_as_maybe_internal(
                span,
                TypingOracleCtxError::BinaryOperatorNotAvailable {
                    left: lhs.node,
                    right: rhs.node,
                    bin_op,
                },
            ))
        } else {
            match bin_op {
                bin_op if bin_op.always_bool() => Ok(Ty::bool()),
                _ => Ok(Ty::unions(good)),
            }
        }
    }

    pub(crate) fn expr_bin_op(
        &self,
        span: Span,
        lhs: Spanned<Ty>,
        bin_op: BinOp,
        rhs: Spanned<Ty>,
    ) -> Result<Ty, TypingOrInternalError> {
        let bool_ret = if lhs.is_never() || rhs.is_never() {
            Ty::never()
        } else {
            Ty::bool()
        };
        match bin_op {
            BinOp::And | BinOp::Or => {
                if lhs.is_never() {
                    Ok(Ty::never())
                } else {
                    Ok(Ty::union2(lhs.node, rhs.node))
                }
            }
            BinOp::Equal | BinOp::NotEqual => {
                // It's not an error to compare two different types, but it is pointless
                self.validate_type(rhs.as_ref(), &lhs)?;
                Ok(bool_ret)
            }
            BinOp::In | BinOp::NotIn => {
                // We dispatch `x in y` as y.__in__(x) as that's how we validate
                self.expr_bin_op_ty(span, rhs, TypingBinOp::In, lhs)
            }
            BinOp::Less | BinOp::LessOrEqual | BinOp::Greater | BinOp::GreaterOrEqual => {
                self.expr_bin_op_ty(span, lhs, TypingBinOp::Less, rhs)
            }
            BinOp::Subtract => self.expr_bin_op_ty(span, lhs, TypingBinOp::Sub, rhs),
            BinOp::Add => self.expr_bin_op_ty(span, lhs, TypingBinOp::Add, rhs),
            BinOp::Multiply => self.expr_bin_op_ty(span, lhs, TypingBinOp::Mul, rhs),
            BinOp::Percent => self.expr_bin_op_ty(span, lhs, TypingBinOp::Percent, rhs),
            BinOp::Divide => self.expr_bin_op_ty(span, lhs, TypingBinOp::Div, rhs),
            BinOp::FloorDivide => self.expr_bin_op_ty(span, lhs, TypingBinOp::FloorDiv, rhs),
            BinOp::BitAnd => self.expr_bin_op_ty(span, lhs, TypingBinOp::BitAnd, rhs),
            BinOp::BitOr => self.expr_bin_op_ty(span, lhs, TypingBinOp::BitOr, rhs),
            BinOp::BitXor => self.expr_bin_op_ty(span, lhs, TypingBinOp::BitXor, rhs),
            BinOp::LeftShift => self.expr_bin_op_ty(span, lhs, TypingBinOp::LeftShift, rhs),
            BinOp::RightShift => self.expr_bin_op_ty(span, lhs, TypingBinOp::RightShift, rhs),
        }
    }

    /// Returns false on Void, since that is definitely not a list
    pub(crate) fn probably_a_list(&self, ty: &Ty) -> Result<bool, InternalError> {
        if ty.is_never() {
            return Ok(false);
        }
        self.intersects(ty, &Ty::list(Ty::any()))
    }

    /// If you get to a point where these types are being checked, might they succeed
    pub(crate) fn intersects(&self, xs: &Ty, ys: &Ty) -> Result<bool, InternalError> {
        if xs.is_any() || xs.is_never() || ys.is_any() || ys.is_never() {
            return Ok(true);
        }

        for x in xs.iter_union() {
            for y in ys.iter_union() {
                if self.intersects_basic(x, y)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn intersects_basic(&self, x: &TyBasic, y: &TyBasic) -> Result<bool, InternalError> {
        Ok(x == y || self.intersects_one_side(x, y)? || self.intersects_one_side(y, x)?)
    }

    fn params_intersect(&self, x: &ParamSpec, y: &ParamSpec) -> Result<bool, InternalError> {
        // Fast path.
        if x == y {
            return Ok(true);
        }
        // Another fast path.
        if x.is_any() || y.is_any() {
            return Ok(true);
        }
        match (
            x.all_required_pos_only_named_only(),
            y.all_required_pos_only_named_only(),
        ) {
            (Some((x_p, x_n)), Some((y_p, y_n))) => {
                if x_p.len() != y_p.len() || x_n.len() != y_n.len() {
                    return Ok(false);
                }
                for (x, y) in x_p.iter().zip(y_p.iter()) {
                    if !self.intersects(x, y)? {
                        return Ok(false);
                    }
                }
                let y_n = SmallMap::from_iter(y_n);
                for (name, x) in x_n {
                    if let Some(y) = y_n.get(name) {
                        if !self.intersects(x, y)? {
                            return Ok(false);
                        }
                    } else {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Some((x_p, x_n)), None) => {
                self.params_all_pos_only_named_only_intersect(&x_p, &x_n, y)
            }
            (None, Some((y_p, y_n))) => {
                self.params_all_pos_only_named_only_intersect(&y_p, &y_n, x)
            }
            _ => {
                // The rest is hard to check, but required pos-only in signatures
                // is what we need the most.
                Ok(true)
            }
        }
    }

    fn params_all_pos_only_named_only_intersect(
        &self,
        x_p: &[&Ty],
        x_n: &[(&str, &Ty)],
        y: &ParamSpec,
    ) -> Result<bool, InternalError> {
        match self.validate_args(
            y,
            &TyCallArgs {
                pos: x_p
                    .iter()
                    .map(|ty| Spanned {
                        node: (*ty).dupe(),
                        // TODO(nga): proper span.
                        span: Span::default(),
                    })
                    .collect(),
                named: x_n
                    .iter()
                    .map(|(name, ty)| Spanned {
                        node: (*name, (*ty).dupe()),
                        // TODO(nga): proper span.
                        span: Span::default(),
                    })
                    .collect(),
                args: None,
                kwargs: None,
            },
            Span::default(),
        ) {
            Ok(()) => Ok(true),
            Err(TypingOrInternalError::Internal(e)) => Err(e),
            Err(TypingOrInternalError::Typing(_)) => Ok(false),
        }
    }

    pub(crate) fn callables_intersect(
        &self,
        x: &TyCallable,
        y: &TyCallable,
    ) -> Result<bool, InternalError> {
        Ok(self.params_intersect(x.params(), y.params())?
            && self.intersects(x.result(), y.result())?)
    }

    /// We consider two type intersecting if either side knows if they intersect.
    /// This function checks the left side.
    fn intersects_one_side(&self, x: &TyBasic, y: &TyBasic) -> Result<bool, InternalError> {
        match (x, y) {
            (TyBasic::Any, _) => Ok(true),
            (TyBasic::List(x), TyBasic::List(y)) => self.intersects(x, y),
            (TyBasic::List(_), TyBasic::StarlarkValue(y)) => Ok(y.is_list()),
            (TyBasic::List(_), _) => Ok(false),
            (TyBasic::Set(x), TyBasic::Set(y)) => self.intersects(x, y),
            (TyBasic::Set(_), TyBasic::StarlarkValue(y)) => Ok(y.is_set()),
            (TyBasic::Set(_), _) => Ok(false),
            (TyBasic::Dict(x_k, x_v), TyBasic::Dict(y_k, y_v)) => {
                Ok(self.intersects(x_k, y_k)? && self.intersects(x_v, y_v)?)
            }
            (TyBasic::Dict(..), TyBasic::StarlarkValue(y)) => Ok(y.is_dict()),
            (TyBasic::Dict(..), _) => Ok(false),
            (TyBasic::Tuple(x), TyBasic::Tuple(y)) => TyTuple::intersects(x, y, self),
            (TyBasic::Tuple(_), TyBasic::StarlarkValue(y)) => Ok(y.is_tuple()),
            (TyBasic::Tuple(_), _) => Ok(false),
            (TyBasic::Iter(x), TyBasic::Iter(y)) => self.intersects(x, y),
            (TyBasic::Iter(x), y) | (y, TyBasic::Iter(x)) => match self.iter_item_basic(y) {
                Ok(yy) => self.intersects(x, &yy),
                Err(TypingNoContextError) => Ok(false),
            },
            (TyBasic::Callable(x), TyBasic::Callable(y)) => self.callables_intersect(x, y),
            (TyBasic::Callable(_), TyBasic::Custom(_)) => {
                // Handled when custom is lhs
                Ok(false)
            }
            (TyBasic::Callable(_), _) => Ok(false),
            (TyBasic::Custom(x), y) => x.intersects_with(y, *self),
            (TyBasic::StarlarkValue(x), TyBasic::Callable(_)) => Ok(x.is_callable()),
            (TyBasic::StarlarkValue(_), _) => Ok(false),
            (TyBasic::Type, TyBasic::StarlarkValue(y)) => Ok(y.is_type()),
            (TyBasic::Type, _) => {
                // TODO(nga): more precise.
                Ok(true)
            }
        }
    }
}
