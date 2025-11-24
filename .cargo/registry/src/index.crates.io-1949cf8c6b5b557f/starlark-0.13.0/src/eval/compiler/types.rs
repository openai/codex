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

use starlark_syntax::eval_exception::EvalException;
use starlark_syntax::internal_error;
use starlark_syntax::slice_vec_ext::VecExt;
use starlark_syntax::syntax::type_expr::TypeExprUnpackP;
use starlark_syntax::syntax::type_expr::TypePathP;

use crate::codemap::Span;
use crate::codemap::Spanned;
use crate::eval::compiler::constants::Constants;
use crate::eval::compiler::scope::payload::CstIdent;
use crate::eval::compiler::scope::payload::CstPayload;
use crate::eval::compiler::scope::payload::CstStmt;
use crate::eval::compiler::scope::payload::CstTypeExpr;
use crate::eval::compiler::scope::ResolvedIdent;
use crate::eval::compiler::scope::Slot;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::Compiler;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
use crate::typing::Ty;
use crate::values::types::ellipsis::Ellipsis;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::FrozenValue;
use crate::values::Value;

#[derive(Debug, thiserror::Error)]
enum TypesError {
    #[error("Identifier is not resolved (internal error)")]
    UnresolvedIdentifier,
    #[error("Identifier is resolve as local variable (internal error)")]
    LocalIdentifier,
    #[error("Module variable is not set: `{0}`")]
    ModuleVariableNotSet(String),
    #[error("Type payload not set (internal error)")]
    TypePayloadNotSet,
    #[error("[] can only be applied to list function in type expression")]
    TypeIndexOnNonList,
    #[error("[,] can only be applied to dict or tuple functions in type expression")]
    TypeIndexOnNonDictOrTuple,
}

