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

use crate::eval::bc::bytecode::Bc;
use crate::eval::bc::compiler::if_compiler::write_if_else;
use crate::eval::bc::compiler::if_compiler::write_if_then;
use crate::eval::bc::instr_impl::InstrCheckType;
use crate::eval::bc::instr_impl::InstrPossibleGc;
use crate::eval::bc::instr_impl::InstrReturn;
use crate::eval::bc::instr_impl::InstrReturnCheckType;
use crate::eval::bc::instr_impl::InstrReturnConst;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::writer::BcWriter;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::expr::MaybeNot;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::stmt::AssignCompiledValue;
use crate::eval::compiler::stmt::StmtCompileContext;
use crate::eval::compiler::stmt::StmtCompiled;
use crate::eval::compiler::stmt::StmtsCompiled;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;

pub(crate) fn write_for(
    over: &IrSpanned<ExprCompiled>,
    var: &IrSpanned<AssignCompiledValue>,
    span: FrameSpan,
    bc: &mut BcWriter,
    body: impl FnOnce(&mut BcWriter),
) {
    let definitely_assigned = bc.save_definitely_assigned();

    over.write_bc_cb(bc, |over, bc| {
        if let Some(var) = var.as_local_non_captured() {
            // Typical case: `for x in ...: ...`,
            // compile loop assignment directly to a local variable.
            bc.write_for(over, var.to_bc_slot().to_out(), span, |bc| {
                bc.mark_definitely_assigned(var);
                body(bc);
            })
        } else {
            // General case, e. g. `for (x, y[0]) in ...: ...`,
            // compile loop assignment to a temporary variable,
            // and reassign it in the loop body.
            bc.alloc_slot(|var_slot, bc| {
                bc.write_for(over, var_slot.to_out(), span, |bc| {
                    var.write_bc(var_slot.to_in(), bc);
                    var.mark_definitely_assigned_after(bc);
                    body(bc);
                })
            })
        }
    });

    bc.restore_definitely_assigned(definitely_assigned);
}

impl StmtsCompiled {
    pub(crate) fn write_bc(&self, compiler: &StmtCompileContext, bc: &mut BcWriter) {
        for stmt in self.stmts() {
            stmt.write_bc(compiler, bc);
        }
    }
}

impl StmtCompiled {
    /// Mark local variables are definitely assigned after this statement executed.
    pub(crate) fn mark_definitely_assigned_after(&self, bc: &mut BcWriter) {
        match self {
            StmtCompiled::PossibleGc => {}
            StmtCompiled::Return(e) => {
                // `e` is definitely assigned after `return` statement,
                // but no code is executed after `return`, so marking would be useless.
                let _ = e;
            }
            StmtCompiled::Expr(e) => e.mark_definitely_assigned_after(bc),
            StmtCompiled::Assign(lhs, ty, rhs) => {
                lhs.mark_definitely_assigned_after(bc);
                rhs.mark_definitely_assigned_after(bc);
                // We might have evaluate types turned off
                let _ = ty;
            }
            StmtCompiled::AssignModify(lhs, _op, rhs) => {
                rhs.mark_definitely_assigned_after(bc);
                lhs.mark_definitely_assigned_after(bc);
            }
            StmtCompiled::If(cond_t_f) => {
                let (cond, t, f) = &**cond_t_f;
                cond.mark_definitely_assigned_after(bc);
                // We could merge `t` and `f` definitely assigned, e. g.
                // ```
                // if cond:
                //   x = 1
                // else:
                //   x = 2
                // ```
                // we could mark `x` as definitely assigned.
                let _ = (t, f);
            }
            StmtCompiled::For(var_over_body) => {
                let (_var, over, _body) = &**var_over_body;
                over.mark_definitely_assigned_after(bc);
            }
            StmtCompiled::Break => {}
            StmtCompiled::Continue => {}
        }
    }

    /// If statement is `return x`, return `x`.
    pub(crate) fn as_return(&self) -> Option<&IrSpanned<ExprCompiled>> {
        match self {
            StmtCompiled::Return(e) => Some(e),
            _ => None,
        }
    }
}

impl IrSpanned<StmtCompiled> {
    fn write_bc(&self, compiler: &StmtCompileContext, bc: &mut BcWriter) {
        bc.mark_before_stmt(self.span);
        self.write_bc_inner(compiler, bc);
        self.mark_definitely_assigned_after(bc);
    }

    fn write_if_then(
        compiler: &StmtCompileContext,
        bc: &mut BcWriter,
        c: &IrSpanned<ExprCompiled>,
        maybe_not: MaybeNot,
        t: &dyn Fn(&StmtCompileContext, &mut BcWriter),
    ) {
        write_if_then(
            c,
            maybe_not,
            |bc| {
                t(compiler, bc);
            },
            bc,
        );
    }

