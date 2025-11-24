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

//! Write operators like `+=`.

use starlark_syntax::syntax::ast::AssignOp;

use crate::collections::symbol::symbol::Symbol;
use crate::eval::bc::compiler::expr::write_n_exprs;
use crate::eval::bc::instr_impl::InstrAddAssign;
use crate::eval::bc::instr_impl::InstrArrayIndex;
use crate::eval::bc::instr_impl::InstrArrayIndexSet;
use crate::eval::bc::instr_impl::InstrBitAnd;
use crate::eval::bc::instr_impl::InstrBitOrAssign;
use crate::eval::bc::instr_impl::InstrBitXor;
use crate::eval::bc::instr_impl::InstrDivide;
use crate::eval::bc::instr_impl::InstrFloorDivide;
use crate::eval::bc::instr_impl::InstrLeftShift;
use crate::eval::bc::instr_impl::InstrLoadModule;
use crate::eval::bc::instr_impl::InstrMultiply;
use crate::eval::bc::instr_impl::InstrObjectField;
use crate::eval::bc::instr_impl::InstrPercent;
use crate::eval::bc::instr_impl::InstrRightShift;
use crate::eval::bc::instr_impl::InstrSetObjectField;
use crate::eval::bc::instr_impl::InstrStoreModule;
use crate::eval::bc::instr_impl::InstrSub;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::bc::stack_ptr::BcSlotsN;
use crate::eval::bc::writer::BcWriter;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::stmt::AssignModifyLhs;
use crate::eval::runtime::frame_span::FrameSpan;

trait AssignOnWriteBc {
    fn write_bc(
        self,
        v0: BcSlotIn,
        v1: BcSlotIn,
        target: BcSlotOut,
        span: FrameSpan,
        bc: &mut BcWriter,
    );
}

impl AssignOnWriteBc for AssignOp {
    fn write_bc(
        self,
        v0: BcSlotIn,
        v1: BcSlotIn,
        target: BcSlotOut,
        span: FrameSpan,
        bc: &mut BcWriter,
    ) {
        let arg = (v0, v1, target);
        match self {
            AssignOp::Add => bc.write_instr::<InstrAddAssign>(span, arg),
            AssignOp::Subtract => bc.write_instr::<InstrSub>(span, arg),
            AssignOp::Multiply => bc.write_instr::<InstrMultiply>(span, arg),
            AssignOp::Divide => bc.write_instr::<InstrDivide>(span, arg),
            AssignOp::FloorDivide => bc.write_instr::<InstrFloorDivide>(span, arg),
            AssignOp::Percent => bc.write_instr::<InstrPercent>(span, arg),
            AssignOp::BitAnd => bc.write_instr::<InstrBitAnd>(span, arg),
            AssignOp::BitOr => bc.write_instr::<InstrBitOrAssign>(span, arg),
            AssignOp::BitXor => bc.write_instr::<InstrBitXor>(span, arg),
            AssignOp::LeftShift => bc.write_instr::<InstrLeftShift>(span, arg),
            AssignOp::RightShift => bc.write_instr::<InstrRightShift>(span, arg),
        }
    }
}

impl AssignModifyLhs {
    /// After evaluation of `x[y] += ...`, variables `x` and `y` are definitely assigned.
    pub(crate) fn mark_definitely_assigned_after(&self, bc: &mut BcWriter) {
        match self {
            AssignModifyLhs::Dot(object, _field) => object.mark_definitely_assigned_after(bc),
            AssignModifyLhs::Array(array, index) => {
                array.mark_definitely_assigned_after(bc);
                index.mark_definitely_assigned_after(bc);
            }
            AssignModifyLhs::LocalCaptured(_) => {}
            AssignModifyLhs::Local(local) => bc.mark_definitely_assigned(local.node),
            AssignModifyLhs::Module(_) => {}
        }
    }

