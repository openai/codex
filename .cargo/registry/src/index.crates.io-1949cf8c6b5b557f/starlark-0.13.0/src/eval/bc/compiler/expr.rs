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

//! Compile expressions.

use std::collections::HashSet;

use starlark_syntax::slice_vec_ext::SliceExt;

use crate::collections::Hashed;
use crate::collections::SmallMap;
use crate::eval::bc::compiler::if_compiler::write_if_else;
use crate::eval::bc::instr_impl::*;
use crate::eval::bc::slow_arg::BcInstrSlowArg;
use crate::eval::bc::stack_ptr::BcSlot;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::stack_ptr::BcSlotInRange;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::bc::writer::BcWriter;
use crate::eval::compiler::expr::Builtin1;
use crate::eval::compiler::expr::Builtin2;
use crate::eval::compiler::expr::CompareOp;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::expr::ExprLogicalBinOp;
use crate::eval::compiler::expr::MaybeNot;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::values::layout::value_not_special::FrozenValueNotSpecial;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::ValueLike;

/// Try extract consecutive definitely initialized locals from expressions.
fn try_slot_range<'a>(
    exprs: impl IntoIterator<Item = &'a IrSpanned<ExprCompiled>>,
    bc: &BcWriter,
) -> Option<BcSlotInRange> {
    let mut range = BcSlotInRange::default();
    for expr in exprs {
        let local = expr.as_local_non_captured()?;
        let slot = bc.try_definitely_assigned(local)?;
        if !range.try_push(slot) {
            return None;
        }
    }
    Some(range)
}

/// Compile several expressions into consecutive registers.
pub(crate) fn write_exprs<'a>(
    exprs: impl IntoIterator<Item = &'a IrSpanned<ExprCompiled>>,
    bc: &mut BcWriter,
    k: impl FnOnce(BcSlotInRange, &mut BcWriter),
) {
    let exprs: Vec<_> = exprs.into_iter().collect();

    if let Some(slots) = try_slot_range(exprs.iter().copied(), bc) {
        k(slots, bc);
    } else {
        bc.alloc_slots_for_exprs(exprs, |slot, expr, bc| expr.write_bc(slot.to_out(), bc), k)
    }
}

pub(crate) fn write_expr_opt(
    expr: &Option<IrSpanned<ExprCompiled>>,
    bc: &mut BcWriter,
    k: impl FnOnce(Option<BcSlotIn>, &mut BcWriter),
) {
    if let Some(expr) = expr {
        expr.write_bc_cb(bc, |slot, bc| k(Some(slot), bc))
    } else {
        k(None, bc)
    }
}

pub(crate) fn write_n_exprs<const N: usize>(
    exprs: [&IrSpanned<ExprCompiled>; N],
    bc: &mut BcWriter,
    k: impl FnOnce([BcSlotIn; N], &mut BcWriter),
) {
    fn help<const N: usize>(
        mut filled: [BcSlotIn; N],
        rem_exprs: &[&IrSpanned<ExprCompiled>],
        bc: &mut BcWriter,
        k: impl FnOnce([BcSlotIn; N], &mut BcWriter),
    ) {
        match rem_exprs.split_first() {
            Some((first, rem)) => first.write_bc_cb(bc, |first, bc| {
                filled[N - rem.len() - 1] = first;
                help(filled, rem, bc, k)
            }),
            None => k(filled, bc),
        }
    }

    help([BcSlot(98765).to_in(); N], &exprs, bc, k)
}

