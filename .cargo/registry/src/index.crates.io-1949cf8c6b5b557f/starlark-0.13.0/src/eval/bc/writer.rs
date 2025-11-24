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

//! Bytecode writer.

use std::cmp;

use crate::cast::transmute;
use crate::eval::bc::addr::BcAddr;
use crate::eval::bc::addr::BcAddrOffset;
use crate::eval::bc::bytecode::Bc;
use crate::eval::bc::definitely_assigned::BcDefinitelyAssigned;
use crate::eval::bc::for_loop::LoopDepth;
use crate::eval::bc::instr::BcInstr;
use crate::eval::bc::instr_impl::InstrBr;
use crate::eval::bc::instr_impl::InstrBreak;
use crate::eval::bc::instr_impl::InstrConst;
use crate::eval::bc::instr_impl::InstrContinue;
use crate::eval::bc::instr_impl::InstrIfBr;
use crate::eval::bc::instr_impl::InstrIfNotBr;
use crate::eval::bc::instr_impl::InstrIter;
use crate::eval::bc::instr_impl::InstrIterStop;
use crate::eval::bc::instr_impl::InstrLoadLocal;
use crate::eval::bc::instr_impl::InstrLoadLocalCaptured;
use crate::eval::bc::instr_impl::InstrMov;
use crate::eval::bc::instr_impl::InstrStoreLocalCaptured;
use crate::eval::bc::instrs::BcInstrsWriter;
use crate::eval::bc::instrs::PatchAddr;
use crate::eval::bc::repr::BC_INSTR_ALIGN;
use crate::eval::bc::slow_arg::BcInstrSlowArg;
use crate::eval::bc::stack_ptr::BcSlot;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::stack_ptr::BcSlotInRange;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::bc::stack_ptr::BcSlotRange;
use crate::eval::bc::stack_ptr::BcSlotsN;
use crate::eval::compiler::expr::MaybeNot;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::slots::LocalCapturedSlotId;
use crate::eval::runtime::slots::LocalSlotId;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;

#[derive(Debug)]
pub(crate) struct BcStmtLoc {
    pub(crate) span: FrameSpan,
}

/// This records the locations of the first instruction for each starlark statement. It's effectively
/// Map<BcAddr, BcStmtLoc>. This is very performance sensitive (when profiling/debugging are enabled we
/// do a lookup for every instruction) and so it's implemented as a vec of statements and then a vec of
/// statement indexes for each possible BcAddr in a bytecode Bc.
pub(crate) struct BcStatementLocations {
    pub(crate) locs: Vec<BcStmtLoc>,
    /// Map bytecode offset to index in `locs`.
    pub(crate) stmts: Vec<u32>,
}

impl BcStatementLocations {
    pub(crate) fn new() -> Self {
        Self {
            locs: Vec::new(),
            stmts: Vec::new(),
        }
    }

    fn idx_for(addr: BcAddr) -> usize {
        let addr = addr.0 as usize;
        debug_assert!(addr % BC_INSTR_ALIGN == 0);
        addr / BC_INSTR_ALIGN
    }

    fn push(&mut self, addr: BcAddr, span: BcStmtLoc) {
        let idx = Self::idx_for(addr);
        let stmt_idx = self.locs.len().try_into().unwrap();
        self.locs.push(span);
        while self.stmts.len() <= idx {
            self.stmts.push(u32::MAX);
        }
        // we could use .push() to get this in place, but doing by index just makes it clearer that we're doing it correctly.
        self.stmts[idx] = stmt_idx;
    }

    pub(crate) fn stmt_at(&self, offset: BcAddr) -> Option<&BcStmtLoc> {
        match self.stmts.get(Self::idx_for(offset)) {
            None | Some(&u32::MAX) => None,
            Some(v) => Some(&self.locs[*v as usize]),
        }
    }
}

/// For loop during bytecode write.
struct BcWriterForLoop {
    /// Iterator variable.
    iter: BcSlotIn,
    /// Variable to store the next value in.
    var: BcSlotOut,
    /// Address of the first instruction in the loop body.
    inner_addr: BcAddr,
    /// Addresses to patch with the address of the instruction after the loop.
    end_addrs_to_patch: Vec<PatchAddr>,
}

