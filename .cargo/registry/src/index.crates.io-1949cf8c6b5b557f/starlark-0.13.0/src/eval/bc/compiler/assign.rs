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

//! Compile assignment lhs.

use starlark_syntax::slice_vec_ext::SliceExt;

use crate::collections::symbol::symbol::Symbol;
use crate::eval::bc::compiler::expr::write_n_exprs;
use crate::eval::bc::instr_impl::InstrSetArrayIndex;
use crate::eval::bc::instr_impl::InstrSetObjectField;
use crate::eval::bc::instr_impl::InstrStoreModuleAndExport;
use crate::eval::bc::instr_impl::InstrUnpack;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::bc::writer::BcWriter;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::stmt::AssignCompiledValue;

impl AssignCompiledValue {
    /// After evaluation of `(x, y[z]) = ...`, variables `x`, `y` and `z` as definitely assigned.
    pub(crate) fn mark_definitely_assigned_after(&self, bc: &mut BcWriter) {
        match self {
            AssignCompiledValue::Dot(object, field) => {
                object.mark_definitely_assigned_after(bc);
                let _ = field;
            }
            AssignCompiledValue::Module(..) => {}
            AssignCompiledValue::Index(array, index) => {
                array.mark_definitely_assigned_after(bc);
                index.mark_definitely_assigned_after(bc);
            }
            AssignCompiledValue::LocalCaptured(_slot) => {}
            AssignCompiledValue::Local(slot) => {
                bc.mark_definitely_assigned(*slot);
            }
            AssignCompiledValue::Tuple(xs) => {
                for x in xs {
                    x.mark_definitely_assigned_after(bc);
                }
            }
        }
    }
}

impl IrSpanned<AssignCompiledValue> {
    pub(crate) fn write_bc(&self, value: BcSlotIn, bc: &mut BcWriter) {
        let span = self.span;
        match self.node {
            AssignCompiledValue::Dot(ref object, ref field) => {
                object.write_bc_cb(bc, |object, bc| {
                    let symbol = Symbol::new(field.as_str());
                    bc.write_instr::<InstrSetObjectField>(span, (value, object, symbol));
                });
            }
            AssignCompiledValue::Index(ref array, ref index) => {
                write_n_exprs([array, index], bc, |[array, index], bc| {
                    bc.write_instr::<InstrSetArrayIndex>(span, (value, array, index));
                });
            }
            AssignCompiledValue::Tuple(ref xs) => {
                // All assignments are to local variables, e. g.
                // ```
                // (x, y, z) = ...
                // ```
                // so we can avoid using intermediate register.
                let all_local = xs
                    .try_map(|x| {
                        x.as_local_non_captured()
                            .map(|l| l.to_bc_slot().to_out())
                            .ok_or(())
                    })
                    .ok();
                if let Some(all_local) = all_local {
                    let args = bc.heap.alloc_any_slice(&all_local);
                    bc.write_instr::<InstrUnpack>(span, (value, args));
                } else {
                    bc.alloc_slots(xs.len() as u32, |slots, bc| {
                        let args: Vec<BcSlotOut> = slots.iter().map(|s| s.to_out()).collect();
                        let args = bc.heap.alloc_any_slice(&args);
                        bc.write_instr::<InstrUnpack>(span, (value, args));

                        for (x, slot) in xs.iter().zip(slots.iter()) {
                            x.write_bc(slot.to_in(), bc);
                        }
                    });
                }
            }
            AssignCompiledValue::Local(slot) => {
                bc.write_mov(span, value, slot.to_bc_slot().to_out());
            }
            AssignCompiledValue::LocalCaptured(slot) => {
                bc.write_store_local_captured(span, value, slot);
            }
            AssignCompiledValue::Module(slot, ref name) => {
                bc.write_instr::<InstrStoreModuleAndExport>(span, (value, slot, name.clone()));
            }
        }
    }
}
