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

//! Instruction implementations.

use std::cmp::Ordering;
use std::marker;
use std::ptr;

use starlark_syntax::eval_exception::EvalException;

use crate::coerce::coerce;
use crate::collections::symbol::symbol::Symbol;
use crate::collections::Hashed;
use crate::collections::SmallMap;
use crate::const_frozen_string;
use crate::environment::slots::ModuleSlotId;
use crate::eval::bc::addr::BcAddrOffset;
use crate::eval::bc::addr::BcAddrOffsetNeg;
use crate::eval::bc::addr::BcPtrAddr;
use crate::eval::bc::bytecode::Bc;
use crate::eval::bc::call::BcCallArgs;
use crate::eval::bc::call::BcCallArgsForDef;
use crate::eval::bc::call::BcCallArgsFull;
use crate::eval::bc::call::BcCallArgsPos;
use crate::eval::bc::for_loop::LoopDepth;
use crate::eval::bc::frame::BcFramePtr;
use crate::eval::bc::instr::BcInstr;
use crate::eval::bc::instr::InstrControl;
use crate::eval::bc::instr_arg::BcInstrArg;
use crate::eval::bc::native_function::BcNativeFunction;
use crate::eval::bc::slow_arg::BcInstrEndArg;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::stack_ptr::BcSlotInRange;
use crate::eval::bc::stack_ptr::BcSlotInRangeFrom;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::compiler::add_span_to_expr_error;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;
use crate::eval::compiler::def::ParameterCompiled;
use crate::eval::compiler::def::ParametersCompiled;
use crate::eval::compiler::expr::get_attr_hashed_bind;
use crate::eval::compiler::expr::get_attr_hashed_raw;
use crate::eval::compiler::expr::EvalError;
use crate::eval::compiler::expr_throw_starlark_result;
use crate::eval::compiler::stmt::add_assign;
use crate::eval::compiler::stmt::bit_or_assign;
use crate::eval::compiler::stmt::possible_gc;
use crate::eval::compiler::stmt::AssignError;
use crate::eval::runtime::arguments::ResolvedArgName;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::profile::instant::ProfilerInstant;
use crate::eval::runtime::slots::LocalCapturedSlotId;
use crate::eval::runtime::slots::LocalSlotId;
use crate::eval::Arguments;
use crate::eval::DefInfo;
use crate::eval::Evaluator;
use crate::eval::ParametersSpec;
use crate::values::dict::Dict;
use crate::values::int::pointer_i32::PointerI32;
use crate::values::layout::value_not_special::FrozenValueNotSpecial;
use crate::values::string::dot_format::format_one;
use crate::values::string::interpolation::percent_s_one;
use crate::values::types::known_methods::KnownMethod;
use crate::values::types::list::value::ListData;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::StringValueLike;
use crate::values::Value;

/// Instructions which either fail or proceed to the following instruction,
/// and it returns error with span.
pub(crate) trait InstrNoFlowImpl: 'static {
    type Arg: BcInstrArg;

    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr,
        arg: &Self::Arg,
    ) -> crate::Result<()>;
}

pub(crate) struct InstrNoFlow<I: InstrNoFlowImpl>(marker::PhantomData<I>);

impl<I: InstrNoFlowImpl> BcInstr for InstrNoFlow<I> {
    type Arg = I::Arg;

    #[inline(always)]
    fn run<'v, 'b>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        arg: &Self::Arg,
    ) -> InstrControl<'v, 'b> {
        match I::run_with_args(eval, frame, ip, arg) {
            Ok(()) => InstrControl::Next(ip.add_instr::<Self>()),
            Err(e) => InstrControl::Err(e),
        }
    }
}

pub(crate) struct InstrConstImpl;
pub(crate) type InstrConst = InstrNoFlow<InstrConstImpl>;

impl InstrNoFlowImpl for InstrConstImpl {
    type Arg = (FrozenValue, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (constant, target): &(FrozenValue, BcSlotOut),
    ) -> crate::Result<()> {
        frame.set_bc_slot(*target, constant.to_value());
        Ok(())
    }
}

pub(crate) struct InstrLoadLocalImpl;
pub(crate) struct InstrLoadLocalCapturedImpl;
pub(crate) struct InstrLoadModuleImpl;
pub(crate) struct InstrMovImpl;
pub(crate) struct InstrStoreLocalCapturedImpl;
pub(crate) struct InstrStoreModuleImpl;
pub(crate) struct InstrStoreModuleAndExportImpl;
pub(crate) struct InstrUnpackImpl;
pub(crate) struct InstrArrayIndexImpl;
pub(crate) struct InstrSetArrayIndexImpl;
pub(crate) struct InstrArrayIndexSetImpl;
pub(crate) struct InstrObjectFieldImpl;
pub(crate) struct InstrSetObjectFieldImpl;
pub(crate) struct InstrSliceImpl;
pub(crate) struct InstrArrayIndex2Impl;

pub(crate) type InstrLoadLocal = InstrNoFlow<InstrLoadLocalImpl>;
pub(crate) type InstrLoadLocalCaptured = InstrNoFlow<InstrLoadLocalCapturedImpl>;
pub(crate) type InstrLoadModule = InstrNoFlow<InstrLoadModuleImpl>;
pub(crate) type InstrMov = InstrNoFlow<InstrMovImpl>;
pub(crate) type InstrStoreLocalCaptured = InstrNoFlow<InstrStoreLocalCapturedImpl>;
pub(crate) type InstrStoreModule = InstrNoFlow<InstrStoreModuleImpl>;
pub(crate) type InstrStoreModuleAndExport = InstrNoFlow<InstrStoreModuleAndExportImpl>;
pub(crate) type InstrUnpack = InstrNoFlow<InstrUnpackImpl>;
pub(crate) type InstrArrayIndex = InstrNoFlow<InstrArrayIndexImpl>;
pub(crate) type InstrSetArrayIndex = InstrNoFlow<InstrSetArrayIndexImpl>;
pub(crate) type InstrArrayIndexSet = InstrNoFlow<InstrArrayIndexSetImpl>;
pub(crate) type InstrObjectField = InstrNoFlow<InstrObjectFieldImpl>;
pub(crate) type InstrSetObjectField = InstrNoFlow<InstrSetObjectFieldImpl>;
pub(crate) type InstrSlice = InstrNoFlow<InstrSliceImpl>;
pub(crate) type InstrArrayIndex2 = InstrNoFlow<InstrArrayIndex2Impl>;