impl ExprCompiled {
    /// Mark variables which are definitely assigned after execution of this expression.
    ///
    /// For example, when this expression if executed:
    ///
    /// ```python
    /// t if c else f
    /// ```
    ///
    /// `c` is definitely assigned (because if it is not, then execution fails),
    /// but we don't know about `t` or `f` because one of them was not executed.
    pub(crate) fn mark_definitely_assigned_after(&self, bc: &mut BcWriter) {
        match self {
            ExprCompiled::Value(_) => {}
            ExprCompiled::Local(local) => bc.mark_definitely_assigned(*local),
            ExprCompiled::LocalCaptured(_) => {}
            ExprCompiled::Module(_) => {}
            ExprCompiled::Tuple(xs) | ExprCompiled::List(xs) => {
                for x in xs {
                    x.mark_definitely_assigned_after(bc);
                }
            }
            ExprCompiled::Dict(xs) => {
                for (k, v) in xs {
                    k.mark_definitely_assigned_after(bc);
                    v.mark_definitely_assigned_after(bc);
                }
            }
            ExprCompiled::Compr(compr) => compr.mark_definitely_assigned_after(bc),
            ExprCompiled::If(c_t_f) => {
                let (c, _t, _f) = &**c_t_f;
                // Condition is executed unconditionally, so we use it to mark definitely assigned.
                // But we don't know which of the branches will be executed.
                c.mark_definitely_assigned_after(bc);
            }
            ExprCompiled::Slice(l_a_b_c) => {
                let (l, a, b, c) = &**l_a_b_c;
                l.mark_definitely_assigned_after(bc);
                if let Some(a) = a {
                    a.mark_definitely_assigned_after(bc);
                }
                if let Some(b) = b {
                    b.mark_definitely_assigned_after(bc);
                }
                if let Some(c) = c {
                    c.mark_definitely_assigned_after(bc);
                }
            }
            ExprCompiled::Builtin1(_op, expr) => {
                expr.mark_definitely_assigned_after(bc);
            }
            ExprCompiled::LogicalBinOp(_op, a_b) => {
                let (a, b) = &**a_b;
                // `a` is executed unconditionally, but `b` is not,
                // so we mark only `a` as definitely assigned.
                a.mark_definitely_assigned_after(bc);
                let _ = b;
            }
            ExprCompiled::Seq(a_b) => {
                let (a, b) = &**a_b;
                a.mark_definitely_assigned_after(bc);
                b.mark_definitely_assigned_after(bc);
            }
            ExprCompiled::Builtin2(_op, a_b) => {
                let (a, b) = &**a_b;
                a.mark_definitely_assigned_after(bc);
                b.mark_definitely_assigned_after(bc);
            }
            ExprCompiled::Index2(a_i0_i1) => {
                let (a, i0, i1) = &**a_i0_i1;
                a.mark_definitely_assigned_after(bc);
                i0.mark_definitely_assigned_after(bc);
                i1.mark_definitely_assigned_after(bc);
            }
            ExprCompiled::Call(c) => c.mark_definitely_assigned_after(bc),
            ExprCompiled::Def(d) => d.mark_definitely_assigned_after(bc),
        }
    }
}

impl IrSpanned<ExprCompiled> {
    fn try_dict_of_consts(
        xs: &[(IrSpanned<ExprCompiled>, IrSpanned<ExprCompiled>)],
    ) -> Option<SmallMap<FrozenValue, FrozenValue>> {
        let mut res = SmallMap::new();
        for (k, v) in xs {
            let k = k.as_value()?.get_hashed().ok()?;
            let v = v.as_value()?;
            let prev = res.insert_hashed(k, v);
            if prev.is_some() {
                // If there are duplicates, so don't take the fast-literal
                // path and go down the slow runtime path (which will raise the error).
                // We have a lint that will likely fire on this issue (and others).
                return None;
            }
        }
        Some(res)
    }

    fn try_dict_const_keys(
        xs: &[(IrSpanned<ExprCompiled>, IrSpanned<ExprCompiled>)],
    ) -> Option<Box<[Hashed<FrozenValue>]>> {
        let mut keys = Vec::new();
        let mut keys_unique = HashSet::new();
        for (k, _) in xs {
            let k = k.as_value()?.get_hashed().ok()?;
            keys.push(k);
            let inserted = keys_unique.insert(k);
            if !inserted {
                // Otherwise fail at runtime
                return None;
            }
        }
        Some(keys.into_boxed_slice())
    }

    fn write_dict(
        span: FrameSpan,
        xs: &[(IrSpanned<ExprCompiled>, IrSpanned<ExprCompiled>)],
        target: BcSlotOut,
        bc: &mut BcWriter,
    ) {
        if xs.is_empty() {
            bc.write_instr::<InstrDictNew>(span, target);
        } else if let Some(d) = Self::try_dict_of_consts(xs) {
            bc.write_instr::<InstrDictOfConsts>(span, (d, target));
        } else if let Some(keys) = Self::try_dict_const_keys(xs) {
            assert_eq!(keys.len(), xs.len());
            write_exprs(xs.iter().map(|(_, v)| v), bc, |values, bc| {
                assert_eq!(values.len() as usize, keys.len());
                bc.write_instr::<InstrDictConstKeys>(span, (keys, values.to_range_from(), target));
            });
        } else {
            let key_spans = xs.map(|(k, _v)| k.span);
            write_exprs(xs.iter().flat_map(|(k, v)| [k, v]), bc, |kvs, bc| {
                bc.write_instr_explicit::<InstrDictNPop>(
                    BcInstrSlowArg {
                        span,
                        spans: key_spans,
                    },
                    (kvs, target),
                );
            });
        }
    }

    fn write_not(expr: &IrSpanned<ExprCompiled>, target: BcSlotOut, bc: &mut BcWriter) {
        expr.write_bc_cb(bc, |slot, bc| {
            bc.write_instr::<InstrNot>(expr.span, (slot, target));
        });
    }

