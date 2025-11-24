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

//! A heap storing AValue traits. The heap is a sequence of the
//! AValue vtable, followed by the payload.
//! Every payload must be at least 1 usize large (even ZST).
//! Some elements are created using reserve, in which case they point
//! to a BlackHole until they are filled in.
//!
//! Some elements can be overwritten (typically during GC) by a usize.
//! In these cases the bottom bit of the usize as used by the heap
//! to tag it as being a usize, and the word after is the size of the
//! item it replaced.

use std::collections::HashMap;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr;
use std::slice;

use allocative::Allocative;
use allocative::Visitor;
use bumpalo::Bump;
use dupe::Dupe;
use starlark_map::small_map::SmallMap;

use crate::collections::StarlarkHashValue;
use crate::eval::runtime::profile::instant::ProfilerInstant;
use crate::values::layout::aligned_size::AlignedSize;
use crate::values::layout::avalue::starlark_str;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::avalue::BlackHole;
use crate::values::layout::heap::allocator::api::ArenaAllocator;
use crate::values::layout::heap::allocator::api::ChunkAllocationDirection;
use crate::values::layout::heap::call_enter_exit::CallEnter;
use crate::values::layout::heap::call_enter_exit::CallExit;
use crate::values::layout::heap::call_enter_exit::NeedsDrop;
use crate::values::layout::heap::call_enter_exit::NoDrop;
use crate::values::layout::heap::heap_type::HeapKind;
use crate::values::layout::heap::profile::alloc_counts::AllocCounts;
use crate::values::layout::heap::profile::by_type::HeapSummary;
use crate::values::layout::heap::repr::AValueForward;
use crate::values::layout::heap::repr::AValueHeader;
use crate::values::layout::heap::repr::AValueOrForward;
use crate::values::layout::heap::repr::AValueOrForwardUnpack;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::layout::vtable::AValueVTable;
use crate::values::string::str_type::StarlarkStr;
use crate::values::Value;
use crate::values::ValueLike;

/// Min size of allocated object including header.
/// Should be able to fit `BlackHole` or forward.
pub(crate) const MIN_ALLOC: AlignedSize = {
    const fn max(a: AlignedSize, b: AlignedSize) -> AlignedSize {
        if a.bytes() > b.bytes() { a } else { b }
    }

    max(
        AlignedSize::of::<AValueForward>(),
        AlignedSize::of::<AValueRepr<BlackHole>>(),
    )
};

#[derive(Default)]
pub(crate) struct Arena<A: ArenaAllocator> {
    /// Arena for things which don't need dropping (e.g. strings)
    non_drop: A,
    /// Arena for things which might need dropping (e.g. Vec, with memory on heap)
    drop: A,
}

/// Reservation is morally a Reservation<T>, but we treat is as an
/// existential.
/// Tied to the lifetime of the heap.
pub(crate) struct Reservation<'v, T: AValue<'v>> {
    pointer: *mut AValueRepr<T::StarlarkValue>,
}

impl<'v, T: AValue<'v>> Reservation<'v, T> {
    pub(crate) fn fill(self, x: T::StarlarkValue) {
        unsafe {
            ptr::write(
                self.pointer,
                AValueRepr {
                    header: AValueHeader::new::<T>(),
                    payload: x,
                },
            );
        }
    }

    pub(crate) fn ptr(&self) -> &'v AValueHeader {
        unsafe { &(*self.pointer).header }
    }
}

pub(crate) trait ArenaVisitor<'v> {
    fn enter_bump(&mut self);
    fn regular_value(&mut self, value: &'v AValueOrForward);
    fn call_enter(&mut self, function: Value<'v>, time: ProfilerInstant);
    fn call_exit(&mut self, time: ProfilerInstant);
}

/// Iterate over chunk contents.
struct ChunkIter<'c> {
    chunk: &'c [MaybeUninit<u8>],
}

impl<'c> Iterator for ChunkIter<'c> {
    type Item = &'c AValueOrForward;

    fn next(&mut self) -> Option<&'c AValueOrForward> {
        unsafe {
            if self.chunk.is_empty() {
                None
            } else {
                let or_forward = &*(self.chunk.as_ptr() as *const AValueOrForward);
                let n = or_forward.alloc_size();
                debug_assert!(n.bytes() as usize <= self.chunk.len());
                self.chunk = self.chunk.split_at(n.bytes() as usize).1;
                Some(or_forward)
            }
        }
    }
}