impl InstrNoFlowImpl for InstrLoadLocalImpl {
    type Arg = (LocalSlotId, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, target): &(LocalSlotId, BcSlotOut),
    ) -> crate::Result<()> {
        let value = eval.get_slot_local(frame, *source)?;
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrLoadLocalCapturedImpl {
    type Arg = (LocalCapturedSlotId, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, target): &(LocalCapturedSlotId, BcSlotOut),
    ) -> crate::Result<()> {
        let value = eval.get_slot_local_captured(*source)?;
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrLoadModuleImpl {
    type Arg = (ModuleSlotId, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, target): &(ModuleSlotId, BcSlotOut),
    ) -> crate::Result<()> {
        let value = eval.get_slot_module(*source)?;
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrMovImpl {
    type Arg = (BcSlotIn, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, target): &(BcSlotIn, BcSlotOut),
    ) -> crate::Result<()> {
        let v = frame.get_bc_slot(*source);
        frame.set_bc_slot(*target, v);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrStoreLocalCapturedImpl {
    type Arg = (BcSlotIn, LocalCapturedSlotId);

    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, target): &(BcSlotIn, LocalCapturedSlotId),
    ) -> crate::Result<()> {
        let v = frame.get_bc_slot(*source);
        eval.set_slot_local_captured(*target, v);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrStoreModuleAndExportImpl {
    type Arg = (BcSlotIn, ModuleSlotId, String);

    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, slot, name): &(BcSlotIn, ModuleSlotId, String),
    ) -> crate::Result<()> {
        let v = frame.get_bc_slot(*source);
        v.export_as(name.as_str(), eval)?;
        eval.set_slot_module(*slot, v);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrStoreModuleImpl {
    type Arg = (BcSlotIn, ModuleSlotId);

    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, target): &(BcSlotIn, ModuleSlotId),
    ) -> crate::Result<()> {
        let v = frame.get_bc_slot(*source);
        eval.set_slot_module(*target, v);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrUnpackImpl {
    type Arg = (BcSlotIn, FrozenRef<'static, [BcSlotOut]>);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, target): &(BcSlotIn, FrozenRef<'static, [BcSlotOut]>),
    ) -> crate::Result<()> {
        let v = frame.get_bc_slot(*source);
        let nvl = v.length()?;
        if nvl != target.len() as i32 {
            return Err(crate::Error::new_other(
                AssignError::IncorrectNumberOfValueToUnpack(target.len() as i32, nvl),
            ));
        }
        let mut i = 0;
        for item in v.iterate(eval.heap())? {
            // Use unconditional assertion here because we cannot trust
            // user defined `length` and `with_iterator` consistently.
            assert!(i < target.len());
            frame.set_bc_slot(target[i], item);
            i += 1;
        }
        assert!(i == target.len());
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrArrayIndexImpl {
    type Arg = (BcSlotIn, BcSlotIn, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (array, index, target): &(BcSlotIn, BcSlotIn, BcSlotOut),
    ) -> crate::Result<()> {
        let array = frame.get_bc_slot(*array);
        let index = frame.get_bc_slot(*index);
        let value = array.at(index, eval.heap())?;
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrSetArrayIndexImpl {
    type Arg = (BcSlotIn, BcSlotIn, BcSlotIn);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, array, index): &(BcSlotIn, BcSlotIn, BcSlotIn),
    ) -> crate::Result<()> {
        let value = frame.get_bc_slot(*source);
        let array = frame.get_bc_slot(*array);
        let index = frame.get_bc_slot(*index);
        array.set_at(index, value)
    }
}

impl InstrNoFlowImpl for InstrArrayIndexSetImpl {
    type Arg = (BcSlotIn, BcSlotIn, BcSlotIn);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (array, index, source): &(BcSlotIn, BcSlotIn, BcSlotIn),
    ) -> crate::Result<()> {
        let value = frame.get_bc_slot(*source);
        let array = frame.get_bc_slot(*array);
        let index = frame.get_bc_slot(*index);
        array.set_at(index, value)
    }
}

impl InstrNoFlowImpl for InstrObjectFieldImpl {
    type Arg = (BcSlotIn, Symbol, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (object, field, target): &(BcSlotIn, Symbol, BcSlotOut),
    ) -> crate::Result<()> {
        let object = frame.get_bc_slot(*object);
        let value = get_attr_hashed_bind(object, field, eval.heap())?;
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrSetObjectFieldImpl {
    type Arg = (BcSlotIn, BcSlotIn, Symbol);

    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, object, field): &(BcSlotIn, BcSlotIn, Symbol),
    ) -> crate::Result<()> {
        let v = frame.get_bc_slot(*source);
        let object = frame.get_bc_slot(*object);
        object.set_attr(field.as_str(), v)
    }
}

impl InstrNoFlowImpl for InstrSliceImpl {
    type Arg = (
        BcSlotIn,
        Option<BcSlotIn>,
        Option<BcSlotIn>,
        Option<BcSlotIn>,
        BcSlotOut,
    );

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (list, start, stop, step, target): &(
            BcSlotIn,
            Option<BcSlotIn>,
            Option<BcSlotIn>,
            Option<BcSlotIn>,
            BcSlotOut,
        ),
    ) -> crate::Result<()> {
        let list = frame.get_bc_slot(*list);
        let start = start.map(|s| frame.get_bc_slot(s));
        let stop = stop.map(|s| frame.get_bc_slot(s));
        let step = step.map(|s| frame.get_bc_slot(s));
        let value = list.slice(start, stop, step, eval.heap())?;
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrArrayIndex2Impl {
    type Arg = (BcSlotIn, BcSlotIn, BcSlotIn, BcSlotOut);

    #[cold]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (array, index0, index1, target): &(BcSlotIn, BcSlotIn, BcSlotIn, BcSlotOut),
    ) -> crate::Result<()> {
        let array = frame.get_bc_slot(*array);
        let index0 = frame.get_bc_slot(*index0);
        let index1 = frame.get_bc_slot(*index1);
        let value = array.get_ref().at2(index0, index1, eval.heap())?;
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

pub(crate) struct InstrEqImpl;
pub(crate) struct InstrEqConstImpl;
pub(crate) struct InstrEqPtrImpl;
pub(crate) struct InstrEqStrImpl;
pub(crate) struct InstrEqIntImpl;

pub(crate) type InstrEq = InstrBinOp<InstrEqImpl>;
pub(crate) type InstrEqConst = InstrNoFlow<InstrEqConstImpl>;
pub(crate) type InstrEqPtr = InstrNoFlow<InstrEqPtrImpl>;
pub(crate) type InstrEqStr = InstrNoFlow<InstrEqStrImpl>;
pub(crate) type InstrEqInt = InstrNoFlow<InstrEqIntImpl>;

impl InstrBinOpImpl for InstrEqImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.equals(v1).map(Value::new_bool)
    }
}

impl InstrNoFlowImpl for InstrEqConstImpl {
    type Arg = (BcSlotIn, FrozenValueNotSpecial, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (a, b, target): &(BcSlotIn, FrozenValueNotSpecial, BcSlotOut),
    ) -> crate::Result<()> {
        let a = frame.get_bc_slot(*a);
        let r = b.equals(a)?;
        frame.set_bc_slot(*target, Value::new_bool(r));
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrEqPtrImpl {
    type Arg = (BcSlotIn, FrozenValue, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (a, b, target): &(BcSlotIn, FrozenValue, BcSlotOut),
    ) -> crate::Result<()> {
        let a = frame.get_bc_slot(*a);
        let r = a.ptr_eq(b.to_value());
        frame.set_bc_slot(*target, Value::new_bool(r));
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrEqIntImpl {
    type Arg = (BcSlotIn, FrozenValueTyped<'static, PointerI32>, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (a, b, target): &(BcSlotIn, FrozenValueTyped<'static, PointerI32>, BcSlotOut),
    ) -> crate::Result<()> {
        let a = frame.get_bc_slot(*a);
        let r = if let Some(a) = a.unpack_int_value() {
            a.as_ref() == b.as_ref()
        } else {
            b.equals(a)?
        };
        frame.set_bc_slot(*target, Value::new_bool(r));
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrEqStrImpl {
    type Arg = (BcSlotIn, FrozenStringValue, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (a, b, target): &(BcSlotIn, FrozenStringValue, BcSlotOut),
    ) -> crate::Result<()> {
        let a = frame.get_bc_slot(*a);
        let r = if let Some(a) = StringValue::new(a) {
            a == b.to_string_value()
        } else {
            false
        };
        frame.set_bc_slot(*target, Value::new_bool(r));
        Ok(())
    }
}

pub(crate) struct InstrNotImpl;
pub(crate) struct InstrMinusImpl;
pub(crate) struct InstrPlusImpl;
pub(crate) struct InstrBitNotImpl;

pub(crate) type InstrNot = InstrUnOp<InstrNotImpl>;
pub(crate) type InstrMinus = InstrUnOp<InstrMinusImpl>;
pub(crate) type InstrPlus = InstrUnOp<InstrPlusImpl>;
pub(crate) type InstrBitNot = InstrUnOp<InstrBitNotImpl>;

impl InstrUnOpImpl for InstrNotImpl {
    #[inline(always)]
    fn eval<'v>(v: Value<'v>, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(Value::new_bool(!v.to_bool()))
    }
}

impl InstrUnOpImpl for InstrPlusImpl {
    #[inline(always)]
    fn eval<'v>(v: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v.plus(heap)
    }
}

impl InstrUnOpImpl for InstrMinusImpl {
    #[inline(always)]
    fn eval<'v>(v: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v.minus(heap)
    }
}

impl InstrUnOpImpl for InstrBitNotImpl {
    #[inline(always)]
    fn eval<'v>(v: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v.bit_not(heap)
    }
}

pub(crate) trait InstrBinOpImpl: 'static {
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>>;
}

pub(crate) trait InstrUnOpImpl: 'static {
    fn eval<'v>(v: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>>;
}

pub(crate) struct InstrBinOpWrapper<I: InstrBinOpImpl>(marker::PhantomData<I>);
pub(crate) struct InstrUnOpWrapper<I: InstrUnOpImpl>(marker::PhantomData<I>);
pub(crate) type InstrBinOp<I> = InstrNoFlow<InstrBinOpWrapper<I>>;
pub(crate) type InstrUnOp<I> = InstrNoFlow<InstrUnOpWrapper<I>>;

impl<I: InstrBinOpImpl> InstrNoFlowImpl for InstrBinOpWrapper<I> {
    type Arg = (BcSlotIn, BcSlotIn, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (v0, v1, target): &(BcSlotIn, BcSlotIn, BcSlotOut),
    ) -> crate::Result<()> {
        let v0 = frame.get_bc_slot(*v0);
        let v1 = frame.get_bc_slot(*v1);
        let v = I::eval(v0, v1, eval.heap())?;
        frame.set_bc_slot(*target, v);
        Ok(())
    }
}

impl<I: InstrUnOpImpl> InstrNoFlowImpl for InstrUnOpWrapper<I> {
    type Arg = (BcSlotIn, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (source, target): &(BcSlotIn, BcSlotOut),
    ) -> crate::Result<()> {
        let source = frame.get_bc_slot(*source);
        let value = I::eval(source, eval.heap())?;
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

pub(crate) struct InstrAddImpl;
pub(crate) struct InstrAddAssignImpl;
pub(crate) struct InstrSubImpl;
pub(crate) struct InstrMultiplyImpl;
pub(crate) struct InstrPercentImpl;
pub(crate) struct InstrDivideImpl;
pub(crate) struct InstrFloorDivideImpl;
pub(crate) struct InstrBitAndImpl;
pub(crate) struct InstrBitOrImpl;
pub(crate) struct InstrBitOrAssignImpl;
pub(crate) struct InstrBitXorImpl;
pub(crate) struct InstrLeftShiftImpl;
pub(crate) struct InstrRightShiftImpl;
pub(crate) struct InstrInImpl;

pub(crate) type InstrAdd = InstrBinOp<InstrAddImpl>;
pub(crate) type InstrAddAssign = InstrBinOp<InstrAddAssignImpl>;
pub(crate) type InstrSub = InstrBinOp<InstrSubImpl>;
pub(crate) type InstrMultiply = InstrBinOp<InstrMultiplyImpl>;
pub(crate) type InstrPercent = InstrBinOp<InstrPercentImpl>;
pub(crate) type InstrDivide = InstrBinOp<InstrDivideImpl>;
pub(crate) type InstrFloorDivide = InstrBinOp<InstrFloorDivideImpl>;
pub(crate) type InstrBitAnd = InstrBinOp<InstrBitAndImpl>;
pub(crate) type InstrBitOr = InstrBinOp<InstrBitOrImpl>;
pub(crate) type InstrBitOrAssign = InstrBinOp<InstrBitOrAssignImpl>;
pub(crate) type InstrBitXor = InstrBinOp<InstrBitXorImpl>;
pub(crate) type InstrLeftShift = InstrBinOp<InstrLeftShiftImpl>;
pub(crate) type InstrRightShift = InstrBinOp<InstrRightShiftImpl>;
pub(crate) type InstrIn = InstrBinOp<InstrInImpl>;

impl InstrBinOpImpl for InstrAddImpl {
    #[inline(always)]
    fn eval<'v>(l: Value<'v>, r: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        l.add(r, heap)
    }
}

impl InstrBinOpImpl for InstrAddAssignImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        add_assign(v0, v1, heap)
    }
}

impl InstrBinOpImpl for InstrSubImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.sub(v1, heap)
    }
}

