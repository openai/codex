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

//! Inline functions.

pub(crate) mod local_as_value;

use crate::eval::compiler::args::ArgsCompiledValue;
use crate::eval::compiler::call::CallCompiled;
use crate::eval::compiler::def::ParametersCompiled;
use crate::eval::compiler::def_inline::local_as_value::LocalAsValue;
use crate::eval::compiler::expr::Builtin1;
use crate::eval::compiler::expr::Builtin2;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::expr::ExprLogicalBinOp;
use crate::eval::compiler::opt_ctx::OptCtx;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::stmt::StmtCompiled;
use crate::eval::compiler::stmt::StmtsCompiled;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::slots::LocalSlotId;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;

/// Function body suitable for inlining.
#[derive(Debug)]
pub(crate) enum InlineDefBody {
    /// Function body is `return type(x) == "y"`
    ReturnTypeIs(FrozenStringValue),
    /// Any expression which can be safely inlined.
    ///
    /// See the function where this enum variant is computed for the definition
    /// of safe to inline expression.
    ReturnSafeToInlineExpr(IrSpanned<ExprCompiled>),
}

/// If a statement is `return type(x) == "y"` where `x` is a first slot.
fn is_return_type_is(stmt: &StmtsCompiled) -> Option<FrozenStringValue> {
    let (x, t) = stmt.first()?.as_return()?.as_type_is()?;
    match &x.node {
        // Slot 0 is a slot for the first function parameter.
        ExprCompiled::Local(LocalSlotId(0)) => Some(t),
        _ => None,
    }
}

struct IsSafeToInlineExpr {
    /// Function parameter count.
    param_count: u32,
    /// How many expressions we visited already.
    counter: u32,
}

impl IsSafeToInlineExpr {
    fn new(param_count: u32) -> IsSafeToInlineExpr {
        Self {
            param_count,
            counter: 0,
        }
    }

    fn is_safe_to_inline_opt_expr(&mut self, expr: &Option<IrSpanned<ExprCompiled>>) -> bool {
        if let Some(expr) = expr {
            self.is_safe_to_inline_expr(expr)
        } else {
            true
        }
    }

    /// Expression which is has no access to locals or globals.
    fn is_safe_to_inline_expr(&mut self, expr: &ExprCompiled) -> bool {
        // Do not inline too large functions.
        if self.counter > 100 {
            return false;
        }
        self.counter += 1;
        match expr {
            ExprCompiled::Value(..) => true,
            ExprCompiled::LocalCaptured(..) | ExprCompiled::Module(..) | ExprCompiled::Def(..) => {
                false
            }
            ExprCompiled::Local(l) => {
                // `l >= param_count` should be unreachable, but it is safer this way.
                l.0 < self.param_count
            }
            ExprCompiled::Call(call) => {
                self.is_safe_to_inline_expr(&call.fun)
                    && call
                        .args
                        .arg_exprs()
                        .all(|e| self.is_safe_to_inline_expr(e))
            }
            ExprCompiled::Compr(..) => {
                // TODO: some comprehensions are safe to inline.
                false
            }
            ExprCompiled::Slice(a_b_c_d) => {
                let (a, b, c, d) = &**a_b_c_d;
                self.is_safe_to_inline_expr(a)
                    && self.is_safe_to_inline_opt_expr(b)
                    && self.is_safe_to_inline_opt_expr(c)
                    && self.is_safe_to_inline_opt_expr(d)
            }
            ExprCompiled::Builtin2(bin_op, a_b) => {
                let (a, b) = &**a_b;
                let _: &Builtin2 = bin_op;
                self.is_safe_to_inline_expr(a) && self.is_safe_to_inline_expr(b)
            }
            ExprCompiled::Index2(a_i0_i1) => {
                let (a, i0, i1) = &**a_i0_i1;
                self.is_safe_to_inline_expr(a)
                    && self.is_safe_to_inline_expr(i0)
                    && self.is_safe_to_inline_expr(i1)
            }
            ExprCompiled::Builtin1(un_op, arg) => {
                let _: &Builtin1 = un_op;
                self.is_safe_to_inline_expr(arg)
            }
            ExprCompiled::Tuple(xs) | ExprCompiled::List(xs) => {
                xs.iter().all(|x| self.is_safe_to_inline_expr(x))
            }
            ExprCompiled::Dict(xs) => xs
                .iter()
                .all(|(x, y)| self.is_safe_to_inline_expr(x) && self.is_safe_to_inline_expr(y)),
            ExprCompiled::If(c_t_f) => {
                let (c, t, f) = &**c_t_f;
                self.is_safe_to_inline_expr(c)
                    && self.is_safe_to_inline_expr(t)
                    && self.is_safe_to_inline_expr(f)
            }
            ExprCompiled::LogicalBinOp(op, x_y) => {
                let (x, y) = &**x_y;
                let _: &ExprLogicalBinOp = op;
                self.is_safe_to_inline_expr(x) && self.is_safe_to_inline_expr(y)
            }
            ExprCompiled::Seq(x_y) => {
                let (x, y) = &**x_y;
                self.is_safe_to_inline_expr(x) && self.is_safe_to_inline_expr(y)
            }
        }
    }
}

