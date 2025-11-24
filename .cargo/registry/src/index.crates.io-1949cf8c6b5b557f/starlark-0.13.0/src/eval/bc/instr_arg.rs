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

//! Instruction arguments.

use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;

use itertools::Itertools;

use crate::collections::symbol::symbol::Symbol;
use crate::collections::Hashed;
use crate::collections::SmallMap;
use crate::environment::slots::ModuleSlotId;
use crate::eval::bc::addr::BcAddr;
use crate::eval::bc::addr::BcAddrOffset;
use crate::eval::bc::addr::BcAddrOffsetNeg;
use crate::eval::bc::addr::BcPtrAddr;
use crate::eval::bc::call::BcCallArgsFull;
use crate::eval::bc::call::BcCallArgsPos;
use crate::eval::bc::for_loop::LoopDepth;
use crate::eval::bc::instr::BcInstr;
use crate::eval::bc::instr_impl::InstrDefData;
use crate::eval::bc::native_function::BcNativeFunction;
use crate::eval::bc::opcode::BcOpcode;
use crate::eval::bc::opcode::BcOpcodeHandler;
use crate::eval::bc::slow_arg::BcInstrEndArg;
use crate::eval::bc::stack_ptr::BcSlot;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::stack_ptr::BcSlotInRange;
use crate::eval::bc::stack_ptr::BcSlotInRangeFrom;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::runtime::arguments::ArgSymbol;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::slots::LocalCapturedSlotId;
use crate::eval::runtime::slots::LocalSlotId;
use crate::values::layout::value_not_special::FrozenValueNotSpecial;
use crate::values::types::known_methods::KnownMethod;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::FrozenRef;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::StarlarkValue;

/// Truncate value if it is too long.
struct TruncateValueRepr(FrozenValue);

impl Display for TruncateValueRepr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let repr = self.0.to_value().to_repr();
        // Truncate too long constants (like dicts with hundreds of elements).
        if repr.len() > 100 {
            write!(f, "<{}>", self.0.to_value().get_type())
        } else {
            write!(f, "{}", repr)
        }
    }
}

/// Instruction fixed argument.
pub(crate) trait BcInstrArg: 'static {
    /// Append space then append the argument, or append nothing if the argument is empty.
    fn fmt_append(
        param: &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result;
    /// Collect instruction jump addresses.
    fn visit_jump_addr(param: &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr));
}

