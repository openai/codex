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

//! Instructions serialized in byte array.

use std::collections::HashSet;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;
use std::iter;
use std::mem;
use std::ptr;
use std::slice;

use either::Either;

use crate::eval::bc::addr::BcAddr;
use crate::eval::bc::addr::BcAddrOffset;
use crate::eval::bc::addr::BcPtrAddr;
use crate::eval::bc::instr::BcInstr;
use crate::eval::bc::instr_impl::InstrEnd;
use crate::eval::bc::instr_impl::InstrIter;
use crate::eval::bc::opcode::BcOpcode;
use crate::eval::bc::opcode::BcOpcodeHandler;
use crate::eval::bc::repr::BcInstrHeader;
use crate::eval::bc::repr::BcInstrRepr;
use crate::eval::bc::repr::BC_INSTR_ALIGN;
use crate::eval::bc::slow_arg::BcInstrEndArg;
use crate::eval::bc::slow_arg::BcInstrSlowArg;
use crate::eval::bc::writer::BcStatementLocations;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;

impl BcOpcode {
    /// Drop instruction at given address.
    unsafe fn drop_in_place(self, ptr: BcPtrAddr) {
        struct HandlerImpl<'b> {
            ptr: BcPtrAddr<'b>,
        }

        impl BcOpcodeHandler<()> for HandlerImpl<'_> {
            fn handle<I: BcInstr>(self) {
                let HandlerImpl { ptr } = self;
                let instr = ptr.get_instr_mut::<I>();
                unsafe {
                    ptr::drop_in_place(instr);
                }
            }
        }

        self.dispatch(HandlerImpl { ptr });
    }
}

/// Invoke drop for instructions in the buffer.
unsafe fn drop_instrs(instrs: &[u64]) {
    let end = BcPtrAddr::for_slice_end(instrs);
    let mut ptr = BcPtrAddr::for_slice_start(instrs);
    while ptr != end {
        assert!(ptr < end);
        let opcode = ptr.get_opcode();
        opcode.drop_in_place(ptr);
        ptr = ptr.add(opcode.size_of_repr());
    }
}

/// Statically allocate a valid instruction buffer micro-optimization.
///
/// Valid bytecode must end with `EndOfBc` instruction, otherwise evaluation overruns
/// the instruction buffer.
///
/// `BcInstrs` type need to have `Default` (it is convenient).
///
/// Allocating a vec in `BcInstrs::default` is non-free.
///
/// Assertion that `BcInstrs::instrs` is not empty is cheap but not free.
///
/// But if `BcInstrs::instrs` is `Either` allocated instructions or a pointer to statically
/// allocated instructions, then both `BcInstrs::default` is free
/// and evaluation start [is free](https://rust.godbolt.org/z/3nEhWGo4Y).
fn empty_instrs() -> &'static [u64] {
    static END_OF_BC: BcInstrRepr<InstrEnd> = BcInstrRepr {
        header: BcInstrHeader::for_opcode(BcOpcode::End),
        arg: BcInstrEndArg {
            end_addr: BcAddr(0),
            slow_args: Vec::new(),
            local_names: FrozenRef::new(&[]),
        },
        _align: [],
    };
    unsafe {
        slice::from_raw_parts(
            &END_OF_BC as *const BcInstrRepr<_> as *const u64,
            mem::size_of_val(&END_OF_BC) / mem::size_of::<u64>(),
        )
    }
}

pub(crate) struct BcInstrs {
    // We use `usize` here to guarantee the buffer is properly aligned
    // to store `BcInstrLayout`.
    instrs: Either<Box<[u64]>, &'static [u64]>,
    pub(crate) stmt_locs: BcStatementLocations,
}

/// Raw instructions writer.
///
/// Higher level wrapper is `BcWriter`.
pub(crate) struct BcInstrsWriter {
    pub(crate) instrs: Vec<u64>,
}

impl Default for BcInstrs {
    fn default() -> Self {
        Self::for_instrs(Either::Right(empty_instrs()), BcStatementLocations::new())
    }
}

impl Drop for BcInstrs {
    fn drop(&mut self) {
        match &self.instrs {
            Either::Left(heap_allocated) => unsafe {
                drop_instrs(heap_allocated);
            },
            Either::Right(_statically_allocated) => {}
        }
    }
}

impl Drop for BcInstrsWriter {
    fn drop(&mut self) {
        unsafe {
            drop_instrs(&self.instrs);
        }
    }
}

pub(crate) struct PatchAddr {
    pub(crate) instr_start: BcAddr,
    pub(crate) arg: BcAddr,
}