/// Function body is a `return` safe to inline expression (as defined above).
fn is_return_safe_to_inline_expr(
    stmts: &StmtsCompiled,
    param_count: u32,
) -> Option<IrSpanned<ExprCompiled>> {
    match stmts.first() {
        None => {
            // Empty function is equivalent to `return None`.
            Some(IrSpanned {
                span: FrameSpan::default(),
                node: ExprCompiled::Value(FrozenValue::new_none()),
            })
        }
        Some(stmt) => match &stmt.node {
            StmtCompiled::Return(expr)
                if IsSafeToInlineExpr::new(param_count).is_safe_to_inline_expr(expr) =>
            {
                Some(expr.clone())
            }
            _ => None,
        },
    }
}

pub(crate) fn inline_def_body(
    params: &ParametersCompiled<IrSpanned<ExprCompiled>>,
    body: &StmtsCompiled,
) -> Option<InlineDefBody> {
    if params.params.len() == 1 && params.params[0].accepts_positional() {
        if let Some(t) = is_return_type_is(body) {
            return Some(InlineDefBody::ReturnTypeIs(t));
        }
    }
    if !params.has_args_or_kwargs() {
        // It is possible to sometimes inline functions with `*args` or `**kwargs`,
        // but let's postpone that for now.
        let param_count = params.count_param_variables();
        if let Some(expr) = is_return_safe_to_inline_expr(body, param_count) {
            return Some(InlineDefBody::ReturnSafeToInlineExpr(expr));
        }
    }
    None
}

pub(crate) struct CannotInline;

/// Utility to inline function body at call site.
pub(crate) struct InlineDefCallSite<'s, 'v, 'a, 'e, 'x> {
    pub(crate) ctx: &'s mut OptCtx<'v, 'a, 'e, 'x>,
    // Values in the slots are either real frozen values
    // or `LocalAsValue` which are the parameters to be substituted with caller locals.
    pub(crate) slots: &'s [FrozenValue],
}

