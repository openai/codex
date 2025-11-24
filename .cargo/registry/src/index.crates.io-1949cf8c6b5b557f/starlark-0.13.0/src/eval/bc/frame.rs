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

//! Local variables and stack, in single allocation.

use std::mem;
use std::mem::MaybeUninit;
use std::ptr;
use std::slice;

use dupe::Dupe;

use crate::eval::bc::for_loop::LoopDepth;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::stack_ptr::BcSlotInRange;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::runtime::slots::LocalSlotIdCapturedOrNot;
use crate::eval::Evaluator;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;

/// Current `def` frame (but not native function frame).
///
/// Frame memory layout:
///
/// ```text
/// [ loop_indices | BcFrame | locals | stack ]
///   BcFramePtr points here ^
/// ```
#[repr(C)]
struct BcFrame<'v> {
    /// Number of local slots.
    local_count: u32,
    /// Number of stack slots.
    max_stack_size: u32,
    /// Max number of nested for loops.
    max_loop_depth: LoopDepth,
    /// `local_count` local slots followed by `max_stack_size` stack slots.
    slots: [Option<Value<'v>>; 0],
}

const _: () = assert!(mem::size_of::<BcFrame>() % mem::size_of::<usize>() == 0);
const _: () = assert!(mem::align_of::<BcFrame>() == mem::align_of::<usize>());

#[derive(Copy, Clone, Dupe)]
pub(crate) struct BcFramePtr<'v> {
    /// Pointer to the `slots` field of `BcFrame`.
    ///
    /// We could store `BcFrame` pointer here, but since the most common
    /// data accessed is slots, storing `slots` pointer is slightly more efficient:
    /// no need to add a constant when accessing the field.
    slots_ptr: *mut Option<Value<'v>>,
}

impl<'v> PartialEq for BcFramePtr<'v> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.slots_ptr, other.slots_ptr)
    }
}

impl<'v> BcFramePtr<'v> {
    pub(crate) fn null() -> BcFramePtr<'v> {
        BcFramePtr {
            slots_ptr: ptr::null_mut(),
        }
    }

    /// Is this frame allocated or constructed empty?
    pub(crate) fn is_inititalized(self) -> bool {
        !self.slots_ptr.is_null()
    }

    #[inline(always)]
    fn frame<'a>(self) -> &'a BcFrame<'v> {
        debug_assert!(self.is_inititalized());
        unsafe {
            let frame = (self.slots_ptr as *mut u8).sub(BcFrame::offset_of_slots()) as *mut BcFrame;
            &*frame
        }
    }

    #[inline(always)]
    fn frame_mut<'a>(self) -> &'a mut BcFrame<'v> {
        debug_assert!(self.is_inititalized());
        unsafe {
            let frame = (self.slots_ptr as *mut u8).sub(BcFrame::offset_of_slots()) as *mut BcFrame;
            &mut *frame
        }
    }

    pub(crate) fn get_slot_slow(self, slot: LocalSlotIdCapturedOrNot) -> Option<Value<'v>> {
        assert!(slot.0 < self.frame().local_count);
        self.get_slot(slot)
    }

    #[inline(always)]
    pub(crate) fn get_slot(self, slot: LocalSlotIdCapturedOrNot) -> Option<Value<'v>> {
        self.frame().get_slot(slot)
    }

    pub(crate) fn set_slot_slow(self, slot: LocalSlotIdCapturedOrNot, value: Value<'v>) {
        assert!(slot.0 < self.frame().local_count);
        self.set_slot(slot, value);
    }

    #[inline(always)]
    pub(crate) fn set_slot(self, slot: LocalSlotIdCapturedOrNot, value: Value<'v>) {
        self.frame_mut().set_slot(slot, value)
    }

    #[inline(always)]
    pub(crate) fn get_bc_slot(self, slot: BcSlotIn) -> Value<'v> {
        self.frame().get_bc_slot(slot)
    }

    #[inline(always)]
    pub(crate) fn set_bc_slot(self, slot: BcSlotOut, value: Value<'v>) {
        self.frame_mut().set_bc_slot(slot, value)
    }

    #[inline(always)]
    pub(crate) fn get_bc_slot_range<'a>(self, slots: BcSlotInRange) -> &'a [Value<'v>] {
        self.frame().get_bc_slot_range(slots)
    }

    #[inline(always)]
    pub(crate) fn get_iter_index(self, loop_depth: LoopDepth) -> usize {
        self.frame().get_iter_index(loop_depth)
    }

    #[inline(always)]
    pub(crate) fn set_iter_index(self, loop_depth: LoopDepth, index: usize) {
        self.frame_mut().set_iter_index(loop_depth, index)
    }

    pub(crate) fn max_stack_size(self) -> u32 {
        self.frame().max_stack_size
    }

    #[inline(always)]
    pub(crate) unsafe fn locals_mut<'a>(self) -> &'a mut [Option<Value<'v>>] {
        self.frame_mut().locals_mut()
    }
}