impl InstrBinOpImpl for InstrMultiplyImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.mul(v1, heap)
    }
}

impl InstrBinOpImpl for InstrPercentImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.percent(v1, heap)
    }
}

impl InstrBinOpImpl for InstrFloorDivideImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.floor_div(v1, heap)
    }
}

impl InstrBinOpImpl for InstrDivideImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.div(v1, heap)
    }
}

impl InstrBinOpImpl for InstrBitAndImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.bit_and(v1, heap)
    }
}

impl InstrBinOpImpl for InstrBitOrImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.bit_or(v1, heap)
    }
}

impl InstrBinOpImpl for InstrBitOrAssignImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        bit_or_assign(v0, v1, heap)
    }
}

impl InstrBinOpImpl for InstrBitXorImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.bit_xor(v1, heap)
    }
}

impl InstrBinOpImpl for InstrLeftShiftImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.left_shift(v1, heap)
    }
}

impl InstrBinOpImpl for InstrRightShiftImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        v0.right_shift(v1, heap)
    }
}

impl InstrBinOpImpl for InstrInImpl {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(Value::new_bool(v1.is_in(v0)?))
    }
}

pub(crate) struct InstrPercentSOneImpl;
pub(crate) type InstrPercentSOne = InstrNoFlow<InstrPercentSOneImpl>;
pub(crate) struct InstrFormatOneImpl;
pub(crate) type InstrFormatOne = InstrNoFlow<InstrFormatOneImpl>;