/// Result of allocation. Both fields are uninitialized.
pub(crate) struct ArenaUninit<'v, T: AValue<'v>> {
    // We use `MaybeUninit` here to emphasize that the memory is uninitialized.
    repr: *mut MaybeUninit<AValueRepr<T::StarlarkValue>>,
    extra: *mut [MaybeUninit<T::ExtraElem>],
}

impl<'v, T: AValue<'v>> ArenaUninit<'v, T> {
    pub(crate) unsafe fn write_black_hole(
        self,
        extra_len: usize,
    ) -> (Reservation<'v, T>, *mut [MaybeUninit<T::ExtraElem>]) {
        let p = self.repr as *mut AValueRepr<BlackHole>;
        p.write(AValueRepr {
            header: AValueHeader(AValueVTable::new_black_hole()),
            payload: BlackHole(T::alloc_size_for_extra_len(extra_len)),
        });
        (
            Reservation {
                pointer: p as *mut _,
            },
            self.extra,
        )
    }

    pub(crate) fn debug_assert_extra_is_empty(&self) {
        let extra = unsafe { &*self.extra };
        debug_assert!(extra.is_empty());
    }

    pub(crate) fn write(
        self,
        x: T::StarlarkValue,
    ) -> (
        *mut AValueRepr<AValueImpl<'v, T>>,
        *mut [MaybeUninit<T::ExtraElem>],
    ) {
        unsafe {
            let repr = self.repr as *mut AValueRepr<AValueImpl<'v, T>>;
            repr.write(AValueRepr {
                header: AValueHeader::new::<T>(),
                payload: AValueImpl::new(x),
            });
            (repr, self.extra)
        }
    }

    pub(crate) fn write_no_extra(self, x: T::StarlarkValue) -> *mut AValueRepr<AValueImpl<'v, T>> {
        self.debug_assert_extra_is_empty();
        self.write(x).0
    }
}

enum ArenaVisitEvent<'a> {
    /// Called when entering new bump.
    EnterBump,
    /// Visiting a value in the bump.
    Value(&'a AValueOrForward),
}

impl<A: ArenaAllocator> Arena<A> {
    pub(crate) fn is_empty(&self) -> bool {
        self.allocated_bytes() == 0
    }

    /// Number of allocated bytes plus padding size.
    pub(crate) fn allocated_bytes(&self) -> usize {
        // This overestimates the allocates size, see `Bump::allocated_bytes()`:
        // "those padding bytes will add to this method’s resulting sum".
        // https://docs.rs/bumpalo/3.11.0/bumpalo/struct.Bump.html#method.allocated_bytes
        self.drop.allocated_bytes() + self.non_drop.allocated_bytes()
    }

    pub(crate) fn available_bytes(&self) -> usize {
        self.drop.remaining_capacity() + self.non_drop.remaining_capacity()
    }

    /// Don't forget to call this function to release memory.
    pub(crate) fn finish(&mut self) {
        self.drop.finish();
        self.non_drop.finish();
    }