impl<'v> BcFrame<'v> {
    #[inline(always)]
    fn offset_of_slots() -> usize {
        let offset = memoffset::offset_of!(BcFrame<'v>, slots);
        assert_eq!(mem::size_of::<BcFrame<'v>>(), offset);
        offset
    }

    #[inline(always)]
    fn frame_ptr(&mut self) -> BcFramePtr<'v> {
        unsafe {
            BcFramePtr {
                slots_ptr: (self as *mut _ as *mut u8).add(Self::offset_of_slots()) as *mut _,
            }
        }
    }

    #[inline(always)]
    fn locals_mut(&mut self) -> &mut [Option<Value<'v>>] {
        unsafe { slice::from_raw_parts_mut(self.slots.as_mut_ptr(), self.local_count as usize) }
    }

    #[inline(always)]
    fn locals_uninit(&mut self) -> &mut [MaybeUninit<Option<Value<'v>>>] {
        unsafe {
            slice::from_raw_parts_mut(self.slots.as_mut_ptr() as *mut _, self.local_count as usize)
        }
    }

    #[inline(always)]
    fn stack_uninit(&mut self) -> &mut [MaybeUninit<Value<'v>>] {
        unsafe {
            slice::from_raw_parts_mut(
                self.slots.as_mut_ptr().add(self.local_count as usize) as *mut _,
                self.max_stack_size as usize,
            )
        }
    }

    /// Initialize frame after it was allocated.
    #[inline(always)]
    fn init(&mut self) {
        self.locals_uninit().fill(MaybeUninit::new(None));

        if cfg!(debug_assertions) {
            // Write junk to the stack to trigger memory error if the stack is used incorrectly.
            unsafe {
                // Any bit pattern would do except
                // * a bit pattern which would make valid int tag (0b_xxxx_x010)
                // * zeros, which would be interpreted as uninitialized if copied to a local
                let byte = 0xef;

                let start = self.stack_uninit().as_ptr() as *mut u8;
                ptr::write_bytes(
                    start,
                    byte,
                    (self.max_stack_size as usize) * mem::size_of::<Value>(),
                );
            }
        }
    }

    /// Gets a local variable. Returns None to indicate the variable is not yet assigned.
    #[inline(always)]
    pub(crate) fn get_slot(&self, slot: LocalSlotIdCapturedOrNot) -> Option<Value<'v>> {
        debug_assert!(slot.0 < self.local_count);
        unsafe { self.slots.as_ptr().add(slot.0 as usize).read() }
    }