impl BcInstrs {
    pub(crate) fn start_ptr(&self) -> BcPtrAddr {
        BcPtrAddr::for_slice_start(&self.instrs)
    }

    pub(crate) fn for_instrs(
        instrs: Either<Box<[u64]>, &'static [u64]>,
        stmt_locs: BcStatementLocations,
    ) -> Self {
        Self { instrs, stmt_locs }
    }

    pub(crate) fn end(&self) -> BcAddr {
        BcAddr(
            self.instrs
                .len()
                .checked_mul(mem::size_of::<u64>())
                .unwrap()
                .try_into()
                .unwrap(),
        )
    }

    pub(crate) fn end_ptr(&self) -> BcPtrAddr {
        self.start_ptr().offset(self.end())
    }

    #[cfg(test)]
    pub(crate) fn opcodes(&self) -> Vec<BcOpcode> {
        let mut opcodes = Vec::new();
        let end = BcPtrAddr::for_slice_end(&self.instrs);
        let mut ptr = BcPtrAddr::for_slice_start(&self.instrs);
        while ptr != end {
            assert!(ptr < end);
            let opcode = ptr.get_opcode();
            opcodes.push(opcode);
            ptr = ptr.add(opcode.size_of_repr());
        }
        opcodes
    }

    fn iter(&self) -> impl Iterator<Item = (BcPtrAddr, BcAddr)> {
        let mut next_ptr = self.start_ptr();
        iter::from_fn(move || {
            assert!(next_ptr <= self.end_ptr());
            if next_ptr == self.end_ptr() {
                None
            } else {
                let ptr = next_ptr;
                let ip = ptr.offset_from(self.start_ptr());
                next_ptr = next_ptr.add(ptr.get_opcode().size_of_repr());
                Some((ptr, ip))
            }
        })
    }

    fn end_arg(&self) -> Option<&BcInstrEndArg> {
        self.iter()
            .find_map(|(ptr, _ip)| ptr.get_instr_checked::<InstrEnd>().map(|i| &i.arg))
    }

    pub(crate) fn fmt_impl(&self, f: &mut dyn Write, newline: bool) -> fmt::Result {
        let end_arg = self.end_arg();

        let mut loop_ends = Vec::new();
        let mut jump_targets = HashSet::new();
        for (ptr, ip) in self.iter() {
            ptr.get_opcode().visit_jump_addr(ptr, ip, &mut |jump_addr| {
                jump_targets.insert(jump_addr);
            });
        }
        for (ptr, ip) in self.iter() {
            if ptr != self.start_ptr() && !newline {
                write!(f, "; ")?;
            }

            if loop_ends.last() == Some(&ip) {
                loop_ends.pop().unwrap();
            }
            let opcode = ptr.get_opcode();
            if !jump_targets.is_empty() {
                if jump_targets.contains(&ip) {
                    write!(f, ">")?;
                } else if newline {
                    write!(f, " ")?;
                }
            }
            if newline {
                for _ in &loop_ends {
                    write!(f, "  ")?;
                }
            }
            write!(f, "{}: {:?}", ip.0, opcode)?;
            if opcode != BcOpcode::End {
                // `End` args are too verbose and not really instruction args.
                opcode.fmt_append_arg(ptr, ip, end_arg, f)?;
            }
            if newline {
                writeln!(f)?;
            }
            if opcode == BcOpcode::Iter {
                let for_loop = ptr.get_instr::<InstrIter>();
                loop_ends.push(ip.offset(for_loop.arg.4));
            }
        }
        Ok(())
    }

    pub(crate) fn dump_debug(&self) -> String {
        let mut w = String::new();
        self.fmt_impl(&mut w, true).unwrap();
        w
    }
}

impl Display for BcInstrs {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt_impl(f, false)
    }
}

impl BcInstrsWriter {
    pub(crate) fn new() -> BcInstrsWriter {
        BcInstrsWriter { instrs: Vec::new() }
    }

    fn instrs_len_bytes(&self) -> usize {
        self.instrs
            .len()
            .checked_mul(mem::size_of::<u64>())
            .unwrap()
    }

    pub(crate) fn ip(&self) -> BcAddr {
        BcAddr(self.instrs_len_bytes().try_into().unwrap())
    }