impl InstrNoFlowImpl for InstrPercentSOneImpl {
    type Arg = (FrozenStringValue, BcSlotIn, FrozenStringValue, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (before, arg, after, target): &(FrozenStringValue, BcSlotIn, FrozenStringValue, BcSlotOut),
    ) -> crate::Result<()> {
        let arg = frame.get_bc_slot(*arg);
        let r = percent_s_one(before.as_str(), arg, after.as_str(), eval.heap())?;
        frame.set_bc_slot(*target, r.to_value());
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrFormatOneImpl {
    type Arg = (FrozenStringValue, BcSlotIn, FrozenStringValue, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (before, arg, after, target): &(FrozenStringValue, BcSlotIn, FrozenStringValue, BcSlotOut),
    ) -> crate::Result<()> {
        let arg = frame.get_bc_slot(*arg);
        let r = format_one(before.as_str(), arg, after.as_str(), eval.heap());
        frame.set_bc_slot(*target, r.to_value());
        Ok(())
    }
}

pub(crate) trait InstrCompareImpl: 'static {
    fn eval_compare(ordering: Ordering) -> bool;
}

pub(crate) struct InstrCompare<I: InstrCompareImpl>(marker::PhantomData<I>);

impl<I: InstrCompareImpl> InstrBinOpImpl for InstrCompare<I> {
    #[inline(always)]
    fn eval<'v>(v0: Value<'v>, v1: Value<'v>, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(Value::new_bool(I::eval_compare(v0.compare(v1)?)))
    }
}

pub(crate) struct InstrLessImpl;
pub(crate) struct InstrGreaterImpl;
pub(crate) struct InstrLessOrEqualImpl;
pub(crate) struct InstrGreaterOrEqualImpl;

pub(crate) type InstrLess = InstrBinOp<InstrCompare<InstrLessImpl>>;
pub(crate) type InstrGreater = InstrBinOp<InstrCompare<InstrGreaterImpl>>;
pub(crate) type InstrLessOrEqual = InstrBinOp<InstrCompare<InstrLessOrEqualImpl>>;
pub(crate) type InstrGreaterOrEqual = InstrBinOp<InstrCompare<InstrGreaterOrEqualImpl>>;

impl InstrCompareImpl for InstrLessImpl {
    #[inline(always)]
    fn eval_compare(ordering: Ordering) -> bool {
        ordering == Ordering::Less
    }
}

impl InstrCompareImpl for InstrGreaterImpl {
    #[inline(always)]
    fn eval_compare(ordering: Ordering) -> bool {
        ordering == Ordering::Greater
    }
}

impl InstrCompareImpl for InstrLessOrEqualImpl {
    #[inline(always)]
    fn eval_compare(ordering: Ordering) -> bool {
        ordering != Ordering::Greater
    }
}

impl InstrCompareImpl for InstrGreaterOrEqualImpl {
    #[inline(always)]
    fn eval_compare(ordering: Ordering) -> bool {
        ordering != Ordering::Less
    }
}

pub(crate) struct InstrTypeImpl;
pub(crate) type InstrType = InstrUnOp<InstrTypeImpl>;

impl InstrUnOpImpl for InstrTypeImpl {
    #[inline(always)]
    fn eval<'v>(v: Value<'v>, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(v.get_type_value().to_frozen_value().to_value())
    }
}

pub(crate) struct InstrTypeIsImpl;
pub(crate) type InstrTypeIs = InstrNoFlow<InstrTypeIsImpl>;

impl InstrNoFlowImpl for InstrTypeIsImpl {
    type Arg = (BcSlotIn, FrozenStringValue, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        (arg, t, target): &(BcSlotIn, FrozenStringValue, BcSlotOut),
    ) -> crate::Result<()> {
        let arg = frame.get_bc_slot(*arg);
        let r = arg.get_type_value() == *t;
        frame.set_bc_slot(*target, Value::new_bool(r));
        Ok(())
    }
}

pub(crate) struct InstrIsInstanceImpl;
pub(crate) type InstrIsInstance = InstrNoFlow<InstrIsInstanceImpl>;

impl InstrNoFlowImpl for InstrIsInstanceImpl {
    type Arg = (BcSlotIn, TypeCompiled<FrozenValue>, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        (arg, t, target): &(BcSlotIn, TypeCompiled<FrozenValue>, BcSlotOut),
    ) -> crate::Result<()> {
        let arg = frame.get_bc_slot(*arg);
        let r = t.matches(arg);
        frame.set_bc_slot(*target, Value::new_bool(r));
        Ok(())
    }
}

pub(crate) struct InstrLenImpl;
pub(crate) type InstrLen = InstrUnOp<InstrLenImpl>;