impl<'v> Compiler<'v, '_, '_, '_> {
    /// Compile expression when it is expected to be interpreted as type.
    pub(crate) fn expr_for_type(
        &mut self,
        expr: Option<&CstTypeExpr>,
    ) -> Option<IrSpanned<TypeCompiled<FrozenValue>>> {
        if !self.check_types {
            return None;
        }
        let expr = expr?;
        let span = FrameSpan::new(FrozenFileSpan::new(self.codemap, expr.span));
        let Some(ty) = &expr.payload.compiler_ty else {
            // This is unreachable. But unfortunately we do not return error here.
            // Still make an error in panic to produce nice panic message.
            panic!(
                "{:?}",
                EvalException::new_anyhow(
                    TypesError::TypePayloadNotSet.into(),
                    expr.span,
                    &self.codemap
                )
            );
        };
        let type_value = TypeCompiled::from_ty(ty, self.eval.heap());
        if type_value.is_runtime_wildcard() {
            return None;
        }
        let type_value = type_value.to_frozen(self.eval.frozen_heap());
        Some(IrSpanned {
            span,
            node: type_value,
        })
    }

    /// We evaluated type expression to `Value`, now convert it to `FrozenValue`.
    fn alloc_value_for_type(
        &mut self,
        value: Value<'v>,
        span: Span,
    ) -> Result<TypeCompiled<Value<'v>>, EvalException> {
        let ty = TypeCompiled::new(value, self.eval.heap());
        ty.map_err(|e| EvalException::new_anyhow(e, span, &self.codemap))
    }

    fn eval_ident_in_type_expr(&mut self, ident: &CstIdent) -> Result<Value<'v>, EvalException> {
        let Some(ident_payload) = &ident.node.payload else {
            return Err(EvalException::new_anyhow(
                TypesError::UnresolvedIdentifier.into(),
                ident.span,
                &self.codemap,
            ));
        };
        match ident_payload {
            ResolvedIdent::Slot(Slot::Local(..), _) => Err(EvalException::new_anyhow(
                TypesError::LocalIdentifier.into(),
                ident.span,
                &self.codemap,
            )),
            ResolvedIdent::Slot(Slot::Module(module_slot_id), _) => {
                match self.eval.module_env.slots().get_slot(*module_slot_id) {
                    Some(v) => Ok(v),
                    None => Err(EvalException::new_anyhow(
                        TypesError::ModuleVariableNotSet(ident.node.ident.clone()).into(),
                        ident.span,
                        &self.codemap,
                    )),
                }
            }
            ResolvedIdent::Global(v) => Ok(v.to_value()),
        }
    }

    /// We may use non-frozen values as types, so we don't reuse `expr_ident` function
    /// which is used in normal compilation.
    fn eval_path(&mut self, path: TypePathP<CstPayload>) -> Result<Value<'v>, EvalException> {
        let TypePathP { first, rem } = path;
        let mut value = self.eval_ident_in_type_expr(first)?;
        for step in rem {
            value = value
                .get_attr_error(step.node, self.eval.heap())
                .map_err(|e| EvalException::new(e, step.span, &self.codemap))?;
        }
        Ok(value)
    }

    fn eval_expr_as_type(
        &mut self,
        expr: Spanned<TypeExprUnpackP<CstPayload>>,
    ) -> Result<TypeCompiled<Value<'v>>, EvalException> {
        let span = expr.span;
        let value = self.eval_expr(expr)?;
        self.alloc_value_for_type(value, span)
    }

    /// Evaluate expression in context of typechecker.
    /// It is very restricted in what it can do.
    fn eval_expr(
        &mut self,
        expr: Spanned<TypeExprUnpackP<CstPayload>>,
    ) -> Result<Value<'v>, EvalException> {
        match expr.node {
            TypeExprUnpackP::Ellipsis => Ok(Ellipsis::new_value().to_value()),
            TypeExprUnpackP::List(items) => {
                let values: Vec<_> = items
                    .into_iter()
                    .map(|item| self.eval_expr(item))
                    .collect::<Result<_, _>>()?;
                Ok(self.eval.heap().alloc_list(&values))
            }
            TypeExprUnpackP::Path(path) => self.eval_path(path),
            TypeExprUnpackP::Index(a, i) => {
                let a = self.eval_ident_in_type_expr(a)?;
                if !a.ptr_eq(Constants::get().fn_list.0.to_value())
                    && !a.ptr_eq(Constants::get().fn_set.0.to_value())
                {
                    return Err(EvalException::new_anyhow(
                        TypesError::TypeIndexOnNonList.into(),
                        expr.span,
                        &self.codemap,
                    ));
                }
                let i = self.eval_expr_as_type(*i)?;
                a.get_ref()
                    .at(i.to_inner(), self.eval.heap())
                    .map_err(|e| EvalException::new(e, expr.span, &self.codemap))
            }
            TypeExprUnpackP::Index2(a, i0, i1) => {
                let a = self.eval_path(a.node)?;
                if a.ptr_eq(Constants::get().fn_dict.0.to_value())
                    || a.ptr_eq(Constants::get().fn_tuple.0.to_value())
                    || a.ptr_eq(Constants::get().typing_callable.0.to_value())
                {
                    let i0 = self.eval_expr(*i0)?;
                    let i1 = self.eval_expr(*i1)?;
                    a.get_ref()
                        .at2(i0, i1, self.eval.heap())
                        .map_err(|e| EvalException::new(e, expr.span, &self.codemap))
                } else {
                    return Err(EvalException::new_anyhow(
                        TypesError::TypeIndexOnNonDictOrTuple.into(),
                        expr.span,
                        &self.codemap,
                    ));
                }
            }
            TypeExprUnpackP::Union(xs) => {
                let xs = xs.into_try_map(|x| self.eval_expr_as_type(x))?;
                Ok(TypeCompiled::type_any_of(xs, self.eval.heap()).to_inner())
            }
            // TODO(nga): tuple type should be `tuple[str, int, bool]`, not `(str, int, bool)`.
            TypeExprUnpackP::Tuple(xs) => {
                let xs = xs.into_try_map(|x| {
                    Ok::<_, EvalException>(self.eval_expr_as_type(x)?.as_ty().clone())
                })?;
                Ok(TypeCompiled::from_ty(&Ty::tuple(xs), self.eval.heap()).to_inner())
            }
        }
    }

    fn populate_types_in_type_expr(
        &mut self,
        type_expr: &mut CstTypeExpr,
    ) -> Result<(), EvalException> {
        if type_expr.payload.compiler_ty.is_some() {
            return Err(EvalException::new(
                internal_error!("Type already initialized"),
                type_expr.span,
                &self.codemap,
            ));
        }
        // This should not fail because we validated it at parse time.
        let unpack = TypeExprUnpackP::unpack(&type_expr.expr, &self.codemap)?;
        let type_value = self.eval_expr_as_type(unpack)?;
        type_expr.payload.compiler_ty = Some(type_value.as_ty().clone());
        Ok(())
    }

    pub(crate) fn populate_types_in_stmt(
        &mut self,
        stmt: &mut CstStmt,
    ) -> Result<(), EvalException> {
        stmt.visit_type_expr_err_mut(&mut |type_expr| self.populate_types_in_type_expr(type_expr))
    }
}
