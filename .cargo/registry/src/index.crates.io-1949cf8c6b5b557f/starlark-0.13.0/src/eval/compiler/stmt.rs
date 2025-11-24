/*
 * Copyright 2018 The Starlark in Rust Authors.
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

//! Evaluation environment, provide converters from Ast* element to value.
//!
//! # <a name="build_file"></a>Starlark and BUILD dialect
//!
//! All evaluation function can evaluate the full Starlark language (i.e.
//! Bazel's .bzl files) or the BUILD file dialect (i.e. used to interpret
//! Bazel's BUILD file). The BUILD dialect does not allow `def` statements.

use std::cmp;

use starlark_derive::VisitSpanMut;
use starlark_syntax::slice_vec_ext::SliceExt;
use starlark_syntax::syntax::ast::AssignOp;
use starlark_syntax::syntax::ast::AssignP;
use starlark_syntax::syntax::ast::AssignTargetP;
use starlark_syntax::syntax::ast::DefP;
use starlark_syntax::syntax::ast::ForP;
use starlark_syntax::syntax::ast::StmtP;
use thiserror::Error;

use crate::codemap::Span;
use crate::codemap::Spanned;
use crate::environment::slots::ModuleSlotId;
use crate::environment::FrozenModuleData;
use crate::eval::compiler::error::CompilerInternalError;
use crate::eval::compiler::expr::Builtin1;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::expr::ExprLogicalBinOp;
use crate::eval::compiler::expr_bool::ExprCompiledBool;
use crate::eval::compiler::known::list_to_tuple;
use crate::eval::compiler::opt_ctx::OptCtx;
use crate::eval::compiler::scope::payload::CstAssignTarget;
use crate::eval::compiler::scope::payload::CstExpr;
use crate::eval::compiler::scope::payload::CstStmt;
use crate::eval::compiler::scope::Captured;
use crate::eval::compiler::scope::Slot;
use crate::eval::compiler::small_vec_1::SmallVec1;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::Compiler;
use crate::eval::runtime::evaluator::Evaluator;
use crate::eval::runtime::evaluator::GC_THRESHOLD;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
use crate::eval::runtime::slots::LocalCapturedSlotId;
use crate::eval::runtime::slots::LocalSlotId;
use crate::values::dict::Dict;
use crate::values::dict::DictMut;
use crate::values::dict::DictRef;
use crate::values::types::list::value::ListData;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;
use crate::values::ValueError;

#[derive(Clone, Debug)]
pub(crate) enum AssignModifyLhs {
    Dot(IrSpanned<ExprCompiled>, String),
    Array(IrSpanned<ExprCompiled>, IrSpanned<ExprCompiled>),
    Local(IrSpanned<LocalSlotId>),
    LocalCaptured(IrSpanned<LocalCapturedSlotId>),
    Module(IrSpanned<ModuleSlotId>),
}

#[derive(Clone, Debug)]
pub(crate) enum StmtCompiled {
    PossibleGc,
    Return(IrSpanned<ExprCompiled>),
    Expr(IrSpanned<ExprCompiled>),
    Assign(
        IrSpanned<AssignCompiledValue>,
        Option<IrSpanned<TypeCompiled<FrozenValue>>>,
        IrSpanned<ExprCompiled>,
    ),
    AssignModify(AssignModifyLhs, AssignOp, IrSpanned<ExprCompiled>),
    If(Box<(IrSpanned<ExprCompiled>, StmtsCompiled, StmtsCompiled)>),
    For(
        Box<(
            IrSpanned<AssignCompiledValue>,
            IrSpanned<ExprCompiled>,
            StmtsCompiled,
        )>,
    ),
    Break,
    Continue,
}

#[derive(Debug, Default)]
pub(crate) struct StmtCompileContext {
    /// Current function has return type.
    pub(crate) has_return_type: bool,
}

pub(crate) struct OptimizeOnFreezeContext<'v, 'a> {
    pub(crate) module: &'a FrozenModuleData,
    /// Nothing useful should be left in the heap after the freeze,
    /// but having a heap is useful to allocate objects temporarily
    /// (when invoking operations which require heap).
    pub(crate) heap: &'v Heap,
    pub(crate) frozen_heap: &'a FrozenHeap,
}

impl AssignModifyLhs {
    fn optimize(&self, ctx: &mut OptCtx) -> AssignModifyLhs {
        match self {
            AssignModifyLhs::Dot(expr, name) => {
                AssignModifyLhs::Dot(expr.optimize(ctx), name.clone())
            }
            AssignModifyLhs::Array(expr, index) => {
                AssignModifyLhs::Array(expr.optimize(ctx), index.optimize(ctx))
            }
            l @ (AssignModifyLhs::Local(..)
            | AssignModifyLhs::LocalCaptured(..)
            | AssignModifyLhs::Module(..)) => l.clone(),
        }
    }
}

impl IrSpanned<StmtCompiled> {
    fn optimize(&self, ctx: &mut OptCtx) -> StmtsCompiled {
        let span = self.span;
        match &self.node {
            StmtCompiled::Return(e) => StmtsCompiled::one(IrSpanned {
                span,
                node: StmtCompiled::Return(e.optimize(ctx)),
            }),
            StmtCompiled::Expr(expr) => {
                let expr = expr.optimize(ctx);
                StmtsCompiled::expr(expr)
            }
            StmtCompiled::Assign(lhs, ty, rhs) => {
                let lhs = lhs.optimize(ctx);
                let rhs = rhs.optimize(ctx);
                StmtsCompiled::one(IrSpanned {
                    span,
                    node: StmtCompiled::Assign(lhs, *ty, rhs),
                })
            }
            StmtCompiled::If(cond_t_f) => {
                let (cond, t, f) = &**cond_t_f;
                let cond = cond.optimize(ctx);
                let t = t.optimize(ctx);
                let f = f.optimize(ctx);
                StmtsCompiled::if_stmt(span, cond, t, f)
            }
            StmtCompiled::For(var_over_body) => {
                let (var, over, body) = &**var_over_body;
                let var = var.optimize(ctx);
                let over = over.optimize(ctx);
                let body = body.optimize(ctx);
                StmtsCompiled::for_stmt(span, var, over, body)
            }
            s @ (StmtCompiled::PossibleGc | StmtCompiled::Break | StmtCompiled::Continue) => {
                StmtsCompiled::one(IrSpanned {
                    span,
                    node: s.clone(),
                })
            }
            StmtCompiled::AssignModify(lhs, op, rhs) => StmtsCompiled::one(IrSpanned {
                span,
                node: StmtCompiled::AssignModify(lhs.optimize(ctx), *op, rhs.optimize(ctx)),
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StmtsCompiled(SmallVec1<IrSpanned<StmtCompiled>>);

impl StmtsCompiled {
    pub(crate) fn empty() -> StmtsCompiled {
        StmtsCompiled(SmallVec1::new())
    }

    pub(crate) fn one(stmt: IrSpanned<StmtCompiled>) -> StmtsCompiled {
        StmtsCompiled(SmallVec1::One(stmt))
    }

    pub(crate) fn is_empty(&self) -> bool {
        match &self.0 {
            SmallVec1::One(_) => false,
            SmallVec1::Vec(stmts) => stmts.is_empty(),
        }
    }

    pub(crate) fn stmts(&self) -> &[IrSpanned<StmtCompiled>] {
        self.0.as_slice()
    }

    /// Last statement in this block is `break`, `continue` or `return`.
    fn is_terminal(&self) -> bool {
        if let Some(stmt) = self.last() {
            match &stmt.node {
                StmtCompiled::Break | StmtCompiled::Continue | StmtCompiled::Return(..) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    pub(crate) fn extend(&mut self, right: StmtsCompiled) {
        // Do not add any code after `break`, `continue` or `return`.
        if self.is_terminal() {
            return;
        }
        self.0.extend(right.0);
    }

    pub(crate) fn optimize(&self, ctx: &mut OptCtx) -> StmtsCompiled {
        let mut stmts = StmtsCompiled::empty();
        match &self.0 {
            SmallVec1::One(s) => stmts.extend(s.optimize(ctx)),
            SmallVec1::Vec(ss) => {
                for s in ss {
                    if stmts.is_terminal() {
                        break;
                    }
                    stmts.extend(s.optimize(ctx));
                }
            }
        }
        stmts
    }

    pub(crate) fn first(&self) -> Option<&IrSpanned<StmtCompiled>> {
        match &self.0 {
            SmallVec1::One(s) => Some(s),
            SmallVec1::Vec(ss) => ss.first(),
        }
    }

    pub(crate) fn last(&self) -> Option<&IrSpanned<StmtCompiled>> {
        match &self.0 {
            SmallVec1::One(s) => Some(s),
            SmallVec1::Vec(ss) => ss.last(),
        }
    }

    fn expr(expr: IrSpanned<ExprCompiled>) -> StmtsCompiled {
        let span = expr.span;
        match expr.node {
            expr if expr.is_pure_infallible() => StmtsCompiled::empty(),
            ExprCompiled::List(xs) | ExprCompiled::Tuple(xs) => {
                let mut stmts = StmtsCompiled::empty();
                for x in xs {
                    stmts.extend(Self::expr(x));
                }
                stmts
            }
            // Unwrap infallible expressions.
            ExprCompiled::Builtin1(Builtin1::Not | Builtin1::TypeIs(_), x) => Self::expr(*x),
            // "And" and "or" for effect are equivalent to `if`.
            ExprCompiled::LogicalBinOp(ExprLogicalBinOp::And, x_y) => {
                let (x, y) = *x_y;
                Self::if_stmt(expr.span, x, Self::expr(y), StmtsCompiled::empty())
            }
            ExprCompiled::LogicalBinOp(ExprLogicalBinOp::Or, x_y) => {
                let (x, y) = *x_y;
                Self::if_stmt(expr.span, x, StmtsCompiled::empty(), Self::expr(y))
            }
            expr => {
                if let Some(t) = expr.as_type() {
                    StmtsCompiled::expr(t.clone())
                } else {
                    StmtsCompiled::one(IrSpanned {
                        span,
                        node: StmtCompiled::Expr(IrSpanned { span, node: expr }),
                    })
                }
            }
        }
    }

    fn if_stmt(
        span: FrameSpan,
        cond: IrSpanned<ExprCompiled>,
        t: StmtsCompiled,
        f: StmtsCompiled,
    ) -> StmtsCompiled {
        let cond = ExprCompiledBool::new(cond);
        match cond.node {
            ExprCompiledBool::Const(true) => t,
            ExprCompiledBool::Const(false) => f,
            ExprCompiledBool::Expr(cond) => match cond {
                ExprCompiled::Builtin1(Builtin1::Not, cond) => Self::if_stmt(span, *cond, f, t),
                ExprCompiled::Seq(x_cond) => {
                    let (x, cond) = *x_cond;
                    let mut stmt = StmtsCompiled::empty();
                    stmt.extend(Self::expr(x));
                    stmt.extend(Self::if_stmt(span, cond, t, f));
                    stmt
                }
                cond => {
                    let cond = IrSpanned { span, node: cond };
                    if t.is_empty() && f.is_empty() {
                        Self::expr(cond)
                    } else {
                        StmtsCompiled::one(IrSpanned {
                            span,
                            node: StmtCompiled::If(Box::new((cond, t, f))),
                        })
                    }
                }
            },
        }
    }

    fn for_stmt(
        span: FrameSpan,
        var: IrSpanned<AssignCompiledValue>,
        over: IrSpanned<ExprCompiled>,
        body: StmtsCompiled,
    ) -> StmtsCompiled {
        if over.is_iterable_empty() {
            return StmtsCompiled::empty();
        }
        StmtsCompiled::one(IrSpanned {
            span,
            node: StmtCompiled::For(Box::new((var, over, body))),
        })
    }
}

#[derive(Debug, Error)]
pub(crate) enum AssignError {
    // Incorrect number of value to unpack (expected, got)
    #[error("Unpacked {1} values but expected {0}")]
    IncorrectNumberOfValueToUnpack(i32, i32),
}

#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) enum AssignCompiledValue {
    Dot(IrSpanned<ExprCompiled>, String),
    Index(IrSpanned<ExprCompiled>, IrSpanned<ExprCompiled>),
    Tuple(Vec<IrSpanned<AssignCompiledValue>>),
    Local(LocalSlotId),
    LocalCaptured(LocalCapturedSlotId),
    Module(ModuleSlotId, String),
}

impl AssignCompiledValue {
    /// Assignment to a local non-captured variable.
    pub(crate) fn as_local_non_captured(&self) -> Option<LocalSlotId> {
        match self {
            AssignCompiledValue::Local(id) => Some(*id),
            _ => None,
        }
    }
}

impl IrSpanned<AssignCompiledValue> {
    pub(crate) fn optimize(&self, ctx: &mut OptCtx) -> IrSpanned<AssignCompiledValue> {
        let span = self.span;
        let assign = match self.node {
            AssignCompiledValue::Dot(ref object, ref field) => {
                let object = object.optimize(ctx);
                let field = field.clone();
                AssignCompiledValue::Dot(object, field)
            }
            AssignCompiledValue::Index(ref array, ref index) => {
                let array = array.optimize(ctx);
                let index = index.optimize(ctx);
                AssignCompiledValue::Index(array, index)
            }
            AssignCompiledValue::Tuple(ref xs) => {
                let xs = xs.map(|x| x.optimize(ctx));
                AssignCompiledValue::Tuple(xs)
            }
            ref e @ (AssignCompiledValue::Local(..)
            | AssignCompiledValue::LocalCaptured(..)
            | AssignCompiledValue::Module(..)) => e.clone(),
        };
        IrSpanned { node: assign, span }
    }
}

impl Compiler<'_, '_, '_, '_> {
    pub fn assign_target(
        &mut self,
        expr: &CstAssignTarget,
    ) -> Result<IrSpanned<AssignCompiledValue>, CompilerInternalError> {
        let span = FrameSpan::new(FrozenFileSpan::new(self.codemap, expr.span));
        let assign = match &expr.node {
            AssignTargetP::Dot(e, s) => {
                let e = self.expr(e)?;
                let s = &s.node;
                AssignCompiledValue::Dot(e, s.to_owned())
            }
            AssignTargetP::Index(e_idx) => {
                let (e, idx) = &**e_idx;
                let e = self.expr(e)?;
                let idx = self.expr(idx)?;
                AssignCompiledValue::Index(e, idx)
            }
            AssignTargetP::Tuple(v) => {
                let v = v
                    .iter()
                    .map(|x| self.assign_target(x))
                    .collect::<Result<_, CompilerInternalError>>()?;
                AssignCompiledValue::Tuple(v)
            }
            AssignTargetP::Identifier(ident) => {
                let name = ident.node.ident.as_str();
                let binding_id = ident
                    .node
                    .payload
                    .unwrap_or_else(|| panic!("unresolved binding: `{}`", name));
                let binding = self.scope_data.get_binding(binding_id);
                let slot = binding.resolved_slot(&self.codemap).unwrap();
                match (slot, binding.captured) {
                    (Slot::Local(slot), Captured::No) => {
                        AssignCompiledValue::Local(LocalSlotId(slot.0))
                    }
                    (Slot::Local(slot), Captured::Yes) => {
                        AssignCompiledValue::LocalCaptured(LocalCapturedSlotId(slot.0))
                    }
                    (Slot::Module(slot), _) => AssignCompiledValue::Module(slot, name.to_owned()),
                }
            }
        };
        Ok(IrSpanned { node: assign, span })
    }

    fn assign_modify(
        &mut self,
        span_stmt: Span,
        lhs: &CstAssignTarget,
        rhs: IrSpanned<ExprCompiled>,
        op: AssignOp,
    ) -> Result<StmtsCompiled, CompilerInternalError> {
        let span_stmt = FrameSpan::new(FrozenFileSpan::new(self.codemap, span_stmt));
        let span_lhs = FrameSpan::new(FrozenFileSpan::new(self.codemap, lhs.span));
        match &lhs.node {
            AssignTargetP::Dot(e, s) => {
                let e = self.expr(e)?;
                Ok(StmtsCompiled::one(IrSpanned {
                    span: span_stmt,
                    node: StmtCompiled::AssignModify(
                        AssignModifyLhs::Dot(e, s.node.clone()),
                        op,
                        rhs,
                    ),
                }))
            }
            AssignTargetP::Index(e_idx) => {
                let (e, idx) = &**e_idx;
                let e = self.expr(e)?;
                let idx = self.expr(idx)?;
                Ok(StmtsCompiled::one(IrSpanned {
                    span: span_stmt,
                    node: StmtCompiled::AssignModify(AssignModifyLhs::Array(e, idx), op, rhs),
                }))
            }
            AssignTargetP::Identifier(ident) => {
                let (slot, captured) = self.scope_data.get_assign_ident_slot(ident, &self.codemap);
                match (slot, captured) {
                    (Slot::Local(slot), Captured::No) => {
                        let lhs = IrSpanned {
                            node: LocalSlotId(slot.0),
                            span: span_lhs,
                        };
                        Ok(StmtsCompiled::one(IrSpanned {
                            span: span_stmt,
                            node: StmtCompiled::AssignModify(AssignModifyLhs::Local(lhs), op, rhs),
                        }))
                    }
                    (Slot::Local(slot), Captured::Yes) => {
                        let lhs = IrSpanned {
                            node: LocalCapturedSlotId(slot.0),
                            span: span_lhs,
                        };
                        Ok(StmtsCompiled::one(IrSpanned {
                            span: span_stmt,
                            node: StmtCompiled::AssignModify(
                                AssignModifyLhs::LocalCaptured(lhs),
                                op,
                                rhs,
                            ),
                        }))
                    }
                    (Slot::Module(slot), _) => {
                        let lhs = IrSpanned {
                            node: slot,
                            span: span_lhs,
                        };
                        Ok(StmtsCompiled::one(IrSpanned {
                            span: span_stmt,
                            node: StmtCompiled::AssignModify(AssignModifyLhs::Module(lhs), op, rhs),
                        }))
                    }
                }
            }
            AssignTargetP::Tuple(_) => {
                unreachable!("Assign modify validates that the LHS is never a tuple")
            }
        }
    }
}

// There are two requirements to perform a GC:
//
// 1. We can't be profiling, since profiling relies on the redundant heap
//    entries. When profiling we set disable_gc.
// 2. We must be able to access all roots.
//
// We track as many roots as possible, and eventually aim to track them all, but
// for the moment we're only sure we have all roots when we are in the module
// evaluation eval. There are three roots we don't yet know about:
//
// 1. When evaluating an expression which has multiple subexpressions, e.g. List
//    we can't GC during that, as we can't see the root of the first list.
// 2. When evaluating inside a native function, especially if that native
//    function calls back to a non-native function, e.g. sort with a comparison
//    function.
// 3. When iterating we freeze the iteration variable, which means it
//    can't be moved by a GC. A special type of root.
//
// The first issue can be solved by moving to a bytecode interpreter and
// evaluation stack. The second issue can be solved by disabling GC while in
// such functions (it's probably rare). The third issue could be solved by
// making the freeze for iteration a separate flag to the RefCell, at the cost
// of an extra word in ValueMem. Or we could disable GC while iterating.
//
// For the moment we only GC when executing a statement at the root of the
// module, which we know is safe with respect to all three conditions.
//
// We also require that `extra_v` is None, since otherwise the user might have
// additional values stashed somewhere.
pub(crate) fn possible_gc(eval: &mut Evaluator) {
    if !eval.disable_gc && eval.heap().allocated_bytes() >= eval.next_gc_level {
        // When we are at a module scope (as checked above) the eval contains
        // references to all values, so walking covers everything and the unsafe
        // is satisfied.
        unsafe { eval.garbage_collect() }
        eval.next_gc_level = cmp::max(eval.heap().allocated_bytes() * 2, GC_THRESHOLD);
    }
}

/// Implement lhs |= rhs, which is special in Starlark, because dicts are mutated,
/// while all other types are not.
pub(crate) fn bit_or_assign<'v>(
    lhs: Value<'v>,
    rhs: Value<'v>,
    heap: &'v Heap,
) -> crate::Result<Value<'v>> {
    // The Starlark spec says dict |= mutates, while nothing else does.
    // When mutating, be careful if they alias, so we don't have `lhs`
    // mutably borrowed when we iterate over `rhs`, as they might alias.

    let lhs_aref = lhs.get_ref();
    let lhs_ty = lhs_aref.vtable().static_type_of_value.get();

    if Dict::is_dict_type(lhs_ty) {
        let mut dict = DictMut::from_value(lhs)?;
        if lhs.ptr_eq(rhs) {
            // Nothing to do as union is idempotent
        } else {
            let rhs = DictRef::from_value(rhs).map_or_else(
                || {
                    ValueError::unsupported_owned(
                        lhs_aref.vtable().type_name,
                        "|=",
                        Some(rhs.get_type()),
                    )
                },
                Ok,
            )?;
            for (k, v) in rhs.iter_hashed() {
                dict.aref.insert_hashed(k, v);
            }
        }
        Ok(lhs)
    } else {
        lhs_aref.bit_or(rhs, heap)
    }
}

/// Implement lhs += rhs, which is special in Starlark, because lists are mutated,
/// while all other types are not.
pub(crate) fn add_assign<'v>(
    lhs: Value<'v>,
    rhs: Value<'v>,
    heap: &'v Heap,
) -> crate::Result<Value<'v>> {
    // Checking whether a value is an integer or a string is cheap (no virtual call),
    // and `Value::add` has optimizations for these types, so check them first
    // and delegate to `Value::add`.
    if lhs.unpack_inline_int().is_some() || lhs.is_str() {
        return lhs.add(rhs, heap);
    }

    // The Starlark spec says list += mutates, while nothing else does.
    // When mutating, be careful if they alias, so we don't have `lhs`
    // mutably borrowed when we iterate over `rhs`, as they might alias.

    // In practice, select is the only thing that implements radd.
    // If the users does x += select(...) we don't want an error,
    // we really want to x = x + select, so check radd first.
    let lhs_aref = lhs.get_ref();
    let lhs_ty = lhs_aref.vtable().static_type_of_value.get();

    if ListData::is_list_type(lhs_ty) {
        if let Some(v) = rhs.get_ref().radd(lhs, heap) {
            v
        } else {
            let list = ListData::from_value_mut(lhs)?;
            if lhs.ptr_eq(rhs) {
                list.double(heap);
            } else {
                // TODO: if RHS is list, consider calling `List::extend_from_slice`.
                list.extend(rhs.iterate(heap)?, heap);
            }
            Ok(lhs)
        }
    } else {
        lhs.add(rhs, heap)
    }
}

impl Compiler<'_, '_, '_, '_> {
    pub(crate) fn compile_context(&self, has_return_type: bool) -> StmtCompileContext {
        StmtCompileContext { has_return_type }
    }

    pub(crate) fn stmt(
        &mut self,
        stmt: &CstStmt,
        allow_gc: bool,
    ) -> Result<StmtsCompiled, CompilerInternalError> {
        let span = FrameSpan::new(FrozenFileSpan::new(self.codemap, stmt.span));
        let is_statements = matches!(&stmt.node, StmtP::Statements(_));
        let res = self.stmt_direct(stmt, allow_gc)?;
        // No point inserting a GC point around statements, since they will contain inner statements we can do
        if allow_gc && !is_statements {
            // We could do this more efficiently by fusing the possible_gc
            // into the inner closure, but no real need - we insert allow_gc fairly rarely
            let mut with_gc = StmtsCompiled::one(IrSpanned {
                span,
                node: StmtCompiled::PossibleGc,
            });
            with_gc.extend(res);
            Ok(with_gc)
        } else {
            Ok(res)
        }
    }

    pub(crate) fn module_top_level_stmt(
        &mut self,
        stmt: &CstStmt,
    ) -> Result<StmtsCompiled, CompilerInternalError> {
        match &stmt.node {
            StmtP::Statements(..) => {
                unreachable!("top level statement lists are handled by outer loop")
            }
            StmtP::Expression(expr) => {
                let stmt = Spanned {
                    span: expr.span,
                    // When top level statement is an expression, compile it as return.
                    // This is used to obtain the result of evaluation
                    // of the last statement-expression in module.
                    // TODO(nga): unnecessary clone.
                    node: StmtP::Return(Some(expr.clone())),
                };
                self.stmt(&stmt, true)
            }
            _ => self.stmt(stmt, true),
        }
    }

    fn stmt_if(
        &mut self,
        span: FrameSpan,
        cond: &CstExpr,
        then_block: &CstStmt,
        allow_gc: bool,
    ) -> Result<StmtsCompiled, CompilerInternalError> {
        let cond = self.expr(cond)?;
        let then_block = self.stmt(then_block, allow_gc)?;
        Ok(StmtsCompiled::if_stmt(
            span,
            cond,
            then_block,
            StmtsCompiled::empty(),
        ))
    }

    fn stmt_if_else(
        &mut self,
        span: FrameSpan,
        cond: &CstExpr,
        then_block: &CstStmt,
        else_block: &CstStmt,
        allow_gc: bool,
    ) -> Result<StmtsCompiled, CompilerInternalError> {
        let cond = self.expr(cond)?;
        let then_block = self.stmt(then_block, allow_gc)?;
        let else_block = self.stmt(else_block, allow_gc)?;
        Ok(StmtsCompiled::if_stmt(span, cond, then_block, else_block))
    }

    fn stmt_expr(&mut self, expr: &CstExpr) -> Result<StmtsCompiled, CompilerInternalError> {
        let expr = self.expr(expr)?;
        Ok(StmtsCompiled::expr(expr))
    }

    fn stmt_direct(
        &mut self,
        stmt: &CstStmt,
        allow_gc: bool,
    ) -> Result<StmtsCompiled, CompilerInternalError> {
        let span = FrameSpan::new(FrozenFileSpan::new(self.codemap, stmt.span));
        match &stmt.node {
            StmtP::Def(def) => {
                let signature_span = def.signature_span();
                let signature_span = FrozenFileSpan::new(self.codemap, signature_span);
                let DefP {
                    name,
                    params,
                    return_type,
                    body,
                    payload: scope_id,
                } = def;
                let rhs = IrSpanned {
                    node: self.function(
                        &name.ident,
                        signature_span,
                        *scope_id,
                        params,
                        return_type.as_deref(),
                        body,
                    )?,
                    span,
                };
                let lhs = self.assign_target(&Spanned {
                    span: name.span,
                    node: AssignTargetP::Identifier(name.clone()),
                })?;
                Ok(StmtsCompiled::one(IrSpanned {
                    span,
                    node: StmtCompiled::Assign(lhs, None, rhs),
                }))
            }
            StmtP::For(ForP { var, over, body }) => {
                let over = list_to_tuple(over);
                let var = self.assign_target(var)?;
                let over = self.expr(&over)?;
                let st = self.stmt(body, false)?;
                Ok(StmtsCompiled::for_stmt(span, var, over, st))
            }
            StmtP::Return(None) => Ok(StmtsCompiled::one(IrSpanned {
                node: StmtCompiled::Return(IrSpanned {
                    span,
                    node: ExprCompiled::Value(FrozenValue::new_none()),
                }),
                span,
            })),
            StmtP::Return(Some(e)) => Ok(StmtsCompiled::one(IrSpanned {
                node: StmtCompiled::Return(self.expr(e)?),
                span,
            })),
            StmtP::If(cond, then_block) => self.stmt_if(span, cond, then_block, allow_gc),
            StmtP::IfElse(cond, then_block_else_block) => {
                let (then_block, else_block) = &**then_block_else_block;
                self.stmt_if_else(span, cond, then_block, else_block, allow_gc)
            }
            StmtP::Statements(stmts) => {
                let mut r = StmtsCompiled::empty();
                for stmt in stmts {
                    if r.is_terminal() {
                        break;
                    }
                    r.extend(self.stmt(stmt, allow_gc)?);
                }
                Ok(r)
            }
            StmtP::Expression(e) => self.stmt_expr(e),
            StmtP::Assign(AssignP { lhs, ty, rhs }) => {
                let rhs = self.expr(rhs)?;
                let ty = self.expr_for_type(ty.as_ref());
                let lhs = self.assign_target(lhs)?;
                Ok(StmtsCompiled::one(IrSpanned {
                    span,
                    node: StmtCompiled::Assign(lhs, ty, rhs),
                }))
            }
            StmtP::AssignModify(lhs, op, rhs) => {
                let rhs = self.expr(rhs)?;
                self.assign_modify(span.span.span(), lhs, rhs, *op)
            }
            StmtP::Load(..) => unreachable!(),
            StmtP::Pass => Ok(StmtsCompiled::empty()),
            StmtP::Break => Ok(StmtsCompiled::one(IrSpanned {
                span,
                node: StmtCompiled::Break,
            })),
            StmtP::Continue => Ok(StmtsCompiled::one(IrSpanned {
                span,
                node: StmtCompiled::Continue,
            })),
        }
    }
}