impl InstrUnOpImpl for InstrLenImpl {
    #[inline(always)]
    fn eval<'v>(v: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(heap.alloc(v.length()?))
    }
}

pub(crate) struct InstrTupleNPopImpl;
pub(crate) struct InstrListNPopImpl;
pub(crate) struct InstrListOfConstsImpl;
pub(crate) struct InstrDictOfConstsImpl;
pub(crate) struct InstrDictConstKeysImpl;
pub(crate) struct InstrDictNPopImpl;
pub(crate) struct InstrListNewImpl;
pub(crate) struct InstrDictNewImpl;

pub(crate) type InstrTupleNPop = InstrNoFlow<InstrTupleNPopImpl>;
pub(crate) type InstrListNew = InstrNoFlow<InstrListNewImpl>;
pub(crate) type InstrListNPop = InstrNoFlow<InstrListNPopImpl>;
pub(crate) type InstrListOfConsts = InstrNoFlow<InstrListOfConstsImpl>;
pub(crate) type InstrDictNew = InstrNoFlow<InstrDictNewImpl>;
pub(crate) type InstrDictOfConsts = InstrNoFlow<InstrDictOfConstsImpl>;
pub(crate) type InstrDictConstKeys = InstrNoFlow<InstrDictConstKeysImpl>;
pub(crate) type InstrDictNPop = InstrNoFlow<InstrDictNPopImpl>;

impl InstrNoFlowImpl for InstrTupleNPopImpl {
    type Arg = (BcSlotInRange, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        (values, target): &(BcSlotInRange, BcSlotOut),
    ) -> crate::Result<()> {
        let items = frame.get_bc_slot_range(*values);
        let value = eval.heap().alloc_tuple(items);
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrListNPopImpl {
    type Arg = (BcSlotInRange, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        (values, target): &(BcSlotInRange, BcSlotOut),
    ) -> crate::Result<()> {
        let items = frame.get_bc_slot_range(*values);
        let value = eval.heap().alloc_list(items);
        frame.set_bc_slot(*target, value);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrListOfConstsImpl {
    type Arg = (Box<[FrozenValue]>, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        (values, target): &(Box<[FrozenValue]>, BcSlotOut),
    ) -> crate::Result<()> {
        let list = eval.heap().alloc_list(coerce(&values));
        frame.set_bc_slot(*target, list);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrDictOfConstsImpl {
    type Arg = (SmallMap<FrozenValue, FrozenValue>, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        (values, target): &(SmallMap<FrozenValue, FrozenValue>, BcSlotOut),
    ) -> crate::Result<()> {
        let dict = eval.heap().alloc(Dict::new((*coerce(values)).clone()));
        frame.set_bc_slot(*target, dict);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrDictNPopImpl {
    type Arg = (BcSlotInRange, BcSlotOut);

    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr,
        (npops, target): &(BcSlotInRange, BcSlotOut),
    ) -> crate::Result<()> {
        let items = frame.get_bc_slot_range(*npops);
        debug_assert!(items.len() % 2 == 0);
        let mut dict = SmallMap::with_capacity(items.len() / 2);
        for i in 0..items.len() / 2 {
            let k = items[i * 2];
            let v = items[i * 2 + 1];
            let k = match k.get_hashed() {
                Ok(k) => k,
                Err(e) => {
                    let spans = &Bc::slow_arg_at_ptr(ip).spans;
                    return Err(add_span_to_expr_error(e, spans[i], eval).into_error());
                }
            };
            let prev = dict.insert_hashed(k, v);
            if prev.is_some() {
                let e =
                    crate::Error::new_other(EvalError::DuplicateDictionaryKey(k.key().to_string()));
                let spans = &Bc::slow_arg_at_ptr(ip).spans;
                return Err(add_span_to_expr_error(e, spans[i], eval).into_error());
            }
        }
        let dict = eval.heap().alloc(Dict::new(dict));
        frame.set_bc_slot(*target, dict);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrDictConstKeysImpl {
    type Arg = (Box<[Hashed<FrozenValue>]>, BcSlotInRangeFrom, BcSlotOut);

    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        (keys, values, target): &(Box<[Hashed<FrozenValue>]>, BcSlotInRangeFrom, BcSlotOut),
    ) -> crate::Result<()> {
        let values = frame.get_bc_slot_range(values.to_range(keys.len() as u32));
        let mut dict = SmallMap::with_capacity(keys.len());
        for (k, v) in keys.iter().zip(values) {
            let prev = dict.insert_hashed(*k, *v);
            debug_assert!(prev.is_none());
        }
        let dict = eval.heap().alloc(Dict::new(coerce(dict)));
        frame.set_bc_slot(*target, dict);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrListNewImpl {
    type Arg = BcSlotOut;

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        target: &BcSlotOut,
    ) -> crate::Result<()> {
        let list = eval.heap().alloc_list(&[]);
        frame.set_bc_slot(*target, list);
        Ok(())
    }
}

impl InstrNoFlowImpl for InstrDictNewImpl {
    type Arg = BcSlotOut;

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _: BcPtrAddr,
        target: &BcSlotOut,
    ) -> crate::Result<()> {
        let dict = eval.heap().alloc(Dict::default());
        frame.set_bc_slot(*target, dict);
        Ok(())
    }
}

pub(crate) struct InstrComprListAppend;
pub(crate) struct InstrComprDictInsert;

impl BcInstr for InstrComprListAppend {
    type Arg = (BcSlotIn, BcSlotIn);

    #[inline(always)]
    fn run<'v, 'b>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        (list, item): &(BcSlotIn, BcSlotIn),
    ) -> InstrControl<'v, 'b> {
        let list = frame.get_bc_slot(*list);
        let item = frame.get_bc_slot(*item);
        // SAFETY: in generated bytecode this slot can be only occupied by a mutable list.
        let list = unsafe { ListData::from_value_unchecked_mut(list) };
        list.push(item, eval.heap());
        // TODO(nga): call continue routine here.
        InstrControl::Next(ip.add_instr::<Self>())
    }
}

impl BcInstr for InstrComprDictInsert {
    type Arg = (BcSlotIn, BcSlotIn, BcSlotIn);

    #[inline(always)]
    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr<'b>,
        (dict, key, value): &(BcSlotIn, BcSlotIn, BcSlotIn),
    ) -> InstrControl<'v, 'b> {
        let dict = frame.get_bc_slot(*dict);
        let key = frame.get_bc_slot(*key);
        let value = frame.get_bc_slot(*value);
        let key = match key.get_hashed() {
            Ok(key) => key,
            Err(e) => return InstrControl::Err(e),
        };
        // SAFETY: in generated bytecode this slot can be only occupied by a mutable dict.
        let mut dict = unsafe { Dict::from_value_unchecked_mut(dict) };
        dict.insert_hashed(key, value);
        // TODO(nga): call continue routine here.
        InstrControl::Next(_ip.add_instr::<Self>())
    }
}

pub(crate) struct InstrCheckTypeImpl;
pub(crate) type InstrCheckType = InstrNoFlow<InstrCheckTypeImpl>;

impl InstrNoFlowImpl for InstrCheckTypeImpl {
    type Arg = (BcSlotIn, TypeCompiled<FrozenValue>);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (expr, ty): &(BcSlotIn, TypeCompiled<FrozenValue>),
    ) -> crate::Result<()> {
        let expr = frame.get_bc_slot(*expr);
        let start = if eval.typecheck_profile.enabled {
            Some(ProfilerInstant::now())
        } else {
            None
        };
        let res = ty.check_type(expr, None);
        if let Some(start) = start {
            let name = const_frozen_string!("assignment");
            eval.typecheck_profile.add(name, start.elapsed());
        }
        res.map_err(Into::into)
    }
}

pub(crate) struct InstrBr;
pub(crate) struct InstrIfBr;
pub(crate) struct InstrIfNotBr;

impl BcInstr for InstrBr {
    type Arg = BcAddrOffset;

    #[inline(always)]
    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        _frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        target: &BcAddrOffset,
    ) -> InstrControl<'v, 'b> {
        InstrControl::Next(ip.add_rel(*target))
    }
}

impl BcInstr for InstrIfBr {
    type Arg = (BcSlotIn, BcAddrOffset);

    #[inline(always)]
    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        (cond, target): &(BcSlotIn, BcAddrOffset),
    ) -> InstrControl<'v, 'b> {
        let cond = frame.get_bc_slot(*cond);
        if cond.to_bool() {
            InstrControl::Next(ip.add_rel(*target))
        } else {
            InstrControl::Next(ip.add_instr::<Self>())
        }
    }
}

impl BcInstr for InstrIfNotBr {
    type Arg = (BcSlotIn, BcAddrOffset);

    #[inline(always)]
    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        (cond, target): &(BcSlotIn, BcAddrOffset),
    ) -> InstrControl<'v, 'b> {
        let cond = frame.get_bc_slot(*cond);
        if !cond.to_bool() {
            InstrControl::Next(ip.add_rel(*target))
        } else {
            InstrControl::Next(ip.add_instr::<Self>())
        }
    }
}

/// Setup `for` loop.
pub(crate) struct InstrIter;
/// `continue` statement.
pub(crate) struct InstrContinue;
/// `break` statement.
pub(crate) struct InstrBreak;
/// Stop all the iterations to release mutation locks before `return`.
pub(crate) struct InstrIterStop;

impl BcInstr for InstrIter {
    type Arg = (BcSlotIn, LoopDepth, BcSlotOut, BcSlotOut, BcAddrOffset);

    #[inline(always)]
    fn run<'v, 'b>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        (over, loop_depth, iter_slot, var, end): &(
            BcSlotIn,
            LoopDepth,
            BcSlotOut,
            BcSlotOut,
            BcAddrOffset,
        ),
    ) -> InstrControl<'v, 'b> {
        let over = frame.get_bc_slot(*over);
        let iter = match over.get_ref().iterate(over, eval.heap()) {
            Ok(iter) => iter,
            Err(e) => return InstrControl::Err(e),
        };
        match iter.get_ref().iter_next(0, eval.heap()) {
            Some(next) => {
                frame.set_bc_slot(*iter_slot, iter);
                frame.set_bc_slot(*var, next);
                frame.set_iter_index(*loop_depth, 1);
                InstrControl::Next(ip.add_instr::<Self>())
            }
            None => {
                iter.get_ref().iter_stop();
                InstrControl::Next(ip.add_rel(*end))
            }
        }
    }
}

impl BcInstr for InstrContinue {
    type Arg = (
        BcSlotIn,
        LoopDepth,
        BcSlotOut,
        BcAddrOffsetNeg,
        BcAddrOffset,
    );