    fn alloc_uninit<'v, 'v2, T: AValue<'v2>>(bump: &'v A, extra_len: usize) -> ArenaUninit<'v2, T> {
        assert!(
            mem::align_of::<T>() <= AValueHeader::ALIGN,
            "Unexpected alignment in Starlark arena. Type {} has alignment {}, expected <= {}",
            std::any::type_name::<T>(),
            mem::align_of::<T>(),
            AValueHeader::ALIGN,
        );

        let size = T::alloc_size_for_extra_len(extra_len);
        let p = bump.alloc(size).as_ptr();
        unsafe {
            let repr = p as *mut MaybeUninit<AValueRepr<_>>;
            let extra = slice::from_raw_parts_mut(
                p.add(AValueRepr::<T>::offset_of_extra()) as *mut _,
                extra_len,
            );
            ArenaUninit { repr, extra }
        }
    }

    fn bump_for_type<'v, T: AValue<'v>>(&self) -> &A {
        if mem::needs_drop::<T::StarlarkValue>() {
            &self.drop
        } else {
            &self.non_drop
        }
    }

    // Reservation should really be an incremental type
    pub(crate) fn reserve_with_extra<'v2, T: AValue<'v2>>(
        &self,
        extra_len: usize,
    ) -> (Reservation<'v2, T>, *mut [MaybeUninit<T::ExtraElem>]) {
        // We don't create reservations for strings because we don't need to,
        // but also because we need to be able to reconstruct a `Pointer`
        // from `AValueHeader` (with `TAG_STR` when appropriate).
        // `BlackHole` assumes it is created for non-string, so
        // it returns `false` from `is_str`.
        assert!(!T::IS_STR);

        let arena_uninit = Self::alloc_uninit::<T>(self.bump_for_type::<T>(), extra_len);
        // If we don't have a vtable we can't skip over missing elements to drop,
        // so very important to put in a current vtable
        // We always alloc at least one pointer worth of space, so can write in a one-ST blackhole

        unsafe { arena_uninit.write_black_hole(extra_len) }
    }

    /// Allocate a type `T`.
    pub(crate) fn alloc<'v, 'v2, T: AValue<'v2, ExtraElem = ()>>(
        &'v self,
        x: AValueImpl<'v2, T>,
    ) -> &'v AValueRepr<AValueImpl<'v2, T>> {
        debug_assert!(T::extra_len(&x.1) == 0);
        let bump = self.bump_for_type::<T>();
        let arena_uninit = Self::alloc_uninit::<T>(bump, 0);
        arena_uninit.debug_assert_extra_is_empty();
        unsafe { &mut *arena_uninit.write_no_extra(x.1) }
    }

    /// Allocate a type `T` plus `extra` bytes.
    ///
    /// The type `T` will never be dropped, so had better not do any memory allocation.
    pub(crate) fn alloc_extra<'v, T: AValue<'v>>(
        &self,
        x: AValueImpl<'v, T>,
    ) -> (
        *mut AValueRepr<AValueImpl<'v, T>>,
        *mut [MaybeUninit<T::ExtraElem>],
    ) {
        let bump = self.bump_for_type::<T>();
        let extra_len = T::extra_len(&x.1);
        let arena_uninit = Self::alloc_uninit::<T>(bump, extra_len);
        let (p, extra) = arena_uninit.write(x.1);
        (p, extra)
    }

    #[inline]
    pub(crate) fn alloc_str_init(
        &self,
        len: usize,
        hash: StarlarkHashValue,
        init: impl FnOnce(*mut u8),
    ) -> *mut AValueHeader {
        assert!(len > 1);
        let (v, extra) = self.alloc_extra::<_>(starlark_str(len, hash));
        let extra = unsafe { &mut *extra };
        debug_assert_eq!(StarlarkStr::payload_len_for_len(len), extra.len());
        unsafe {
            extra.last_mut().unwrap_unchecked().write(0usize);
        }
        init(extra.as_mut_ptr() as *mut u8);
        unsafe { &mut (*v).header }
    }

    #[inline]
    pub(crate) fn alloc_str(&self, x: &str) -> *mut AValueHeader {
        self.alloc_str_init(x.len(), StarlarkStr::UNINIT_HASH, |dest| unsafe {
            ptr::copy_nonoverlapping(x.as_ptr(), dest, x.len())
        })
    }

    fn iter_chunk<'a>(chunk: &'a [MaybeUninit<u8>]) -> ChunkIter<'a> {
        ChunkIter { chunk }
    }

    // Iterate over the values in the heap in the order they
    // were added.
    fn for_each_ordered<'a>(&'a mut self, mut f: impl FnMut(ArenaVisitEvent<'a>)) {
        // We get the chunks from most newest to oldest as per the bumpalo spec.
        // And within each chunk, the values are filled newest to oldest.
        // So need to do two sets of reversing.
        for bump in [&mut self.drop, &mut self.non_drop] {
            f(ArenaVisitEvent::EnterBump);
            let chunks = unsafe { bump.iter_allocated_chunks_rev().collect::<Vec<_>>() };
            // Use a single buffer to reduce allocations, but clear it after use
            let mut buffer = Vec::new();
            for chunk in chunks.iter().rev() {
                match A::CHUNK_ALLOCATION_DIRECTION {
                    ChunkAllocationDirection::Down => {
                        buffer.extend(Arena::<A>::iter_chunk(chunk));
                        for x in buffer.iter().rev() {
                            f(ArenaVisitEvent::Value(x));
                        }
                        buffer.clear();
                    }
                    ChunkAllocationDirection::Up => {
                        for x in Arena::<A>::iter_chunk(chunk) {
                            f(ArenaVisitEvent::Value(x));
                        }
                    }
                }
            }
        }
    }

    pub(crate) unsafe fn visit_arena<'v>(
        &'v mut self,
        heap_kind: HeapKind,
        forward_heap_kind: HeapKind,
        visitor: &mut impl ArenaVisitor<'v>,
    ) {
        fn fix_function<'v>(function: Value<'v>, forward_heap_kind: HeapKind) -> Value<'v> {
            if let Some(function) = function.unpack_frozen() {
                return function.to_value();
            }

            unsafe {
                match function
                    .0
                    .unpack_ptr()
                    .expect("int cannot be stored in heap")
                    .unpack_forward()
                {
                    None => function,
                    Some(forward) => forward.forward_ptr().unpack_value(forward_heap_kind),
                }
            }
        }

        self.for_each_ordered(|x| match x {
            ArenaVisitEvent::EnterBump => visitor.enter_bump(),
            ArenaVisitEvent::Value(x) => match x.unpack() {
                AValueOrForwardUnpack::Header(header) => {
                    let value = header.unpack_value(heap_kind);
                    if let Some(call_enter) = value.downcast_ref::<CallEnter<NeedsDrop>>() {
                        visitor.call_enter(
                            fix_function(call_enter.function, forward_heap_kind),
                            call_enter.time,
                        );
                    } else if let Some(call_enter) = value.downcast_ref::<CallEnter<NoDrop>>() {
                        visitor.call_enter(
                            fix_function(call_enter.function, forward_heap_kind),
                            call_enter.time,
                        );
                    } else if let Some(call_exit) = value.downcast_ref::<CallExit<NeedsDrop>>() {
                        visitor.call_exit(call_exit.time);
                    } else if let Some(call_exit) = value.downcast_ref::<CallExit<NoDrop>>() {
                        visitor.call_exit(call_exit.time);
                    } else {
                        visitor.regular_value(x);
                    }
                }
                AValueOrForwardUnpack::Forward(_forward) => visitor.regular_value(x),
            },
        });
    }

    // Iterate over the values in the drop bump in any order
    pub(crate) fn for_each_drop_unordered<'a>(&'a mut self, mut f: impl FnMut(&'a AValueHeader)) {
        unsafe {
            for chunk in self.drop.iter_allocated_chunks_rev() {
                for x in Arena::<A>::iter_chunk(chunk) {
                    if let Some(x) = x.unpack_header() {
                        f(x);
                    }
                }
            }
        }
    }

    fn for_each_unordered_in_bump<'a>(bump: &'a A, mut f: impl FnMut(&'a AValueHeader)) {
        // SAFETY: We're consuming the iterator immediately and not allocating from the arena during.
        unsafe {
            bump.iter_allocated_chunks_rev().for_each(|slice| {
                for x in Arena::<A>::iter_chunk(slice) {
                    if let Some(x) = x.unpack_header() {
                        f(x);
                    }
                }
            })
        }
    }

    // Iterate over the values in the both bumps in any order
    fn for_each_unordered<'a>(&'a self, mut f: impl FnMut(&'a AValueHeader)) {
        for bump in [&self.drop, &self.non_drop] {
            Self::for_each_unordered_in_bump(bump, &mut f);
        }
    }

    // For each Rust-level type (the String) report how many entries there are in the heap, and how much size they consume
    pub(crate) fn allocated_summary(&self) -> HeapSummary {
        // Record how many times each header occurs
        // We deliberately hash by the AValueHeader for higher performance, less type lookup
        let mut entries: HashMap<AValueHeader, (&'static str, AllocCounts)> = HashMap::new();
        let f = |x: &AValueHeader| {
            let v = x.unpack();
            let e = entries
                .entry(x.dupe())
                .or_insert_with(|| (v.vtable().type_name, AllocCounts::default()));
            e.1.count += 1;
            e.1.bytes += v.total_memory()
        };
        self.for_each_unordered(f);

        // For a given type, the AValueHeader isn't always unique
        // (if they get compiled in different translation units),
        // so not just a simple map.
        let mut summary = SmallMap::new();
        for (_, (name, counts)) in entries {
            *summary.entry(name).or_insert_with(AllocCounts::default) += counts;
        }
        HeapSummary { summary }
    }
}

