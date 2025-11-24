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

//! Address types used in bytecode interpreter.

use std::marker;
use std::mem;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::Sub;

use derive_more::Display;
use dupe::Dupe;

use crate::eval::bc::if_debug::IfDebug;
use crate::eval::bc::instr::BcInstr;
use crate::eval::bc::opcode::BcOpcode;
use crate::eval::bc::repr::BcInstrHeader;
use crate::eval::bc::repr::BcInstrRepr;
use crate::eval::bc::repr::BC_INSTR_ALIGN;

/// Address relative to bytecode start.
#[derive(
    Eq, PartialEq, Copy, Clone, Dupe, Debug, PartialOrd, Ord, Display, Hash, Default
)]
#[display("@{}", _0)]
pub(crate) struct BcAddr(pub(crate) u32);

impl BcAddr {
    #[inline]
    pub(crate) fn offset_from(self, start: BcAddr) -> BcAddrOffset {
        debug_assert!(self >= start);
        BcAddrOffset(self.0 - start.0)
    }

    #[inline]
    pub(crate) fn offset(self, offset: BcAddrOffset) -> BcAddr {
        BcAddr(self.0 + offset.0)
    }

    #[inline]
    pub(crate) fn offset_neg(self, offset: BcAddrOffsetNeg) -> BcAddr {
        BcAddr(self.0 - offset.0)
    }
}

impl Sub<u32> for BcAddr {
    type Output = BcAddr;

    fn sub(self, rhs: u32) -> BcAddr {
        BcAddr(self.0.checked_sub(rhs).unwrap())
    }
}

impl Add<u32> for BcAddr {
    type Output = BcAddr;

    fn add(self, rhs: u32) -> BcAddr {
        BcAddr(self.0 + rhs)
    }
}

impl AddAssign<u32> for BcAddr {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += rhs;
    }
}

/// Valid pointer range of bytecode.
/// Used for debugging assertions. This objects is not created in release mode.
#[derive(Copy, Clone, Dupe, Debug, PartialEq)]
pub(crate) struct BcPtrRange {
    start: *const u8,
    /// Length in bytes.
    len: usize,
}

impl BcPtrRange {
    pub(crate) fn for_slice(slice: &[u64]) -> BcPtrRange {
        BcPtrRange {
            start: slice.as_ptr() as *const u8,
            len: mem::size_of_val::<[u64]>(slice),
        }
    }

    pub(crate) fn assert_in_range(&self, ptr: *const u8) {
        let offset = unsafe { ptr.offset_from(self.start) };
        assert!(offset >= 0);
        assert!(offset as usize <= self.len);
    }

    fn end(&self) -> *const u8 {
        unsafe { self.start.add(self.len) }
    }
}

/// Pointer to an instruction in memory.
#[derive(Copy, Clone, Dupe, PartialOrd, PartialEq, Debug)]
// Lifetime here is mostly for documentation, it does not guarantee anything.
pub(crate) struct BcPtrAddr<'b> {
    ptr: *const u8,
    /// When assertions enabled, we validate the pointer is in this range.
    range: IfDebug<BcPtrRange>,
    _marker: marker::PhantomData<&'b u8>,
}