    #[inline(always)]
    fn run<'v, 'b>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        (iter, loop_depth, var, begin, end): &(
            BcSlotIn,
            LoopDepth,
            BcSlotOut,
            BcAddrOffsetNeg,
            BcAddrOffset,
        ),
    ) -> InstrControl<'v, 'b> {
        let iter = frame.get_bc_slot(*iter);
        let loop_depth = *loop_depth;
        let i = frame.get_iter_index(loop_depth);
        match iter.get_ref().iter_next(i, eval.heap()) {
            Some(next) => {
                frame.set_iter_index(loop_depth, i + 1);
                frame.set_bc_slot(*var, next);
                InstrControl::Next(ip.add_rel_neg(*begin))
            }
            None => {
                iter.get_ref().iter_stop();
                InstrControl::Next(ip.add_rel(*end))
            }
        }
    }
}

impl BcInstr for InstrBreak {
    type Arg = (BcSlotIn, BcAddrOffset);

    #[inline(always)]
    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        (iter, end): &(BcSlotIn, BcAddrOffset),
    ) -> InstrControl<'v, 'b> {
        let iter = frame.get_bc_slot(*iter);
        iter.get_ref().iter_stop();
        InstrControl::Next(ip.add_rel(*end))
    }
}

impl BcInstr for InstrIterStop {
    type Arg = BcSlotIn;

    #[inline(always)]
    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        ip: BcPtrAddr<'b>,
        iter: &BcSlotIn,
    ) -> InstrControl<'v, 'b> {
        let iter = frame.get_bc_slot(*iter);
        iter.get_ref().iter_stop();
        InstrControl::Next(ip.add_instr::<Self>())
    }
}

pub(crate) struct InstrReturnConst;
pub(crate) struct InstrReturn;
pub(crate) struct InstrReturnCheckType;

impl BcInstr for InstrReturnConst {
    type Arg = FrozenValue;

    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        _frame: BcFramePtr<'v>,
        _ip: BcPtrAddr<'b>,
        value: &FrozenValue,
    ) -> InstrControl<'v, 'b> {
        InstrControl::Return(value.to_value())
    }
}

impl BcInstr for InstrReturn {
    type Arg = BcSlotIn;

    #[inline(always)]
    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr<'b>,
        &slot: &BcSlotIn,
    ) -> InstrControl<'v, 'b> {
        let v = frame.get_bc_slot(slot);
        InstrControl::Return(v)
    }
}

impl BcInstr for InstrReturnCheckType {
    type Arg = BcSlotIn;