    fn write_equals_const(
        span: FrameSpan,
        a: &IrSpanned<ExprCompiled>,
        b: FrozenValue,
        target: BcSlotOut,
        bc: &mut BcWriter,
    ) {
        a.write_bc_cb(bc, |a, bc| {
            if let Some(b) = b.to_value().unpack_int_value() {
                bc.write_instr::<InstrEqInt>(span, (a, b, target));
            } else if b.eq_is_ptr_eq() {
                bc.write_instr::<InstrEqPtr>(span, (a, b, target));
            } else if let Some(b) = FrozenStringValue::new(b) {
                bc.write_instr::<InstrEqStr>(span, (a, b, target));
            } else if let Some(b) = FrozenValueNotSpecial::new(b) {
                bc.write_instr::<InstrEqConst>(span, (a, b, target));
            } else {
                unreachable!("FrozenValue must be either i32, str or not-special");
            }
        });
    }

    fn write_equals(
        span: FrameSpan,
        a: &IrSpanned<ExprCompiled>,
        b: &IrSpanned<ExprCompiled>,
        target: BcSlotOut,
        bc: &mut BcWriter,
    ) {
        if let Some(a) = a.as_value() {
            Self::write_equals_const(span, b, a, target, bc);
        } else if let Some(b) = b.as_value() {
            Self::write_equals_const(span, a, b, target, bc);
        } else {
            write_n_exprs([a, b], bc, |[a, b], bc| {
                bc.write_instr::<InstrEq>(span, (a, b, target));
            });
        }
    }