    fn write_if_else(
        c: &IrSpanned<ExprCompiled>,
        t: &StmtsCompiled,
        f: &StmtsCompiled,
        compiler: &StmtCompileContext,
        bc: &mut BcWriter,
    ) {
        assert!(!t.is_empty() || !f.is_empty());
        if f.is_empty() {
            Self::write_if_then(compiler, bc, c, MaybeNot::Id, &|compiler, bc| {
                t.write_bc(compiler, bc)
            });
        } else if t.is_empty() {
            Self::write_if_then(compiler, bc, c, MaybeNot::Not, &|compiler, bc| {
                f.write_bc(compiler, bc)
            });
        } else {
            write_if_else(
                c,
                |bc| t.write_bc(compiler, bc),
                |bc| f.write_bc(compiler, bc),
                bc,
            );
        }
    }

    fn write_return(
        span: FrameSpan,
        expr: &IrSpanned<ExprCompiled>,
        compiler: &StmtCompileContext,
        bc: &mut BcWriter,
    ) {
        bc.write_iter_stop(span);
        if compiler.has_return_type {
            expr.write_bc_cb(bc, |slot, bc| {
                bc.write_instr::<InstrReturnCheckType>(span, slot);
            });
        } else if let Some(value) = expr.as_value() {
            bc.write_instr::<InstrReturnConst>(span, value);
        } else {
            expr.write_bc_cb(bc, |slot, bc| {
                bc.write_instr::<InstrReturn>(span, slot);
            });
        }
    }

    fn write_bc_inner(&self, compiler: &StmtCompileContext, bc: &mut BcWriter) {
        let span = self.span;
        match &self.node {
            StmtCompiled::PossibleGc => bc.write_instr::<InstrPossibleGc>(span, ()),
            StmtCompiled::Return(expr) => Self::write_return(span, expr, compiler, bc),
            StmtCompiled::Expr(expr) => {
                expr.write_bc_for_effect(bc);
            }
            StmtCompiled::Assign(lhs, ty, rhs) => {
                fn check_type(
                    ty: &Option<IrSpanned<TypeCompiled<FrozenValue>>>,
                    slot_expr: BcSlotIn,
                    bc: &mut BcWriter,
                ) {
                    if let Some(ty) = ty {
                        bc.write_instr::<InstrCheckType>(ty.span, (slot_expr, ty.node))
                    }
                }

                if let Some(local) = lhs.as_local_non_captured() {
                    // Write expression directly to local slot.
                    rhs.write_bc(local.to_bc_slot().to_out(), bc);
                    check_type(ty, local.to_bc_slot().to_in(), bc);
                } else {
                    rhs.write_bc_cb(bc, |slot, bc| {
                        check_type(ty, slot, bc);
                        lhs.write_bc(slot, bc);
                    });
                }
            }
            StmtCompiled::AssignModify(lhs, op, rhs) => {
                lhs.write_bc(span, *op, rhs, bc);
            }
            StmtCompiled::If(c_t_f) => {
                let (c, t, f) = &**c_t_f;
                Self::write_if_else(c, t, f, compiler, bc);
            }
            StmtCompiled::For(assign_over_body) => {
                let (assign, over, body) = &**assign_over_body;
                write_for(over, assign, span, bc, |bc| body.write_bc(compiler, bc));
            }
            StmtCompiled::Break => {
                bc.write_break(span);
            }
            StmtCompiled::Continue => {
                bc.write_continue(span);
            }
        }
    }
}

impl StmtsCompiled {
    pub(crate) fn as_bc(
        &self,
        compiler: &StmtCompileContext,
        local_names: FrozenRef<'static, [FrozenStringValue]>,
        param_count: u32,
        heap: &FrozenHeap,
    ) -> Bc {
        let mut bc = BcWriter::new(local_names, param_count, heap);
        self.write_bc(compiler, &mut bc);

        // Small optimization: if the last statement is return,
        // we do not need to write another return.
        if !matches!(self.last().map(|s| &s.node), Some(StmtCompiled::Return(..))) {
            let span = self.last().map(|s| s.span.end_span()).unwrap_or_default();
            if compiler.has_return_type {
                bc.alloc_slot(|slot, bc| {
                    bc.write_const(span, FrozenValue::new_none(), slot.to_out());
                    bc.write_instr::<InstrReturnCheckType>(span, slot.to_in());
                });
            } else {
                bc.write_instr::<InstrReturnConst>(span, FrozenValue::new_none());
            }
        }

        bc.finish()
    }
}