impl<'b> BcPtrAddr<'b> {
    /// Constructor.
    unsafe fn new(ptr: *const u8, range: IfDebug<BcPtrRange>) -> BcPtrAddr<'b> {
        debug_assert!(ptr as usize % BC_INSTR_ALIGN == 0);
        range.if_debug(|range| range.assert_in_range(ptr));
        BcPtrAddr {
            ptr,
            range,
            _marker: marker::PhantomData,
        }
    }

    /// Create a pointer for the beginning of the slice.
    pub(crate) fn for_slice_start(slice: &'b [u64]) -> BcPtrAddr<'b> {
        unsafe {
            BcPtrAddr::new(
                slice.as_ptr() as *const u8,
                IfDebug::new(BcPtrRange::for_slice(slice)),
            )
        }
    }

    /// Create a pointer for the beginning of the slice.
    pub(crate) fn for_slice_end(slice: &'b [u64]) -> BcPtrAddr<'b> {
        unsafe {
            BcPtrAddr::new(
                slice.as_ptr().add(slice.len()) as *const u8,
                IfDebug::new(BcPtrRange::for_slice(slice)),
            )
        }
    }

    /// Distance from current ptr to the end of instructions.
    fn remaining_if_debug(self) -> usize {
        unsafe { self.range.get_ref_if_debug().end().offset_from(self.ptr) as usize }
    }

    pub(crate) fn get_instr<I: BcInstr>(self) -> &'b BcInstrRepr<I> {
        debug_assert!(self.remaining_if_debug() >= mem::size_of::<BcInstrRepr<I>>());
        let ptr = self.ptr as *const BcInstrRepr<I>;
        let repr = unsafe { &*ptr };
        debug_assert_eq!(repr.header.opcode, BcOpcode::for_instr::<I>());
        repr
    }

    pub(crate) fn get_instr_checked<I: BcInstr>(self) -> Option<&'b BcInstrRepr<I>> {
        if self.get_opcode() == BcOpcode::for_instr::<I>() {
            Some(self.get_instr())
        } else {
            None
        }
    }

    pub(crate) fn get_instr_mut<I: BcInstr>(self) -> *mut BcInstrRepr<I> {
        debug_assert!(
            self.remaining_if_debug() >= mem::size_of::<BcInstrRepr<I>>(),
            "remaining: {}, instr size: {}",
            self.remaining_if_debug(),
            mem::size_of::<BcInstrRepr<I>>()
        );
        self.ptr as *mut BcInstrRepr<I>
    }

    pub(crate) fn get_opcode(self) -> BcOpcode {
        debug_assert!(self.remaining_if_debug() >= mem::size_of::<BcInstrHeader>());
        let ptr = self.ptr as *const BcInstrHeader;
        unsafe { (*ptr).opcode }
    }

    pub(crate) fn offset_from(self, start: BcPtrAddr) -> BcAddr {
        debug_assert!(self.range.get_ref_if_debug() == start.range.get_ref_if_debug());
        unsafe {
            let offset = self.ptr.offset_from(start.ptr);
            debug_assert!(offset >= 0);
            debug_assert!(offset <= i32::MAX as isize);
            BcAddr(offset as u32)
        }
    }

    #[inline(always)]
    fn sub_usize(self, offset: usize) -> BcPtrAddr<'b> {
        unsafe { BcPtrAddr::new(self.ptr.sub(offset), self.range) }
    }

    #[inline(always)]
    pub(crate) fn sub(self, start: BcAddr) -> BcPtrAddr<'b> {
        self.sub_usize(start.0 as usize)
    }

    pub(crate) fn offset(self, addr: BcAddr) -> BcPtrAddr<'b> {
        self.add(addr.0 as usize)
    }

    #[inline(always)]
    pub(crate) fn add_rel(self, rel: BcAddrOffset) -> BcPtrAddr<'b> {
        self.add(rel.0 as usize)
    }

    #[inline(always)]
    pub(crate) fn add_rel_neg(self, rel: BcAddrOffsetNeg) -> BcPtrAddr<'b> {
        self.sub_usize(rel.0 as usize)
    }

    pub(crate) fn add(self, offset: usize) -> BcPtrAddr<'b> {
        unsafe { BcPtrAddr::new(self.ptr.add(offset), self.range) }
    }

    pub(crate) fn add_instr<I: BcInstr>(self) -> BcPtrAddr<'b> {
        self.add_rel(BcAddrOffset::for_instr::<I>())
    }
}

/// Difference between addresses.
#[derive(Eq, PartialEq, Copy, Clone, Dupe, Debug, PartialOrd, Ord, Display)]
pub(crate) struct BcAddrOffset(pub(crate) u32);

impl BcAddrOffset {
    /// Pointer to not yet known address.
    pub(crate) const FORWARD: BcAddrOffset = BcAddrOffset(0xdeadbeef);

    /// Size of an instruction.
    fn for_instr<I: BcInstr>() -> BcAddrOffset {
        <BcInstrRepr<I>>::assert_align();
        BcAddrOffset(mem::size_of::<BcInstrRepr<I>>() as u32)
    }

    #[inline(always)]
    pub(crate) fn neg(self) -> BcAddrOffsetNeg {
        BcAddrOffsetNeg(self.0)
    }
}

/// Negative difference between addresses.
#[derive(Eq, PartialEq, Copy, Clone, Dupe, Debug, PartialOrd, Ord, Display)]
pub(crate) struct BcAddrOffsetNeg(pub(crate) u32);