    pub(crate) fn write_bc(
        &self,
        span: FrameSpan,
        op: AssignOp,
        rhs: &IrSpanned<ExprCompiled>,
        bc: &mut BcWriter,
    ) {
        match *self {
            AssignModifyLhs::Dot(ref object, ref field) => {
                object.write_bc_cb(bc, |object, bc| {
                    bc.alloc_slots_c(|lhs_rhs: BcSlotsN<2>, bc| {
                        let field = Symbol::new(field.as_str());
                        bc.write_instr::<InstrObjectField>(
                            span,
                            (object, field.clone(), lhs_rhs.get::<0>().to_out()),
                        );
                        rhs.write_bc(lhs_rhs.get::<1>().to_out(), bc);
                        op.write_bc(
                            lhs_rhs.get::<0>().to_in(),
                            lhs_rhs.get::<1>().to_in(),
                            lhs_rhs.get::<1>().to_out(),
                            span,
                            bc,
                        );
                        bc.write_instr::<InstrSetObjectField>(
                            span,
                            (lhs_rhs.get::<1>().to_in(), object, field),
                        );
                    })
                });
            }
            AssignModifyLhs::Array(ref array, ref index) => {
                write_n_exprs([array, index], bc, |[array, index], bc| {
                    bc.alloc_slots_c(|slots: BcSlotsN<2>, bc| {
                        let temp_slot = slots.get::<0>();
                        let rhs_slot = slots.get::<1>();

                        bc.write_instr::<InstrArrayIndex>(span, (array, index, temp_slot.to_out()));
                        rhs.write_bc(rhs_slot.to_out(), bc);
                        op.write_bc(
                            temp_slot.to_in(),
                            rhs_slot.to_in(),
                            temp_slot.to_out(),
                            span,
                            bc,
                        );
                        bc.write_instr::<InstrArrayIndexSet>(
                            span,
                            (array, index, temp_slot.to_in()),
                        );
                    })
                });
            }
            AssignModifyLhs::Local(s) => bc.alloc_slots_c(|lhs_rhs: BcSlotsN<2>, bc| {
                let slot = s.node;
                bc.write_load_local(span, slot, lhs_rhs.get::<0>().to_out());
                rhs.write_bc(lhs_rhs.get::<1>().to_out(), bc);

                op.write_bc(
                    lhs_rhs.get::<0>().to_in(),
                    lhs_rhs.get::<1>().to_in(),
                    lhs_rhs.get::<1>().to_out(),
                    span,
                    bc,
                );
                bc.write_mov(span, lhs_rhs.get::<1>().to_in(), slot.to_bc_slot().to_out());
            }),
            AssignModifyLhs::LocalCaptured(s) => bc.alloc_slots_c(|lhs_rhs: BcSlotsN<2>, bc| {
                let slot = s.node;
                bc.write_load_local_captured(span, slot, lhs_rhs.get::<0>().to_out());
                rhs.write_bc(lhs_rhs.get::<1>().to_out(), bc);

                op.write_bc(
                    lhs_rhs.get::<0>().to_in(),
                    lhs_rhs.get::<1>().to_in(),
                    lhs_rhs.get::<1>().to_out(),
                    span,
                    bc,
                );
                bc.write_store_local_captured(span, lhs_rhs.get::<1>().to_in(), slot);
            }),
            AssignModifyLhs::Module(m) => bc.alloc_slots_c(|lhs_rhs: BcSlotsN<2>, bc| {
                let slot = m.node;
                bc.write_instr::<InstrLoadModule>(span, (slot, lhs_rhs.get::<0>().to_out()));
                rhs.write_bc(lhs_rhs.get::<1>().to_out(), bc);
                op.write_bc(
                    lhs_rhs.get::<0>().to_in(),
                    lhs_rhs.get::<1>().to_in(),
                    lhs_rhs.get::<1>().to_out(),
                    span,
                    bc,
                );
                bc.write_instr::<InstrStoreModule>(span, (lhs_rhs.get::<1>().to_in(), slot));
            }),
        }
    }
}