impl<A: ArenaAllocator> Drop for Arena<A> {
    fn drop(&mut self) {
        self.for_each_drop_unordered(|x| {
            // Safe to convert to *mut because we are the only owner
            let value = x.payload_ptr();
            x.0.drop_in_place(value);
        });
    }
}

impl<A: ArenaAllocator> Allocative for Arena<A> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut allocative::Visitor<'b>) {
        let Arena { drop, non_drop } = self;

        fn visit_bump<'a, 'b: 'a, A: ArenaAllocator>(bump: &A, visitor: &'a mut Visitor<'b>) {
            let mut visitor =
                visitor.enter_unique(allocative::Key::new("data"), mem::size_of::<*const ()>());
            let mut allocated_visitor =
                visitor.enter(allocative::Key::new("allocated"), bump.allocated_bytes());
            Arena::for_each_unordered_in_bump(bump, |x| {
                let key = x.0.type_as_allocative_key.clone();
                let size = x.alloc_size();
                let mut object_visitor = allocated_visitor.enter(key, size.bytes() as usize);
                // We visit both drop and non-drop bumps, because although
                // non-drop `Bump` cannot contain malloc pointers, it can still provide
                // useful information about headers/payload/padding.
                x.unpack().as_allocative().visit(&mut object_visitor);
                object_visitor.exit();
            });
            allocated_visitor.exit();
            visitor.visit_simple(
                allocative::Key::new("allocation_overhead"),
                bump.allocation_overhead(),
            );
            visitor.exit();
        }

        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let mut visitor = visitor.enter(allocative::Key::new("drop"), mem::size_of::<Bump>());
            visit_bump(drop, &mut visitor);
            visitor.exit();
        }
        {
            let mut visitor =
                visitor.enter(allocative::Key::new("non_drop"), mem::size_of::<Bump>());
            visit_bump(non_drop, &mut visitor);
            visitor.exit();
        }
        visitor.exit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::any::StarlarkAny;
    use crate::values::layout::avalue::simple;

    fn to_repr(x: &AValueHeader) -> String {
        let mut s = String::new();
        x.unpack().collect_repr(&mut s);
        s
    }

    fn mk_str(x: &str) -> AValueImpl<'static, impl AValue<'static, ExtraElem = ()>> {
        simple(StarlarkAny::new(x.to_owned()))
    }

    fn reserve_str<'v, T: AValue<'static>>(
        arena: &'v Arena<Bump>,
        _: &AValueImpl<'static, T>,
    ) -> Reservation<'static, T> {
        arena.reserve_with_extra::<T>(0).0
    }

    #[test]
    fn test_trait_arena_iteration() {
        // We want iteration to proceed in the same order as allocation,
        // otherwise profiling won't work
        const LIMIT: usize = 10000;
        let mut arena = Arena::default();
        let mut reserved = Vec::new();
        for i in 0..LIMIT {
            if i % 100 == 0 {
                let r = reserve_str(&arena, &mk_str(""));
                reserved.push((r, i));
            } else {
                arena.alloc(mk_str(&i.to_string()));
            }
        }
        assert!(!reserved.is_empty());
        for (r, i) in reserved {
            r.fill(mk_str(&i.to_string()).1);
        }

        // Not a functional part of the test, just makes sure we go through
        // the interesting cases (last time 56 was sufficient, so 10K is plenty of margin of error)
        assert!(
            arena.drop.iter_allocated_chunks().count() > 1,
            "Didn't allocate enough to test properly"
        );
        let mut j = 0;
        arena.for_each_ordered(|i| match i {
            ArenaVisitEvent::EnterBump => {}
            ArenaVisitEvent::Value(i) => {
                if let Some(i) = i.unpack_header() {
                    assert_eq!(to_repr(i), format!("{:?}", j.to_string()));
                    j += 1;
                }
            }
        });
        assert_eq!(j, LIMIT);
        j = 0;
        arena.for_each_drop_unordered(|_| j += 1);
        assert_eq!(j, LIMIT);
    }

    #[test]
    // Make sure that even if there are some blackholes when we drop, we can still walk to heap
    fn drop_with_blackhole() {
        let mut arena = Arena::default();
        arena.alloc(mk_str("test"));
        // reserve but do not fill!
        reserve_str(&arena, &mk_str(""));
        arena.alloc(mk_str("hello"));
        let mut res = Vec::new();
        arena.for_each_ordered(|x| match x {
            ArenaVisitEvent::EnterBump => {}
            ArenaVisitEvent::Value(x) => {
                if let Some(x) = x.unpack_header() {
                    res.push(x);
                }
            }
        });
        assert_eq!(res.len(), 3);
        assert_eq!(to_repr(res[0]), "\"test\"");
        assert_eq!(to_repr(res[2]), "\"hello\"");
    }

    #[test]
    fn test_allocated_summary() {
        let arena = Arena::<Bump>::default();
        arena.alloc(mk_str("test"));
        arena.alloc(mk_str("test"));
        let res = arena.allocated_summary().summary;
        assert_eq!(res.len(), 1);
        let entry = res.values().next().unwrap();
        assert_eq!(entry.count, 2);
        // Because `arena.allocated_bytes` over-approximates.
        assert!(entry.bytes <= arena.allocated_bytes());
    }

    #[test]
    fn test_is_empty() {
        let arena = Arena::<Bump>::default();
        assert!(arena.is_empty());
        arena.alloc_str("xyz");
        assert!(!arena.is_empty());
    }
}