    #[inline(always)]
    fn run<'v, 'b>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr<'b>,
        &slot: &BcSlotIn,
    ) -> InstrControl<'v, 'b> {
        let v = frame.get_bc_slot(slot);
        if let Err(e) = eval.check_return_type(v) {
            return InstrControl::Err(e.into());
        }
        InstrControl::Return(v)
    }
}

pub(crate) struct InstrDefImpl;
pub(crate) type InstrDef = InstrNoFlow<InstrDefImpl>;

#[derive(Debug)]
pub(crate) struct InstrDefData {
    pub(crate) function_name: String,
    pub(crate) params: ParametersCompiled<u32>,
    pub(crate) return_type: Option<TypeCompiled<FrozenValue>>,
    pub(crate) info: FrozenRef<'static, DefInfo>,
}

impl InstrNoFlowImpl for InstrDefImpl {
    type Arg = (BcSlotInRange, InstrDefData, BcSlotOut);

    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (pops, def_data, target): &(BcSlotInRange, InstrDefData, BcSlotOut),
    ) -> crate::Result<()> {
        let pop = frame.get_bc_slot_range(*pops);

        let mut parameters = ParametersSpec::with_capacity(
            def_data.function_name.clone(),
            def_data.params.params.len(),
        );
        let mut parameter_types = Vec::new();

        let mut pop_index = 0;

        for (i, x) in def_data.params.params.iter().enumerate() {
            let i = i as u32;

            if i == def_data.params.indices.num_positional_only && !x.is_star_or_star_star() {
                parameters.no_more_positional_only_args();
            }

            if i == def_data.params.indices.num_positional && !x.is_star_or_star_star() {
                parameters.no_more_positional_args();
            }

            if let (name, Some(t)) = x.name_ty() {
                parameter_types.push((LocalSlotId(i), name.name.clone(), t));
            }

            match &x.node {
                ParameterCompiled::Normal(n, _, None) => parameters.required(&n.name),
                ParameterCompiled::Normal(n, ty, Some(v)) => {
                    assert!(*v == pop_index);
                    let value = pop[pop_index as usize];
                    pop_index += 1;

                    if ty.is_some() {
                        // Check the type of the default
                        let (_, _, ty_compiled) = parameter_types.last().unwrap();
                        expr_throw_starlark_result(
                            ty_compiled.check_type(value, Some(&n.name)),
                            x.span,
                            eval,
                        )
                        .map_err(EvalException::into_error)?;
                    }
                    parameters.defaulted(&n.name, value);
                }
                ParameterCompiled::Args(_, _) => parameters.args(),
                ParameterCompiled::KwArgs(_, _) => parameters.kwargs(),
            };
        }
        let return_type = def_data.return_type;
        assert!(pop_index as usize == pop.len());
        let def = eval.heap().alloc(Def::new(
            parameters.finish(),
            parameter_types,
            return_type,
            def_data.info,
            eval,
        )?);
        frame.set_bc_slot(*target, def);
        Ok(())
    }
}

/// A frozen function argument to a call instruction.
pub(crate) trait BcFrozenCallable: BcInstrArg + Copy {
    fn bc_invoke<'v>(
        self,
        location: FrozenRef<'static, FrameSpan>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>>;
}

impl BcFrozenCallable for FrozenValue {
    #[inline(always)]
    fn bc_invoke<'v>(
        self,
        location: FrozenRef<'static, FrameSpan>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        self.to_value().invoke_with_loc(Some(location), args, eval)
    }
}

impl BcFrozenCallable for FrozenValueTyped<'static, FrozenDef> {
    #[inline(always)]
    fn bc_invoke<'v>(
        self,
        location: FrozenRef<'static, FrameSpan>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        eval.with_call_stack(self.to_value(), Some(location), |eval| {
            self.as_ref().invoke(self.to_value(), args, eval)
        })
    }
}

impl BcFrozenCallable for BcNativeFunction {
    #[inline(always)]
    fn bc_invoke<'v>(
        self,
        location: FrozenRef<'static, FrameSpan>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        eval.with_call_stack(self.to_value(), Some(location), |eval| {
            self.invoke(args, eval)
        })
    }
}

pub(crate) struct InstrCallImpl<A: BcCallArgs<Symbol>>(marker::PhantomData<fn(A)>);
pub(crate) struct InstrCallFrozenGenericImpl<F: BcFrozenCallable, A: BcCallArgs<Symbol>>(
    marker::PhantomData<(F, A)>,
);
pub(crate) struct InstrCallFrozenDefImpl<A: BcCallArgsForDef>(marker::PhantomData<A>);
pub(crate) struct InstrCallMethodImpl<A: BcCallArgs<Symbol>>(marker::PhantomData<A>);
pub(crate) struct InstrCallMaybeKnownMethodImpl<A: BcCallArgs<Symbol>>(marker::PhantomData<A>);

pub(crate) type InstrCall = InstrNoFlow<InstrCallImpl<BcCallArgsFull<Symbol>>>;
pub(crate) type InstrCallPos = InstrNoFlow<InstrCallImpl<BcCallArgsPos>>;
pub(crate) type InstrCallFrozenDef =
    InstrNoFlow<InstrCallFrozenDefImpl<BcCallArgsFull<ResolvedArgName>>>;
pub(crate) type InstrCallFrozenDefPos = InstrNoFlow<InstrCallFrozenDefImpl<BcCallArgsPos>>;
pub(crate) type InstrCallFrozenNative =
    InstrNoFlow<InstrCallFrozenGenericImpl<BcNativeFunction, BcCallArgsFull<Symbol>>>;
pub(crate) type InstrCallFrozenNativePos =
    InstrNoFlow<InstrCallFrozenGenericImpl<BcNativeFunction, BcCallArgsPos>>;
pub(crate) type InstrCallFrozen =
    InstrNoFlow<InstrCallFrozenGenericImpl<FrozenValue, BcCallArgsFull<Symbol>>>;
pub(crate) type InstrCallFrozenPos =
    InstrNoFlow<InstrCallFrozenGenericImpl<FrozenValue, BcCallArgsPos>>;
pub(crate) type InstrCallMethod = InstrNoFlow<InstrCallMethodImpl<BcCallArgsFull<Symbol>>>;
pub(crate) type InstrCallMethodPos = InstrNoFlow<InstrCallMethodImpl<BcCallArgsPos>>;
pub(crate) type InstrCallMaybeKnownMethod =
    InstrNoFlow<InstrCallMaybeKnownMethodImpl<BcCallArgsFull<Symbol>>>;
pub(crate) type InstrCallMaybeKnownMethodPos =
    InstrNoFlow<InstrCallMaybeKnownMethodImpl<BcCallArgsPos>>;