    /// Get a stack slot.
    #[inline(always)]
    pub(crate) fn get_bc_slot(&self, slot: BcSlotIn) -> Value<'v> {
        debug_assert!(slot.get().0 < self.local_count + self.max_stack_size);
        unsafe {
            let option = self.slots.as_ptr().add(slot.get().0 as usize).read();
            // Slot must be always initialized.
            option.unwrap_unchecked()
        }
    }

    #[inline(always)]
    pub(crate) fn set_slot(&mut self, slot: LocalSlotIdCapturedOrNot, value: Value<'v>) {
        debug_assert!(slot.0 < self.local_count);
        unsafe {
            self.slots
                .as_mut_ptr()
                .add(slot.0 as usize)
                .write(Some(value))
        }
    }

    #[inline(always)]
    pub(crate) fn get_bc_slot_range(&self, slots: BcSlotInRange) -> &[Value<'v>] {
        debug_assert!(slots.end.get().0 <= self.local_count + self.max_stack_size);
        unsafe {
            let start = self.slots.as_ptr().add(slots.start.get().0 as usize);
            let start = start as *const Value;
            slice::from_raw_parts(start, slots.len() as usize)
        }
    }

    #[inline(always)]
    pub(crate) fn set_bc_slot(&mut self, slot: BcSlotOut, value: Value<'v>) {
        debug_assert!(slot.get().0 < self.local_count + self.max_stack_size);
        unsafe {
            self.slots
                .as_mut_ptr()
                .add(slot.get().0 as usize)
                .write(Some(value))
        }
    }

    #[inline(always)]
    pub(crate) fn set_iter_index(&mut self, iter_index: LoopDepth, index: usize) {
        debug_assert!(iter_index < self.max_loop_depth);
        unsafe {
            (self as *mut BcFrame as *mut usize)
                .sub(iter_index.0 as usize)
                .sub(1)
                .write(index);
        }
    }

    #[inline(always)]
    pub(crate) fn get_iter_index(&self, iter_index: LoopDepth) -> usize {
        debug_assert!(iter_index < self.max_loop_depth);
        unsafe {
            (self as *const BcFrame as *const usize)
                .sub(iter_index.0 as usize)
                .sub(1)
                .read()
        }
    }
}

unsafe impl<'v> Trace<'v> for BcFrame<'v> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.locals_mut().trace(tracer);
        // Note this does not trace the stack.
        // GC can be performed only when the stack is empty.
    }
}

unsafe impl<'v> Trace<'v> for BcFramePtr<'v> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.frame_mut().trace(tracer);
    }
}

#[inline(always)]
fn alloca_raw<'v, 'a, 'e, R>(
    eval: &mut Evaluator<'v, 'a, 'e>,
    local_count: u32,
    max_stack_size: u32,
    max_loop_depth: LoopDepth,
    k: impl FnOnce(&mut Evaluator<'v, 'a, 'e>, BcFramePtr<'v>) -> R,
) -> R {
    assert_eq!(mem::align_of::<BcFrame>() % mem::size_of::<usize>(), 0);
    assert_eq!(mem::size_of::<Value>(), mem::size_of::<usize>());
    let alloca_size_in_words = mem::size_of::<BcFrame>() / mem::size_of::<usize>()
        + (local_count as usize)
        + (max_stack_size as usize)
        + (max_loop_depth.0 as usize);
    eval.alloca_uninit::<usize, _, _>(alloca_size_in_words, |slice, eval| unsafe {
        let frame_ptr = slice.as_mut_ptr().add(max_loop_depth.0 as usize) as *mut BcFrame;
        *(frame_ptr) = BcFrame {
            local_count,
            max_stack_size,
            max_loop_depth,
            slots: [],
        };

        k(eval, (*frame_ptr).frame_ptr())
    })
}

/// Allocate a frame and store it in the evaluator.
///
/// After callback finishes, previous frame is restored.
#[inline(always)]
pub(crate) fn alloca_frame<'v, 'a, 'e, R>(
    eval: &mut Evaluator<'v, 'a, 'e>,
    local_count: u32,
    max_stack_size: u32,
    loop_depth: LoopDepth,
    k: impl FnOnce(&mut Evaluator<'v, 'a, 'e>) -> R,
) -> R {
    alloca_raw(
        eval,
        local_count,
        max_stack_size,
        loop_depth,
        |eval, frame| {
            // TODO(nga): no need to fill the slots for parameters.
            frame.frame_mut().init();
            let old_frame = mem::replace(&mut eval.current_frame, frame);
            let r = k(eval);
            eval.current_frame = old_frame;
            r
        },
    )
}
