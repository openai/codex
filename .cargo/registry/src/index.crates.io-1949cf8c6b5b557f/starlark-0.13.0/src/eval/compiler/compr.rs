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

//! List/dict/set comprenension evaluation.

use starlark_derive::VisitSpanMut;
use starlark_syntax::slice_vec_ext::SliceExt;
use starlark_syntax::syntax::ast::ClauseP;
use starlark_syntax::syntax::ast::ForClauseP;

use crate::eval::compiler::error::CompilerInternalError;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::expr_bool::ExprCompiledBool;
use crate::eval::compiler::known::list_to_tuple;
use crate::eval::compiler::opt_ctx::OptCtx;
use crate::eval::compiler::scope::payload::CstExpr;
use crate::eval::compiler::scope::payload::CstPayload;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::stmt::AssignCompiledValue;
use crate::eval::compiler::Compiler;

impl Compiler<'_, '_, '_, '_> {
    pub fn list_comprehension(
        &mut self,
        x: &CstExpr,
        for_: &ForClauseP<CstPayload>,
        clauses: &[ClauseP<CstPayload>],
    ) -> Result<ExprCompiled, CompilerInternalError> {
        let clauses = self.compile_clauses(for_, clauses)?;
        let x = self.expr(x)?;
        Ok(ExprCompiled::compr(ComprCompiled::List(
            Box::new(x),
            clauses,
        )))
    }

    pub fn dict_comprehension(
        &mut self,
        k: &CstExpr,
        v: &CstExpr,
        for_: &ForClauseP<CstPayload>,
        clauses: &[ClauseP<CstPayload>],
    ) -> Result<ExprCompiled, CompilerInternalError> {
        let clauses = self.compile_clauses(for_, clauses)?;
        let k = self.expr(k)?;
        let v = self.expr(v)?;
        Ok(ExprCompiled::compr(ComprCompiled::Dict(
            Box::new((k, v)),
            clauses,
        )))
    }

    /// Peel the final if's from clauses, and return them (in the order they started), plus the next for you get to
    fn compile_ifs(
        &mut self,
        clauses: &mut Vec<ClauseP<CstPayload>>,
    ) -> Result<(Option<ForClauseP<CstPayload>>, Vec<IrSpanned<ExprCompiled>>), CompilerInternalError>
    {
        let mut ifs = Vec::new();
        while let Some(x) = clauses.pop() {
            match x {
                ClauseP::For(f) => {
                    ifs.reverse();
                    return Ok((Some(f), ifs));
                }
                ClauseP::If(x) => {
                    let x = self.expr_truth(&x)?;
                    if let ExprCompiledBool::Const(true) = &x.node {
                        // If the condition is always true, skip the clause.
                        continue;
                    }
                    // TODO(nga): if condition is false, do something better.
                    ifs.push(x.into_expr());
                }
            }
        }
        ifs.reverse();
        Ok((None, ifs))
    }

    fn compile_clauses(
        &mut self,
        for_: &ForClauseP<CstPayload>,
        clauses: &[ClauseP<CstPayload>],
    ) -> Result<ClausesCompiled, CompilerInternalError> {
        // The first for.over is scoped before we enter the list comp
        let over = self.expr(&list_to_tuple(&for_.over))?;

        // TODO(nga): unnecessary clone.
        let mut clauses = clauses.to_vec();

        // Now we want to group them into a `for`, followed by any number of `if`.
        // The evaluator wants to use pop to consume them, so reverse the order.
        let mut res = Vec::new();
        loop {
            let (next_for, ifs) = self.compile_ifs(&mut clauses)?;
            match next_for {
                None => {
                    let last = ClauseCompiled {
                        var: self.assign_target(&for_.var)?,
                        over,
                        ifs,
                    };
                    return Ok(ClausesCompiled::new(res, last));
                }
                Some(f) => {
                    res.push(ClauseCompiled {
                        over: self.expr(&f.over)?,
                        var: self.assign_target(&f.var)?,
                        ifs,
                    });
                }
            }
        }
    }
}

#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) enum ComprCompiled {
    List(Box<IrSpanned<ExprCompiled>>, ClausesCompiled),
    Dict(
        Box<(IrSpanned<ExprCompiled>, IrSpanned<ExprCompiled>)>,
        ClausesCompiled,
    ),
}

impl ComprCompiled {
    pub(crate) fn clauses(&self) -> &ClausesCompiled {
        match self {
            ComprCompiled::List(_, clauses) => clauses,
            ComprCompiled::Dict(_, clauses) => clauses,
        }
    }

    pub(crate) fn optimize(&self, ctx: &mut OptCtx) -> ExprCompiled {
        match self {
            ComprCompiled::List(ref x, ref clauses) => {
                let clauses = clauses.optimize(ctx);
                ExprCompiled::compr(ComprCompiled::List(Box::new(x.optimize(ctx)), clauses))
            }
            ComprCompiled::Dict(k_v, ref clauses) => {
                let (k, v) = &**k_v;
                let clauses = clauses.optimize(ctx);
                ExprCompiled::compr(ComprCompiled::Dict(
                    Box::new((k.optimize(ctx), v.optimize(ctx))),
                    clauses.optimize(ctx),
                ))
            }
        }
    }
}

#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) struct ClauseCompiled {
    pub(crate) var: IrSpanned<AssignCompiledValue>,
    pub(crate) over: IrSpanned<ExprCompiled>,
    pub(crate) ifs: Vec<IrSpanned<ExprCompiled>>,
}

impl ClauseCompiled {
    fn optimize(&self, ctx: &mut OptCtx) -> ClauseCompiled {
        let ClauseCompiled {
            ref var,
            ref over,
            ref ifs,
        } = *self;
        ClauseCompiled {
            var: var.optimize(ctx),
            over: over.optimize(ctx),
            ifs: ifs
                .iter()
                .filter_map(|e| {
                    let e = e.optimize(ctx);
                    let e = ExprCompiledBool::new(e);
                    match &e.node {
                        ExprCompiledBool::Const(true) => None,
                        _ => Some(e.into_expr()),
                    }
                })
                .collect(),
        }
    }
}

/// All clauses in a comprehension. Never empty.
#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) struct ClausesCompiled {
    /// Not empty.
    ///
    /// Clauses are in reverse order, i. e. the first executed clause is the last in the list.
    clauses: Vec<ClauseCompiled>,
}

impl ClausesCompiled {
    fn new(clauses: Vec<ClauseCompiled>, last: ClauseCompiled) -> ClausesCompiled {
        let mut clauses = clauses;
        clauses.push(last);
        ClausesCompiled { clauses }
    }

    /// Clauses are definitely no-op, i. e. zero iterations, and no side effects of iteration.
    pub(crate) fn is_nop(&self) -> bool {
        // NOTE(nga): if the first loop argument is empty collection, clauses are definitely no-op.
        //   But this is not true for the rest of loops: if inner loop collection is empty,
        //   clauses produce no iterations, but the outer loop may still has side effects.
        //   There are missing optimizations here:
        //   * we could separate effects and emit empty list/dict.
        //   * or at least do not generate comprehension terminator.
        self.split_last().0.over.is_iterable_empty()
    }

    /// Last clause is the one which is executed first.
    pub(crate) fn split_last(&self) -> (&ClauseCompiled, &[ClauseCompiled]) {
        self.clauses.split_last().unwrap()
    }

    fn optimize(&self, ctx: &mut OptCtx) -> ClausesCompiled {
        ClausesCompiled {
            clauses: self.clauses.map(|c| c.optimize(ctx)),
        }
    }
}