    pub(crate) fn write<I: BcInstr>(&mut self, arg: I::Arg) -> (BcAddr, *const I::Arg) {
        let repr = BcInstrRepr::<I>::new(arg);
        assert!(mem::size_of_val(&repr) % mem::size_of::<u64>() == 0);

        let ip = self.ip();

        let offset_bytes = self.instrs_len_bytes();
        self.instrs.resize(
            self.instrs.len() + mem::size_of_val(&repr) / mem::size_of::<u64>(),
            0,
        );
        unsafe {
            let ptr =
                (self.instrs.as_mut_ptr() as *mut u8).add(offset_bytes) as *mut BcInstrRepr<I>;
            ptr::write(ptr, repr);
            (ip, &(*ptr).arg)
        }
    }

    pub(crate) fn addr_to_patch(
        &self,
        instr_start: BcAddr,
        addr: *const BcAddrOffset,
    ) -> PatchAddr {
        unsafe { assert_eq!(*addr, BcAddrOffset::FORWARD) };
        let offset_bytes =
            unsafe { (addr as *const u8).offset_from(self.instrs.as_ptr() as *const u8) };
        assert!((offset_bytes as usize) < self.instrs_len_bytes());
        PatchAddr {
            instr_start,
            arg: BcAddr(offset_bytes as u32),
        }
    }

    pub(crate) fn patch_addr(&mut self, addr: PatchAddr) {
        unsafe {
            let mem_addr =
                (self.instrs.as_mut_ptr() as *mut u8).add(addr.arg.0 as usize) as *mut BcAddrOffset;
            assert!(*mem_addr == BcAddrOffset::FORWARD);
            *mem_addr = self.ip().offset_from(addr.instr_start);
            debug_assert!(((*mem_addr).0 as usize) % BC_INSTR_ALIGN == 0);
        }
    }

    pub(crate) fn finish(
        mut self,
        slow_args: Vec<(BcAddr, BcInstrSlowArg)>,
        stmt_locs: BcStatementLocations,
        local_names: FrozenRef<'static, [FrozenStringValue]>,
    ) -> BcInstrs {
        self.write::<InstrEnd>(BcInstrEndArg {
            end_addr: self.ip(),
            slow_args,
            local_names,
        });
        // We cannot destructure `self` to fetch `instrs` because `Self` has `drop,
        // so we `mem::take`.
        let instrs = mem::take(&mut self.instrs);
        let instrs = instrs.into_boxed_slice();
        assert!((instrs.as_ptr() as usize) % BC_INSTR_ALIGN == 0);
        BcInstrs::for_instrs(Either::Left(instrs), stmt_locs)
    }
}

#[cfg(test)]
mod tests {
    use std::mem;

    use crate::const_frozen_string;
    use crate::eval::bc::instr_impl::InstrConst;
    use crate::eval::bc::instr_impl::InstrPossibleGc;
    use crate::eval::bc::instr_impl::InstrReturn;
    use crate::eval::bc::instrs::BcInstrs;
    use crate::eval::bc::instrs::BcInstrsWriter;
    use crate::eval::bc::stack_ptr::BcSlot;
    use crate::eval::bc::writer::BcStatementLocations;
    use crate::values::FrozenHeap;
    use crate::values::FrozenValue;

    #[test]
    fn write() {
        let mut bc = BcInstrsWriter::new();
        bc.write::<InstrPossibleGc>(());
        assert_eq!(1, bc.instrs.len());
        bc.write::<InstrPossibleGc>(());
        assert_eq!(2, bc.instrs.len());
    }

    /// Test `BcInstrs::default()` produces something valid.
    #[test]
    fn default() {
        assert_eq!("0: End", BcInstrs::default().to_string());
    }

    #[test]
    fn display() {
        let heap = FrozenHeap::new();
        let local_names = heap
            .alloc_any(vec![const_frozen_string!("abc")])
            .map(|s| s.as_slice());
        let mut bc = BcInstrsWriter::new();
        bc.write::<InstrConst>((FrozenValue::new_bool(true), BcSlot(0).to_out()));
        bc.write::<InstrReturn>(BcSlot(0).to_in());
        let bc = bc.finish(Vec::new(), BcStatementLocations::new(), local_names);
        if mem::size_of::<usize>() == 8 {
            assert_eq!(
                "0: Const True ->&abc; 24: Return &abc; 32: End",
                bc.to_string()
            );
            assert_eq!(
                "0: Const True ->&abc\n24: Return &abc\n32: End\n",
                bc.dump_debug()
            );
        } else if mem::size_of::<usize>() == 4 {
            // Starlark doesn't work now on 32-bit CPU
        } else {
            panic!("unknown word size: {}", mem::size_of::<usize>());
        }
    }
}
