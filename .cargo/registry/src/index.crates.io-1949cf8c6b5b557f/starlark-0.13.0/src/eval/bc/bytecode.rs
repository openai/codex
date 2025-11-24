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

//! Unsorted/core interpreter stuff.

use std::fmt::Write;

use starlark_syntax::eval_exception::EvalException;

use crate::eval::bc::addr::BcPtrAddr;
use crate::eval::bc::for_loop::LoopDepth;
use crate::eval::bc::frame::BcFramePtr;
use crate::eval::bc::instr::BcInstr;
use crate::eval::bc::instr::InstrControl;
use crate::eval::bc::instr_impl::InstrEnd;
use crate::eval::bc::instrs::BcInstrs;
use crate::eval::bc::opcode::BcOpcode;
use crate::eval::bc::opcode::BcOpcodeHandler;
use crate::eval::bc::slow_arg::BcInstrEndArg;
use crate::eval::bc::slow_arg::BcInstrSlowArg;
use crate::eval::compiler::add_span_to_expr_error;
use crate::eval::runtime::evaluator::EvaluationCallbacks;
use crate::eval::Evaluator;
use crate::values::Value;

/// Ready to execute bytecode.
#[derive(Default)]
pub(crate) struct Bc {
    pub(crate) instrs: BcInstrs,
    /// Number of local variable slots.
    pub(crate) local_count: u32,
    /// Max stack size in values (`Value`).
    pub(crate) max_stack_size: u32,
    /// Max depth of loops.
    pub(crate) max_loop_depth: LoopDepth,
}

impl Bc {
    /// Find span for instruction.
    #[cold]
    #[inline(never)]
    pub(crate) fn slow_arg_at_ptr(addr_ptr: BcPtrAddr) -> &BcInstrSlowArg {
        let mut ptr = addr_ptr;
        loop {
            let opcode = ptr.get_opcode();
            if opcode == BcOpcode::End {
                let end_of_bc = ptr.get_instr::<InstrEnd>();
                let BcInstrEndArg {
                    slow_args,
                    end_addr,
                    ..
                } = &end_of_bc.arg;
                let code_start_ptr = ptr.sub(*end_addr);
                let addr = addr_ptr.offset_from(code_start_ptr);
                for (next_addr, next_span) in slow_args {
                    if *next_addr == addr {
                        return next_span;
                    }
                }
                panic!("span not found for addr: {}", addr);
            }
            ptr = ptr.add(opcode.size_of_repr());
        }
    }

    #[cold]
    #[inline(never)]
    pub(crate) fn wrap_error_for_instr_ptr(
        ptr: BcPtrAddr,
        e: crate::Error,
        eval: &Evaluator,
    ) -> EvalException {
        let span = Self::slow_arg_at_ptr(ptr).span;
        add_span_to_expr_error(e, span, eval)
    }

    /// Run the bytecode in the current frame allocated in the evaluator.
    ///
    /// Frame must be allocated properly, otherwise it will likely result in memory corruption.
    #[inline(always)]
    pub(crate) fn run<'v, EC: EvaluationCallbacks>(
        &self,
        eval: &mut Evaluator<'v, '_, '_>,
        ec: &mut EC,
    ) -> Result<Value<'v>, EvalException> {
        debug_assert!(eval.current_frame.is_inititalized());
        debug_assert_eq!(self.max_stack_size, eval.current_frame.max_stack_size());
        run_block(eval, ec, self.instrs.start_ptr())
    }

    pub(crate) fn dump_debug(&self) -> String {
        let mut w = String::new();
        writeln!(w, "Max stack size: {}", self.max_stack_size).unwrap();
        writeln!(w, "Instructions:").unwrap();
        self.instrs
            .dump_debug()
            .lines()
            .for_each(|line| writeln!(w, "  {}", line).unwrap());
        w
    }
}

/// Execute one instruction.
#[cfg_attr(not(debug_assertions), inline(always))]
fn step<'v, 'b, EC: EvaluationCallbacks>(
    eval: &mut Evaluator<'v, '_, '_>,
    ec: &mut EC,
    frame: BcFramePtr<'v>,
    ip: BcPtrAddr<'b>,
) -> InstrControl<'v, 'b> {
    let opcode = ip.get_opcode();
    // println!("{}: {:?}", self.current_ip, opcode);

    struct HandlerImpl<'v, 'a, 'e, 'y, 'b> {
        eval: &'y mut Evaluator<'v, 'a, 'e>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
    }

    impl<'v, 'a, 'e, 'y, 'b> BcOpcodeHandler<InstrControl<'v, 'b>> for HandlerImpl<'v, 'a, 'e, 'y, 'b> {
        #[cfg_attr(not(debug_assertions), inline(always))]
        fn handle<I: BcInstr>(self) -> InstrControl<'v, 'b> {
            let HandlerImpl { eval, frame, ip } = self;
            let repr = ip.get_instr::<I>();
            I::run(eval, frame, ip, &repr.arg)
        }
    }

    match ec.before_instr(eval, ip, opcode) {
        Ok(()) => {}
        Err(e) => return InstrControl::Err(e),
    }
    opcode.dispatch(HandlerImpl { eval, frame, ip })
}

/// Execute the code block, either a module or a function body.
pub(crate) fn run_block<'v, EC: EvaluationCallbacks>(
    eval: &mut Evaluator<'v, '_, '_>,
    ec: &mut EC,
    mut ip: BcPtrAddr,
) -> Result<Value<'v>, EvalException> {
    // Copy frame pointer to local variable to generate more efficient code.
    let frame = eval.current_frame;

    loop {
        // Note most functions called from here must be carefully annotated
        // as `#[inline(always)]` otherwise LLVM considers them too large to inline.
        //
        // We do inline always only in release mode because otherwise
        // generated stack frame is too large which leads to C stack overflow in debug more.
        ip = match step(eval, ec, frame, ip) {
            InstrControl::Next(ip) => ip,
            InstrControl::Return(v) => return Ok(v),
            InstrControl::Err(e) => return Err(Bc::wrap_error_for_instr_ptr(ip, e, eval)),
        }
    }
}