    pub(crate) fn write_bc(&self, target: BcSlotOut, bc: &mut BcWriter) {
        let span = self.span;
        match &self.node {
            ExprCompiled::Value(v) => {
                bc.write_const(span, *v, target);
            }
            ExprCompiled::Local(slot) => {
                bc.write_load_local(span, *slot, target);
            }
            ExprCompiled::LocalCaptured(slot) => {
                bc.write_load_local_captured(span, *slot, target);
            }
            ExprCompiled::Module(slot) => {
                bc.write_instr::<InstrLoadModule>(span, (*slot, target));
            }
            ExprCompiled::Tuple(xs) => {
                write_exprs(xs, bc, |xs, bc| {
                    bc.write_instr::<InstrTupleNPop>(span, (xs, target));
                });
            }
            ExprCompiled::List(ref xs) => {
                if xs.is_empty() {
                    bc.write_instr::<InstrListNew>(span, target);
                } else if xs.iter().all(|x| x.as_value().is_some()) {
                    let content = xs.map(|v| v.as_value().unwrap()).into_boxed_slice();
                    bc.write_instr::<InstrListOfConsts>(span, (content, target));
                } else {
                    write_exprs(xs, bc, |xs, bc| {
                        bc.write_instr::<InstrListNPop>(span, (xs, target));
                    });
                }
            }
            ExprCompiled::Dict(ref xs) => Self::write_dict(span, xs, target, bc),
            ExprCompiled::Compr(ref compr) => compr.write_bc(span, target, bc),
            ExprCompiled::Slice(l_start_stop_step) => {
                let (l, start, stop, step) = &**l_start_stop_step;
                l.write_bc_cb(bc, |l, bc| {
                    write_expr_opt(start, bc, |start, bc| {
                        write_expr_opt(stop, bc, |stop, bc| {
                            write_expr_opt(step, bc, |step, bc| {
                                bc.write_instr::<InstrSlice>(span, (l, start, stop, step, target))
                            })
                        })
                    })
                });
            }
            ExprCompiled::Builtin1(Builtin1::Not, expr) => Self::write_not(expr, target, bc),
            ExprCompiled::Builtin1(op, expr) => {
                expr.write_bc_cb(bc, |expr, bc| {
                    let arg = (expr, target);
                    match op {
                        Builtin1::Not => unreachable!("handled above"),
                        Builtin1::Minus => bc.write_instr::<InstrMinus>(span, arg),
                        Builtin1::Plus => bc.write_instr::<InstrPlus>(span, arg),
                        Builtin1::BitNot => bc.write_instr::<InstrBitNot>(span, arg),
                        Builtin1::TypeIs(t) => {
                            bc.write_instr::<InstrTypeIs>(span, (expr, *t, target))
                        }
                        Builtin1::PercentSOne(before, after) => bc
                            .write_instr::<InstrPercentSOne>(span, (*before, expr, *after, target)),
                        Builtin1::FormatOne(before, after) => {
                            bc.write_instr::<InstrFormatOne>(span, (*before, expr, *after, target))
                        }
                        Builtin1::Dot(field) => {
                            bc.write_instr::<InstrObjectField>(span, (expr, field.clone(), target))
                        }
                    }
                });
            }
            ExprCompiled::If(cond_t_f) => {
                let (cond, t, f) = &**cond_t_f;
                write_if_else(
                    cond,
                    |bc| t.write_bc(target, bc),
                    |bc| f.write_bc(target, bc),
                    bc,
                );
            }
            ExprCompiled::LogicalBinOp(op, l_r) => {
                let (l, r) = &**l_r;
                l.write_bc_cb(bc, |l_slot, bc| {
                    let maybe_not = match op {
                        ExprLogicalBinOp::And => MaybeNot::Id,
                        ExprLogicalBinOp::Or => MaybeNot::Not,
                    };
                    bc.write_if_else(
                        l_slot,
                        maybe_not,
                        l.span,
                        |bc| r.write_bc(target, bc),
                        |bc| bc.write_mov(span, l_slot, target),
                    );
                });
            }
            ExprCompiled::Seq(l_r) => {
                let (l, r) = &**l_r;
                l.write_bc_for_effect(bc);
                r.write_bc(target, bc);
            }
            ExprCompiled::Builtin2(Builtin2::Equals, l_r) => {
                let (l, r) = &**l_r;
                Self::write_equals(span, l, r, target, bc)
            }
            ExprCompiled::Builtin2(op, l_r) => {
                let (l, r) = &**l_r;
                write_n_exprs([l, r], bc, |[l, r], bc| {
                    let arg = (l, r, target);
                    match op {
                        Builtin2::Equals => unreachable!("handled above"),
                        Builtin2::Compare(CompareOp::Less) => {
                            bc.write_instr::<InstrLess>(span, arg)
                        }
                        Builtin2::Compare(CompareOp::Greater) => {
                            bc.write_instr::<InstrGreater>(span, arg)
                        }
                        Builtin2::Compare(CompareOp::LessOrEqual) => {
                            bc.write_instr::<InstrLessOrEqual>(span, arg)
                        }
                        Builtin2::Compare(CompareOp::GreaterOrEqual) => {
                            bc.write_instr::<InstrGreaterOrEqual>(span, arg)
                        }
                        Builtin2::In => bc.write_instr::<InstrIn>(span, arg),
                        Builtin2::Sub => bc.write_instr::<InstrSub>(span, arg),
                        Builtin2::Add => bc.write_instr::<InstrAdd>(span, arg),
                        Builtin2::Multiply => bc.write_instr::<InstrMultiply>(span, arg),
                        Builtin2::Divide => bc.write_instr::<InstrDivide>(span, arg),
                        Builtin2::FloorDivide => bc.write_instr::<InstrFloorDivide>(span, arg),
                        Builtin2::Percent => bc.write_instr::<InstrPercent>(span, arg),
                        Builtin2::BitAnd => bc.write_instr::<InstrBitAnd>(span, arg),
                        Builtin2::BitOr => bc.write_instr::<InstrBitOr>(span, arg),
                        Builtin2::BitXor => bc.write_instr::<InstrBitXor>(span, arg),
                        Builtin2::LeftShift => bc.write_instr::<InstrLeftShift>(span, arg),
                        Builtin2::RightShift => bc.write_instr::<InstrRightShift>(span, arg),
                        Builtin2::ArrayIndex => bc.write_instr::<InstrArrayIndex>(span, arg),
                    }
                });
            }
            ExprCompiled::Index2(a_i0_i1) => {
                let (a, i0, i1) = &**a_i0_i1;
                write_n_exprs([a, i0, i1], bc, |[a, i0, i1], bc| {
                    bc.write_instr::<InstrArrayIndex2>(span, (a, i0, i1, target))
                });
            }
            ExprCompiled::Call(ref call) => call.write_bc(target, bc),
            ExprCompiled::Def(ref def) => def.write_bc(span, target, bc),
        }
    }

    /// Allocate temporary slot, write expression into it,
    /// and then consume the slot with the callback.
    pub(crate) fn write_bc_cb<R>(
        &self,
        bc: &mut BcWriter,
        k: impl FnOnce(BcSlotIn, &mut BcWriter) -> R,
    ) -> R {
        if let Some(local) = self.as_local_non_captured() {
            // Local is known to be definitely assigned, so there's no need
            // to "load" it just to trigger check that it is assigned.
            if let Some(slot) = bc.try_definitely_assigned(local) {
                return k(slot, bc);
            }
        }

        bc.alloc_slot(|slot, bc| {
            self.write_bc(slot.to_out(), bc);
            k(slot.to_in(), bc)
        })
    }

    pub(crate) fn write_bc_for_effect(&self, bc: &mut BcWriter) {
        self.write_bc_cb(bc, |slot, _bc| {
            let _ = slot;
        });
    }
}