/// Write bytecode here.
pub(crate) struct BcWriter<'f> {
    /// Serialized instructions.
    instrs: BcInstrsWriter,
    /// Instruction spans, used for errors.
    slow_args: Vec<(BcAddr, BcInstrSlowArg)>,
    /// For each statement, will store the span and the BcAddr for the first instruction.
    stmt_locs: BcStatementLocations,
    /// Current stack size.
    stack_size: u32,
    /// Local slot count.
    local_names: FrozenRef<'f, [FrozenStringValue]>,
    /// Local variables which are known to be definitely assigned at current program point.
    definitely_assigned: BcDefinitelyAssigned,
    /// Max observed stack size.
    max_stack_size: u32,
    /// Current loop depth.
    for_loops: Vec<BcWriterForLoop>,
    /// Max observed loop depth.
    max_loop_depth: LoopDepth,

    /// Allocate various objects here.
    pub(crate) heap: &'f FrozenHeap,
}

impl<'f> BcWriter<'f> {
    /// Empty.
    pub(crate) fn new(
        local_names: FrozenRef<'f, [FrozenStringValue]>,
        param_count: u32,
        heap: &'f FrozenHeap,
    ) -> BcWriter<'f> {
        assert!(param_count as usize <= local_names.len());
        let mut definitely_assigned =
            BcDefinitelyAssigned::new(local_names.len().try_into().unwrap());
        for i in 0..param_count {
            definitely_assigned.mark_definitely_assigned(LocalSlotId(i));
        }
        BcWriter {
            instrs: BcInstrsWriter::new(),
            slow_args: Vec::new(),
            stmt_locs: BcStatementLocations::new(),
            stack_size: 0,
            local_names,
            definitely_assigned,
            max_stack_size: 0,
            heap,
            for_loops: Vec::new(),
            max_loop_depth: LoopDepth(0),
        }
    }

    /// Finish writing the bytecode.
    #[allow(let_underscore_drop)]
    pub(crate) fn finish(self) -> Bc {
        let BcWriter {
            instrs,
            slow_args: spans,
            stmt_locs,
            stack_size,
            local_names,
            definitely_assigned,
            max_stack_size,
            heap,
            for_loops,
            max_loop_depth,
        } = self;
        let _ = heap;
        let _ = definitely_assigned;
        assert_eq!(stack_size, 0);
        assert!(for_loops.is_empty());
        // Drop lifetime.
        let local_names = unsafe {
            transmute!(
                FrozenRef<[FrozenStringValue]>,
                FrozenRef<[FrozenStringValue]>,
                local_names
            )
        };
        Bc {
            instrs: instrs.finish(spans, stmt_locs, local_names),
            local_count: local_names.len().try_into().unwrap(),
            max_stack_size,
            max_loop_depth,
        }
    }

    fn local_count(&self) -> u32 {
        self.local_names.len().try_into().unwrap()
    }

    /// Current offset.
    fn ip(&self) -> BcAddr {
        self.instrs.ip()
    }

    /// Version of instruction write with explicit slow arg arg.
    fn do_write_generic_explicit<I: BcInstr>(
        &mut self,
        slow_arg: BcInstrSlowArg,
        arg: I::Arg,
    ) -> (BcAddr, *const I::Arg) {
        self.slow_args.push((self.ip(), slow_arg));
        self.instrs.write::<I>(arg)
    }

    pub(crate) fn mark_before_stmt(&mut self, span: FrameSpan) {
        self.stmt_locs.push(self.ip(), BcStmtLoc { span })
    }

    /// Write an instruction, return address and argument.
    fn write_instr_ret_arg_explicit<I: BcInstr>(
        &mut self,
        slow_arg: BcInstrSlowArg,
        arg: I::Arg,
    ) -> (BcAddr, *const I::Arg) {
        self.do_write_generic_explicit::<I>(slow_arg, arg)
    }

    fn write_instr_ret_arg<I: BcInstr>(
        &mut self,
        span: FrameSpan,
        arg: I::Arg,
    ) -> (BcAddr, *const I::Arg) {
        self.write_instr_ret_arg_explicit::<I>(
            BcInstrSlowArg {
                span,
                ..Default::default()
            },
            arg,
        )
    }

    pub(crate) fn write_instr_explicit<I: BcInstr>(
        &mut self,
        slow_arg: BcInstrSlowArg,
        arg: I::Arg,
    ) {
        self.write_instr_ret_arg_explicit::<I>(slow_arg, arg);
    }

    /// Write an instruction.
    pub(crate) fn write_instr<I: BcInstr>(&mut self, span: FrameSpan, arg: I::Arg) {
        self.write_instr_explicit::<I>(
            BcInstrSlowArg {
                span,
                ..Default::default()
            },
            arg,
        );
    }

    /// Write load constant instruction.
    pub(crate) fn write_const(&mut self, span: FrameSpan, value: FrozenValue, slot: BcSlotOut) {
        assert!(slot.get().0 < self.local_count() + self.stack_size);

        self.write_instr::<InstrConst>(span, (value, slot));
    }

    /// Write load local instruction.
    pub(crate) fn write_load_local(
        &mut self,
        span: FrameSpan,
        slot: LocalSlotId,
        target: BcSlotOut,
    ) {
        assert!(slot.0 < self.local_count());

        if let Some(slot) = self.try_definitely_assigned(slot) {
            self.write_mov(span, slot, target);
        } else {
            self.write_instr::<InstrLoadLocal>(span, (slot, target));
        }
    }

    pub(crate) fn write_load_local_captured(
        &mut self,
        span: FrameSpan,
        source: LocalCapturedSlotId,
        target: BcSlotOut,
    ) {
        assert!(source.0 < self.local_count());
        assert!(target.get().0 < self.local_count() + self.stack_size);
        self.write_instr_ret_arg::<InstrLoadLocalCaptured>(span, (source, target));
    }

    pub(crate) fn write_mov(&mut self, span: FrameSpan, source: BcSlotIn, target: BcSlotOut) {
        assert!(source.get().0 < self.local_count() + self.stack_size);
        assert!(target.get().0 < self.local_count() + self.stack_size);

        // Do not emit no-op `Mov`.
        // It can occur when compiling code like `x = x`.
        // Currently we do not erase these no-op assignments at IR.
        if source.get() == target.get() {
            return;
        }

        self.write_instr_ret_arg::<InstrMov>(span, (source, target));
    }

    pub(crate) fn write_store_local_captured(
        &mut self,
        span: FrameSpan,
        source: BcSlotIn,
        target: LocalCapturedSlotId,
    ) {
        assert!(source.get().0 < self.local_count() + self.stack_size);
        assert!(target.0 < self.local_count());
        self.write_instr_ret_arg::<InstrStoreLocalCaptured>(span, (source, target));
    }

    /// Patch previously writted address with current IP.
    pub(crate) fn patch_addr(&mut self, addr: PatchAddr) {
        self.instrs.patch_addr(addr);
    }

    pub(crate) fn patch_addrs(&mut self, addrs: Vec<PatchAddr>) {
        for adds in addrs {
            self.patch_addr(adds);
        }
    }

    /// Write branch.
    pub(crate) fn write_br(&mut self, span: FrameSpan) -> PatchAddr {
        let (addr, arg) = self.write_instr_ret_arg::<InstrBr>(span, BcAddrOffset::FORWARD);
        self.instrs.addr_to_patch(addr, arg)
    }

    /// Write conditional branch.
    pub(crate) fn write_if_not_br(&mut self, cond: BcSlotIn, span: FrameSpan) -> PatchAddr {
        let (addr, arg) =
            self.write_instr_ret_arg::<InstrIfNotBr>(span, (cond, BcAddrOffset::FORWARD));
        self.instrs.addr_to_patch(addr, unsafe { &(*arg).1 })
    }

    /// Write conditional branch.
    pub(crate) fn write_if_br(&mut self, cond: BcSlotIn, span: FrameSpan) -> PatchAddr {
        let (addr, arg) =
            self.write_instr_ret_arg::<InstrIfBr>(span, (cond, BcAddrOffset::FORWARD));
        self.instrs.addr_to_patch(addr, unsafe { &(*arg).1 })
    }

    fn write_if_else_impl(
        &mut self,
        cond: BcSlotIn,
        span: FrameSpan,
        then_block: impl FnOnce(&mut Self),
        else_block: impl FnOnce(&mut Self),
    ) {
        let definitely_assigned = self.save_definitely_assigned();

        let else_target = self.write_if_not_br(cond, span);
        then_block(self);
        let end_target = self.write_br(span);

        self.restore_definitely_assigned(definitely_assigned.clone());

        self.patch_addr(else_target);
        else_block(self);
        self.patch_addr(end_target);

        self.restore_definitely_assigned(definitely_assigned);
    }

    /// Write if-else block.
    pub(crate) fn write_if_else(
        &mut self,
        cond: BcSlotIn,
        maybe_not: MaybeNot,
        span: FrameSpan,
        then_block: impl FnOnce(&mut Self),
        else_block: impl FnOnce(&mut Self),
    ) {
        match maybe_not {
            MaybeNot::Id => self.write_if_else_impl(cond, span, then_block, else_block),
            MaybeNot::Not => self.write_if_else_impl(cond, span, else_block, then_block),
        }
    }

    pub(crate) fn write_continue(&mut self, span: FrameSpan) {
        let loop_depth = LoopDepth(self.for_loops.len().checked_sub(1).unwrap() as u32);
        let for_loop = self.for_loops.last().unwrap();
        let jump_back = self.ip().offset_from(for_loop.inner_addr).neg();
        let var = for_loop.var;
        let (addr, arg) = self.write_instr_ret_arg::<InstrContinue>(
            span,
            (
                for_loop.iter,
                loop_depth,
                var,
                jump_back,
                BcAddrOffset::FORWARD,
            ),
        );
        let end_patch = self.instrs.addr_to_patch(addr, unsafe { &(*arg).4 });
        let for_loop = self.for_loops.last_mut().unwrap();
        for_loop.end_addrs_to_patch.push(end_patch);
    }

    pub(crate) fn write_break(&mut self, span: FrameSpan) {
        let for_loop = self.for_loops.last().unwrap();
        let (addr, arg) =
            self.write_instr_ret_arg::<InstrBreak>(span, (for_loop.iter, BcAddrOffset::FORWARD));
        let end_patch = self.instrs.addr_to_patch(addr, unsafe { &(*arg).1 });
        let for_loop = self.for_loops.last_mut().unwrap();
        for_loop.end_addrs_to_patch.push(end_patch);
    }

    /// Write for loop.
    pub(crate) fn write_for(
        &mut self,
        over: BcSlotIn,
        var: BcSlotOut,
        span: FrameSpan,
        body: impl FnOnce(&mut BcWriter),
    ) {
        // Allocate a slot to store the iterator.
        self.alloc_slot(|iter, bc| {
            // Definitely assigned save/restore is redundant here, it is performed more precisely
            // by the caller. But it is safer to do it here anyway.
            let definitely_assigned = bc.save_definitely_assigned();

            let loop_depth = LoopDepth(bc.for_loops.len() as u32);
            let (addr, arg) = bc.write_instr_ret_arg::<InstrIter>(
                span,
                (over, loop_depth, iter.to_out(), var, BcAddrOffset::FORWARD),
            );
            let end_patch = bc.instrs.addr_to_patch(addr, unsafe { &(*arg).4 });
            bc.for_loops.push(BcWriterForLoop {
                inner_addr: bc.ip(),
                end_addrs_to_patch: vec![end_patch],
                var,
                iter: iter.to_in(),
            });
            bc.max_loop_depth = cmp::max(bc.max_loop_depth, LoopDepth(bc.for_loops.len() as u32));
            body(bc);
            bc.write_continue(span);
            let for_loop = bc.for_loops.pop().unwrap();
            for addr_to_patch in for_loop.end_addrs_to_patch {
                bc.patch_addr(addr_to_patch);
            }

            bc.restore_definitely_assigned(definitely_assigned);
        })
    }

    /// Write instructions to stop all current iterations.
    /// This is done before `return`.
    pub(crate) fn write_iter_stop(&mut self, span: FrameSpan) {
        // We can stop iteration in any order, but let's for consistency stop them in reverse order.
        for depth in (0..self.for_loops.len()).rev() {
            let iter = self.for_loops[depth].iter;
            self.write_instr::<InstrIterStop>(span, iter);
        }
    }

    fn stack_add(&mut self, add: u32) {
        self.stack_size += add;
        self.max_stack_size = cmp::max(self.max_stack_size, self.stack_size);
    }

    fn stack_sub(&mut self, sub: u32) {
        assert!(self.stack_size >= sub);
        self.stack_size -= sub;
    }

    /// Convert local variable to BC slot if it is known to be definitely assigned
    /// at this execution point.
    pub(crate) fn try_definitely_assigned(&self, local: LocalSlotId) -> Option<BcSlotIn> {
        assert!(local.0 < self.local_count());
        if self.definitely_assigned.is_definitely_assigned(local) {
            Some(local.to_bc_slot().to_in())
        } else {
            None
        }
    }

    pub(crate) fn mark_definitely_assigned(&mut self, local: LocalSlotId) {
        self.definitely_assigned.mark_definitely_assigned(local);
    }

    pub(crate) fn save_definitely_assigned(&self) -> BcDefinitelyAssigned {
        self.definitely_assigned.clone()
    }

    pub(crate) fn restore_definitely_assigned(&mut self, saved: BcDefinitelyAssigned) {
        saved.assert_smaller_then(&self.definitely_assigned);
        self.definitely_assigned = saved;
    }

    /// Allocate a temporary slot, and call a callback.
    ///
    /// The slot is valid during the callback run, and can be reused later.
    pub(crate) fn alloc_slot<R>(&mut self, k: impl FnOnce(BcSlot, &mut BcWriter) -> R) -> R {
        let slot = BcSlot(self.local_count() + self.stack_size);
        self.stack_add(1);
        let r = k(slot, self);
        self.stack_sub(1);
        r
    }

    /// Allocate several slots for the duration of callback run.
    pub(crate) fn alloc_slots<R>(
        &mut self,
        count: u32,
        k: impl FnOnce(BcSlotRange, &mut BcWriter) -> R,
    ) -> R {
        let slots = BcSlotRange {
            start: BcSlot(self.local_count() + self.stack_size),
            end: BcSlot(self.local_count() + self.stack_size + count),
        };
        self.stack_add(count);
        let r = k(slots, self);
        self.stack_sub(count);
        r
    }

    /// Allocate several slots.
    pub(crate) fn alloc_slots_c<const N: usize, R>(
        &mut self,
        k: impl FnOnce(BcSlotsN<N>, &mut BcWriter) -> R,
    ) -> R {
        self.alloc_slots(N as u32, |slots, bc| k(BcSlotsN::from_range(slots), bc))
    }

    /// Allocate several slots for typical compilation of several expressions.
    pub(crate) fn alloc_slots_for_exprs<K, R>(
        &mut self,
        // Iterate over the elements.
        exprs: impl IntoIterator<Item = K>,
        // Invoke a callback which fills the slots.
        mut expr: impl FnMut(BcSlot, K, &mut BcWriter),
        // And then invoke a callback which consumes all the slots again together.
        k: impl FnOnce(BcSlotInRange, &mut BcWriter) -> R,
    ) -> R {
        let start = BcSlot(self.local_count() + self.stack_size);
        let mut end = start;
        for item in exprs {
            self.stack_add(1);
            // `expr` callback may allocate more temporary slots,
            // but they are released after the callback returns.
            // So resulting slots are sequential.
            expr(end, item, self);
            end = BcSlot(end.0 + 1);
        }
        let range = if end == start {
            // This is not really necessary, empty range is equally valid
            // with any starting point, but this makes bytecode output
            // (in particular, in golden tests) more readable.
            BcSlotInRange::default()
        } else {
            BcSlotRange { start, end }.to_in()
        };
        let r = k(range, self);
        self.stack_sub(end.0 - start.0);
        r
    }

    pub(crate) fn alloc_file_span(&self, span: FrameSpan) -> FrozenRef<'static, FrameSpan> {
        self.heap.alloc_any(span)
    }
}