impl InlineDefCallSite<'_, '_, '_, '_, '_> {
    fn inline_opt(
        &mut self,
        expr: Option<&IrSpanned<ExprCompiled>>,
    ) -> Result<Option<IrSpanned<ExprCompiled>>, CannotInline> {
        match expr {
            None => Ok(None),
            Some(expr) => Ok(Some(self.inline(expr)?)),
        }
    }

    fn inline_args(&mut self, args: &ArgsCompiledValue) -> Result<ArgsCompiledValue, CannotInline> {
        args.map_exprs(|expr| self.inline(expr))
    }

    fn inline_call(
        &mut self,
        call: &IrSpanned<CallCompiled>,
    ) -> Result<IrSpanned<ExprCompiled>, CannotInline> {
        let span = call.span;
        let CallCompiled { fun, args } = &call.node;
        let fun = self.inline(fun)?;
        let args = self.inline_args(args)?;
        Ok(IrSpanned {
            span,
            node: CallCompiled::call(span, fun, args, self.ctx),
        })
    }

    pub(crate) fn inline(
        &mut self,
        expr: &IrSpanned<ExprCompiled>,
    ) -> Result<IrSpanned<ExprCompiled>, CannotInline> {
        let span = expr.span;
        Ok(match &expr.node {
            e @ ExprCompiled::Value(..) => IrSpanned {
                span,
                node: e.clone(),
            },
            ExprCompiled::Local(local) => {
                let value = self.slots[local.0 as usize];
                let expr = if let Some(local) = FrozenValueTyped::<LocalAsValue>::new(value) {
                    ExprCompiled::Local(local.local)
                } else {
                    ExprCompiled::Value(value)
                };
                IrSpanned { span, node: expr }
            }
            ExprCompiled::If(c_t_f) => {
                let (c, t, f) = &**c_t_f;
                let c = self.inline(c)?;
                let t = self.inline(t)?;
                let f = self.inline(f)?;
                ExprCompiled::if_expr(c, t, f)
            }
            ExprCompiled::LogicalBinOp(op, l_r) => {
                let (l, r) = &**l_r;
                let l = self.inline(l)?;
                let r = self.inline(r)?;
                ExprCompiled::logical_bin_op(*op, l, r)
            }
            ExprCompiled::List(xs) => {
                let xs = xs
                    .iter()
                    .map(|x| self.inline(x))
                    .collect::<Result<Vec<_>, CannotInline>>()?;
                IrSpanned {
                    span,
                    node: ExprCompiled::List(xs),
                }
            }
            ExprCompiled::Tuple(xs) => {
                let xs = xs
                    .iter()
                    .map(|x| self.inline(x))
                    .collect::<Result<Vec<_>, CannotInline>>()?;
                IrSpanned {
                    span,
                    node: ExprCompiled::tuple(xs, self.ctx.frozen_heap()),
                }
            }
            ExprCompiled::Dict(xs) => {
                let xs = xs
                    .iter()
                    .map(|(x, y)| Ok((self.inline(x)?, self.inline(y)?)))
                    .collect::<Result<Vec<_>, CannotInline>>()?;
                IrSpanned {
                    span,
                    node: ExprCompiled::Dict(xs),
                }
            }
            ExprCompiled::Builtin2(op, l_r) => {
                let (l, r) = &**l_r;
                let l = self.inline(l)?;
                let r = self.inline(r)?;
                IrSpanned {
                    span,
                    node: ExprCompiled::bin_op(*op, l, r, self.ctx),
                }
            }
            ExprCompiled::Index2(a_i0_i1) => {
                let (a, i0, i1) = &**a_i0_i1;
                let a = self.inline(a)?;
                let i0 = self.inline(i0)?;
                let i1 = self.inline(i1)?;
                IrSpanned {
                    span,
                    node: ExprCompiled::Index2(Box::new((a, i0, i1))),
                }
            }
            ExprCompiled::Builtin1(op, x) => {
                let x = self.inline(x)?;
                IrSpanned {
                    span,
                    node: ExprCompiled::un_op(span, op, x, self.ctx),
                }
            }
            ExprCompiled::Slice(l_a_b_c) => {
                let (l, a, b, c) = &**l_a_b_c;
                let l = self.inline(l)?;
                let a = self.inline_opt(a.as_ref())?;
                let b = self.inline_opt(b.as_ref())?;
                let c = self.inline_opt(c.as_ref())?;
                IrSpanned {
                    span,
                    node: ExprCompiled::Slice(Box::new((l, a, b, c))),
                }
            }
            ExprCompiled::Seq(a_b) => {
                let (a, b) = &**a_b;
                let a = self.inline(a)?;
                let b = self.inline(b)?;
                ExprCompiled::seq(a, b)
            }
            ExprCompiled::Call(call) => return self.inline_call(call),
            // These should be unreachable, but it is safer
            // to do unnecessary work in compiler than crash.
            ExprCompiled::LocalCaptured(..)
            | ExprCompiled::Module(..)
            | ExprCompiled::Compr(..)
            | ExprCompiled::Def(..) => return Err(CannotInline),
        })
    }
}