impl BcInstrArg for () {
    fn fmt_append(
        _param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        _f: &mut dyn Write,
    ) -> fmt::Result {
        Ok(())
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for u32 {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", param)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for i32 {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", param)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl<A: BcInstrArg, B: BcInstrArg> BcInstrArg for (A, B) {
    fn fmt_append(
        (a, b): &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        A::fmt_append(a, ip, end_arg, f)?;
        B::fmt_append(b, ip, end_arg, f)?;
        Ok(())
    }

    fn visit_jump_addr((a, b): &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        BcInstrArg::visit_jump_addr(a, ip, consumer);
        BcInstrArg::visit_jump_addr(b, ip, consumer);
    }
}

impl<A: BcInstrArg, B: BcInstrArg, C: BcInstrArg> BcInstrArg for (A, B, C) {
    fn fmt_append(
        (a, b, c): &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        A::fmt_append(a, ip, end_arg, f)?;
        B::fmt_append(b, ip, end_arg, f)?;
        C::fmt_append(c, ip, end_arg, f)?;
        Ok(())
    }

    fn visit_jump_addr((a, b, c): &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        BcInstrArg::visit_jump_addr(a, ip, consumer);
        BcInstrArg::visit_jump_addr(b, ip, consumer);
        BcInstrArg::visit_jump_addr(c, ip, consumer);
    }
}

#[allow(clippy::many_single_char_names)]
impl<A: BcInstrArg, B: BcInstrArg, C: BcInstrArg, D: BcInstrArg> BcInstrArg for (A, B, C, D) {
    fn fmt_append(
        (a, b, c, d): &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        A::fmt_append(a, ip, end_arg, f)?;
        B::fmt_append(b, ip, end_arg, f)?;
        C::fmt_append(c, ip, end_arg, f)?;
        D::fmt_append(d, ip, end_arg, f)?;
        Ok(())
    }

    fn visit_jump_addr((a, b, c, d): &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        BcInstrArg::visit_jump_addr(a, ip, consumer);
        BcInstrArg::visit_jump_addr(b, ip, consumer);
        BcInstrArg::visit_jump_addr(c, ip, consumer);
        BcInstrArg::visit_jump_addr(d, ip, consumer);
    }
}

#[allow(clippy::many_single_char_names)]
impl<A: BcInstrArg, B: BcInstrArg, C: BcInstrArg, D: BcInstrArg, E: BcInstrArg> BcInstrArg
    for (A, B, C, D, E)
{
    fn fmt_append(
        (a, b, c, d, e): &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        A::fmt_append(a, ip, end_arg, f)?;
        B::fmt_append(b, ip, end_arg, f)?;
        C::fmt_append(c, ip, end_arg, f)?;
        D::fmt_append(d, ip, end_arg, f)?;
        E::fmt_append(e, ip, end_arg, f)?;
        Ok(())
    }

    fn visit_jump_addr((a, b, c, d, e): &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        BcInstrArg::visit_jump_addr(a, ip, consumer);
        BcInstrArg::visit_jump_addr(b, ip, consumer);
        BcInstrArg::visit_jump_addr(c, ip, consumer);
        BcInstrArg::visit_jump_addr(d, ip, consumer);
        BcInstrArg::visit_jump_addr(e, ip, consumer);
    }
}

#[allow(clippy::many_single_char_names)]
impl<A: BcInstrArg, B: BcInstrArg, C: BcInstrArg, D: BcInstrArg, E: BcInstrArg, F: BcInstrArg>
    BcInstrArg for (A, B, C, D, E, F)
{
    fn fmt_append(
        (a, b, c, d, e, f): &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        w: &mut dyn Write,
    ) -> fmt::Result {
        A::fmt_append(a, ip, end_arg, w)?;
        B::fmt_append(b, ip, end_arg, w)?;
        C::fmt_append(c, ip, end_arg, w)?;
        D::fmt_append(d, ip, end_arg, w)?;
        E::fmt_append(e, ip, end_arg, w)?;
        F::fmt_append(f, ip, end_arg, w)?;
        Ok(())
    }

    fn visit_jump_addr((a, b, c, d, e, f): &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        BcInstrArg::visit_jump_addr(a, ip, consumer);
        BcInstrArg::visit_jump_addr(b, ip, consumer);
        BcInstrArg::visit_jump_addr(c, ip, consumer);
        BcInstrArg::visit_jump_addr(d, ip, consumer);
        BcInstrArg::visit_jump_addr(e, ip, consumer);
        BcInstrArg::visit_jump_addr(f, ip, consumer);
    }
}

impl<A: BcInstrArg, const N: usize> BcInstrArg for [A; N] {
    fn fmt_append(
        param: &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        for a in param {
            A::fmt_append(a, ip, end_arg, f)?;
        }
        Ok(())
    }

    fn visit_jump_addr(param: &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        for a in param {
            BcInstrArg::visit_jump_addr(a, ip, consumer);
        }
    }
}

impl BcInstrArg for BcAddrOffset {
    fn fmt_append(
        param: &Self,
        ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", ip.offset(*param).0)
    }

    fn visit_jump_addr(param: &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        consumer(ip.offset(*param));
    }
}

impl BcInstrArg for BcAddrOffsetNeg {
    fn fmt_append(
        param: &Self,
        ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", ip.offset_neg(*param).0)
    }

    fn visit_jump_addr(param: &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        consumer(ip.offset_neg(*param));
    }
}

impl BcInstrArg for FrozenValue {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", TruncateValueRepr(*param))
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for FrozenValueNotSpecial {
    fn fmt_append(
        param: &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        FrozenValue::fmt_append(&param.to_frozen_value(), ip, end_arg, f)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for TypeCompiled<FrozenValue> {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", param)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl<T: BcInstrArg> BcInstrArg for Option<T> {
    fn fmt_append(
        param: &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        match param {
            None => write!(f, " ()"),
            Some(v) => T::fmt_append(v, ip, end_arg, f),
        }
    }

    fn visit_jump_addr(param: &Self, ip: BcAddr, consumer: &mut dyn FnMut(BcAddr)) {
        if let Some(param) = param {
            T::visit_jump_addr(param, ip, consumer);
        }
    }
}

impl BcInstrArg for String {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, "{:?}", param)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl<T: Display> BcInstrArg for FrozenRef<'static, T>
where
    FrozenRef<'static, T>: Copy,
{
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", param.as_ref())
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl<T: Display> BcInstrArg for FrozenRef<'static, [T]>
where
    FrozenRef<'static, T>: Copy,
{
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(
            f,
            " [{}]",
            param.iter().map(|v| format!("{}", v)).join(", ")
        )
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl<T: StarlarkValue<'static>> BcInstrArg for FrozenValueTyped<'static, T> {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", TruncateValueRepr(param.to_frozen_value()))
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for BcNativeFunction {
    fn fmt_append(
        param: &Self,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        BcInstrArg::fmt_append(&param.fun(), ip, end_arg, f)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

struct BcSlotDisplay<'a>(BcSlot, Option<&'a BcInstrEndArg>);

impl<'a> Display for BcSlotDisplay<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let name = self
            .1
            .and_then(|end_arg| end_arg.local_names.get(self.0.0 as usize));
        match name {
            Some(name) => write!(f, "&{}", name.as_str()),
            None => write!(f, "&{}", self.0.0),
        }
    }
}

impl BcInstrArg for LocalSlotId {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", BcSlotDisplay(param.to_bc_slot(), end_arg))
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for LocalCapturedSlotId {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", BcSlotDisplay(param.to_bc_slot(), end_arg))
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for BcSlotIn {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", BcSlotDisplay(param.get(), end_arg))
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for BcSlotOut {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " ->{}", BcSlotDisplay(param.get(), end_arg))
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for BcSlotInRange {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(
            f,
            " [{}]",
            param
                .iter()
                .map(|s| BcSlotDisplay(s.get(), end_arg).to_string())
                .join(", ")
        )
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for BcSlotInRangeFrom {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}..", param.0)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for ModuleSlotId {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " m{}", param.0)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for FrameSpan {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", param)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

/// Opcode as instruction argument.
impl BcInstrArg for BcOpcode {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {:?}", param)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for LoopDepth {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", param)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for KnownMethod {
    fn fmt_append(
        _param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " <m>")
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for Symbol {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", param.as_str())
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for Box<[FrozenValue]> {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " [")?;
        for (i, v) in param.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", TruncateValueRepr(*v))?;
        }
        write!(f, "]")?;
        Ok(())
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for Box<[Hashed<FrozenValue>]> {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " [")?;
        for (i, v) in param.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", TruncateValueRepr(*v.key()))?;
        }
        write!(f, "]")?;
        Ok(())
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for SmallMap<FrozenValue, FrozenValue> {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {{")?;
        for (i, (k, v)) in param.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", TruncateValueRepr(*k), TruncateValueRepr(*v))?;
        }
        write!(f, "}}")?;
        Ok(())
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for InstrDefData {
    fn fmt_append(
        _param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " InstrDefData")
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl<S: ArgSymbol> BcInstrArg for BcCallArgsFull<S> {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {{{}}}", param)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for BcCallArgsPos {
    fn fmt_append(
        param: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " {}", param.pos)
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcInstrArg for BcInstrEndArg {
    fn fmt_append(
        _: &Self,
        _ip: BcAddr,
        _end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        write!(f, " BcInstrEndArg")
    }

    fn visit_jump_addr(_param: &Self, _ip: BcAddr, _consumer: &mut dyn FnMut(BcAddr)) {}
}

impl BcOpcode {
    /// Format instruction argument.
    pub(crate) fn fmt_append_arg(
        self,
        ptr: BcPtrAddr,
        ip: BcAddr,
        end_arg: Option<&BcInstrEndArg>,
        f: &mut dyn Write,
    ) -> fmt::Result {
        struct HandlerImpl<'b, 'g> {
            ptr: BcPtrAddr<'b>,
            ip: BcAddr,
            end_arg: Option<&'b BcInstrEndArg>,
            f: &'g mut dyn Write,
        }

        impl BcOpcodeHandler<fmt::Result> for HandlerImpl<'_, '_> {
            fn handle<I: BcInstr>(self) -> fmt::Result {
                let HandlerImpl {
                    ptr,
                    ip,
                    end_arg,
                    f,
                } = self;
                let instr = ptr.get_instr::<I>();
                I::Arg::fmt_append(&instr.arg, ip, end_arg, f)
            }
        }

        self.dispatch(HandlerImpl {
            ptr,
            ip,
            end_arg,
            f,
        })
    }

    pub(crate) fn visit_jump_addr(
        self,
        ptr: BcPtrAddr,
        addr: BcAddr,
        consumer: &mut dyn FnMut(BcAddr),
    ) {
        struct HandlerImpl<'b, 'c> {
            ptr: BcPtrAddr<'b>,
            addr: BcAddr,
            consumer: &'c mut dyn FnMut(BcAddr),
        }

        impl BcOpcodeHandler<()> for HandlerImpl<'_, '_> {
            fn handle<I: BcInstr>(self) {
                let HandlerImpl {
                    ptr,
                    addr,
                    consumer,
                } = self;
                let instr = ptr.get_instr::<I>();
                I::Arg::visit_jump_addr(&instr.arg, addr, consumer);
            }
        }

        self.dispatch(HandlerImpl {
            ptr,
            addr,
            consumer,
        });
    }
}