impl<A: BcCallArgs<Symbol>> InstrNoFlowImpl for InstrCallImpl<A> {
    type Arg = (BcSlotIn, A, FrozenRef<'static, FrameSpan>, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (this, args, span, target): &(BcSlotIn, A, FrozenRef<'static, FrameSpan>, BcSlotOut),
    ) -> crate::Result<()> {
        let f = frame.get_bc_slot(*this);
        let arguments = Arguments(args.pop_from_stack(frame));
        let r = f.invoke_with_loc(Some(*span), &arguments, eval)?;
        frame.set_bc_slot(*target, r);
        Ok(())
    }
}

impl<F: BcFrozenCallable, A: BcCallArgs<Symbol>> InstrNoFlowImpl
    for InstrCallFrozenGenericImpl<F, A>
{
    type Arg = (F, A, FrozenRef<'static, FrameSpan>, BcSlotOut);

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (fun, args, span, target): &(F, A, FrozenRef<'static, FrameSpan>, BcSlotOut),
    ) -> crate::Result<()> {
        let arguments = Arguments(args.pop_from_stack(frame));
        let r = fun.bc_invoke(*span, &arguments, eval)?;
        frame.set_bc_slot(*target, r);
        Ok(())
    }
}

impl<A: BcCallArgsForDef> InstrNoFlowImpl for InstrCallFrozenDefImpl<A> {
    type Arg = (
        FrozenValueTyped<'static, FrozenDef>,
        A,
        FrozenRef<'static, FrameSpan>,
        BcSlotOut,
    );

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (fun, args, span, target): &(
            FrozenValueTyped<'static, FrozenDef>,
            A,
            FrozenRef<'static, FrameSpan>,
            BcSlotOut,
        ),
    ) -> crate::Result<()> {
        let arguments = args.pop_from_stack(frame);
        let r = eval.with_call_stack(fun.to_value(), Some(*span), |eval| {
            fun.as_ref()
                .invoke_with_args(fun.to_value(), &arguments, eval)
        })?;
        frame.set_bc_slot(*target, r);
        Ok(())
    }
}

/// Common of method invocation instructions.
#[inline(always)]
fn call_method_common<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    frame: BcFramePtr<'v>,
    this: Value<'v>,
    symbol: &Symbol,
    arguments: &Arguments<'v, '_>,
    span: FrozenRef<'static, FrameSpan>,
    target: BcSlotOut,
) -> crate::Result<()> {
    // TODO: wrong span: should be span of `object.method`, not of the whole expression
    let method = get_attr_hashed_raw(this, symbol, eval.heap())?;
    let r = method.invoke(this, span, arguments, eval)?;
    frame.set_bc_slot(target, r);
    Ok(())
}

/// Common of method invocation instructions where a method is likely stdlib method.
#[inline(always)]
fn call_maybe_known_method_common<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    frame: BcFramePtr<'v>,
    this: Value<'v>,
    symbol: &Symbol,
    known_method: &KnownMethod,
    arguments: &Arguments<'v, '_>,
    span: FrozenRef<'static, FrameSpan>,
    target: BcSlotOut,
) -> crate::Result<()> {
    if let Some(methods) = this.vtable().methods() {
        // Instead of method lookup by name, we compare `Methods` pointers.
        // If pointers are equal, getattr would return the same method
        // we already have.
        if ptr::eq(methods, known_method.type_methods) {
            let r = eval.with_call_stack(known_method.to_value(), Some(span), |eval| {
                known_method.invoke_method(this, arguments, eval)
            })?;
            frame.set_bc_slot(target, r);
            return Ok(());
        }
    }

    call_method_common(eval, frame, this, symbol, arguments, span, target)
}

impl<A: BcCallArgs<Symbol>> InstrNoFlowImpl for InstrCallMethodImpl<A> {
    type Arg = (
        BcSlotIn,
        Symbol,
        A,
        FrozenRef<'static, FrameSpan>,
        BcSlotOut,
    );

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (this, symbol, args, span, target): &(
            BcSlotIn,
            Symbol,
            A,
            FrozenRef<'static, FrameSpan>,
            BcSlotOut,
        ),
    ) -> crate::Result<()> {
        let this = frame.get_bc_slot(*this);
        let arguments = Arguments(args.pop_from_stack(frame));
        call_method_common(eval, frame, this, symbol, &arguments, *span, *target)
    }
}

impl<A: BcCallArgs<Symbol>> InstrNoFlowImpl for InstrCallMaybeKnownMethodImpl<A> {
    type Arg = (
        BcSlotIn,
        Symbol,
        KnownMethod,
        A,
        FrozenRef<'static, FrameSpan>,
        BcSlotOut,
    );

    #[inline(always)]
    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (this, symbol, known_method, args, span, target): &(
            BcSlotIn,
            Symbol,
            KnownMethod,
            A,
            FrozenRef<'static, FrameSpan>,
            BcSlotOut,
        ),
    ) -> crate::Result<()> {
        let this = frame.get_bc_slot(*this);
        let arguments = Arguments(args.pop_from_stack(frame));
        call_maybe_known_method_common(
            eval,
            frame,
            this,
            symbol,
            known_method,
            &arguments,
            *span,
            *target,
        )
    }
}

pub(crate) struct InstrPossibleGcImpl;

pub(crate) type InstrPossibleGc = InstrNoFlow<InstrPossibleGcImpl>;

impl InstrNoFlowImpl for InstrPossibleGcImpl {
    type Arg = ();

    fn run_with_args<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        _frame: BcFramePtr<'v>,
        _ip: BcPtrAddr,
        (): &(),
    ) -> crate::Result<()> {
        possible_gc(eval);
        Ok(())
    }
}

/// Pseudo-instruction:
/// * to store bytecode metadata (i.e. spans): when bytecode is evaluated, we only have IP,
///   we don't have a pointer to bytecode object. To obtain spans by IP, we scroll
///   through the instruction until we encounter this pseudo-instruction.
/// * as a safety against memory overruns. Function block must terminate with return instruction,
///   but if return was missed, this instruction is executed and it panics.
pub(crate) struct InstrEnd;

impl BcInstr for InstrEnd {
    type Arg = BcInstrEndArg;

    fn run<'v, 'b>(
        _eval: &mut Evaluator<'v, '_, '_>,
        _frame: BcFramePtr<'v>,
        _ip: BcPtrAddr<'b>,
        _: &Self::Arg,
    ) -> InstrControl<'v, 'b> {
        unreachable!("this instruction is not meant to be executed");
    }
}
