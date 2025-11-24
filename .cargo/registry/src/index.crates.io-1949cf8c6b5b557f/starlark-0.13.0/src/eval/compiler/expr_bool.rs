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

//! Boolean expression.

use crate::eval::compiler::expr::Builtin1;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::expr::ExprLogicalBinOp;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::values::FrozenValue;

/// Boolean expression.
pub(crate) enum ExprCompiledBool {
    Const(bool),
    /// Non-const expression.
    Expr(ExprCompiled),
}

impl IrSpanned<ExprCompiledBool> {
    pub(crate) fn into_expr(self) -> IrSpanned<ExprCompiled> {
        IrSpanned {
            span: self.span,
            node: self.node.into_expr(),
        }
    }
}

impl ExprCompiledBool {
    fn into_expr(self) -> ExprCompiled {
        match self {
            ExprCompiledBool::Const(b) => ExprCompiled::Value(FrozenValue::new_bool(b)),
            ExprCompiledBool::Expr(e) => e,
        }
    }

    fn const_value(&self) -> Option<bool> {
        match self {
            ExprCompiledBool::Const(b) => Some(*b),
            ExprCompiledBool::Expr(..) => None,
        }
    }

    /// `bool(x)` and do trivial optimizations.
    pub(crate) fn new(expr: IrSpanned<ExprCompiled>) -> IrSpanned<ExprCompiledBool> {
        fn new_bool(span: FrameSpan, b: bool) -> IrSpanned<ExprCompiledBool> {
            IrSpanned {
                node: ExprCompiledBool::Const(b),
                span,
            }
        }

        let span = expr.span;

        if let Some(b) = expr.is_pure_infallible_to_bool() {
            return new_bool(span, b);
        }

        match expr.node {
            ExprCompiled::Builtin1(Builtin1::Not, x) => {
                let x = Self::new(*x);
                match x.const_value() {
                    Some(b) => new_bool(span, !b),
                    None => IrSpanned {
                        node: ExprCompiledBool::Expr(ExprCompiled::Builtin1(
                            Builtin1::Not,
                            Box::new(x.into_expr()),
                        )),
                        span,
                    },
                }
            }
            ExprCompiled::LogicalBinOp(op, x_y) => {
                let (x, y) = *x_y;
                let x = Self::new(x);
                let y = Self::new(y);
                match (op, x.const_value(), y.const_value()) {
                    (ExprLogicalBinOp::And, Some(false), _) => new_bool(span, false),
                    (ExprLogicalBinOp::Or, Some(true), _) => new_bool(span, true),
                    (ExprLogicalBinOp::And, Some(true), _) => y,
                    (ExprLogicalBinOp::Or, Some(false), _) => y,
                    (ExprLogicalBinOp::And, None, Some(true)) => x,
                    (ExprLogicalBinOp::Or, None, Some(false)) => x,
                    (ExprLogicalBinOp::And, None, Some(false)) => {
                        // The expression evaluates to false,
                        // but we need to preserve LHS for the effect.
                        IrSpanned {
                            span,
                            node: ExprCompiledBool::Expr(
                                ExprCompiled::seq(
                                    x.into_expr(),
                                    new_bool(y.span, false).into_expr(),
                                )
                                .node,
                            ),
                        }
                    }
                    (ExprLogicalBinOp::Or, None, Some(true)) => {
                        // The expression evaluates to true,
                        // but we need to preserve LHS for the effect.
                        IrSpanned {
                            span,
                            node: ExprCompiledBool::Expr(
                                ExprCompiled::seq(
                                    x.into_expr(),
                                    new_bool(y.span, true).into_expr(),
                                )
                                .node,
                            ),
                        }
                    }
                    (op, None, None) => IrSpanned {
                        node: ExprCompiledBool::Expr(ExprCompiled::LogicalBinOp(
                            op,
                            Box::new((x.into_expr(), y.into_expr())),
                        )),
                        span,
                    },
                }
            }
            expr => IrSpanned {
                node: ExprCompiledBool::Expr(expr),
                span,
            },
        }
    }
}
