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

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::fmt;
use std::fmt::Debug;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::slice;

use crate::values::layout::aligned_size::AlignedSize;
use crate::values::layout::heap::allocator::alloc::chain::ChunkChain;
use crate::values::layout::heap::allocator::alloc::chain::ChunkChainIterator;
use crate::values::layout::heap::allocator::alloc::per_thread::thread_local_alloc_at_least;
use crate::values::layout::heap::allocator::alloc::per_thread::thread_local_release;
use crate::values::layout::heap::allocator::api::ArenaAllocator;
use crate::values::layout::heap::allocator::api::ChunkAllocationDirection;
use crate::values::layout::value_alloc_size::ValueAllocSize;

pub(crate) struct ChunkAllocator {
    /// Current chunk in the chunk chain is partially filled.
    /// The rest of the chain contain allocated data.
    chain: UnsafeCell<ChunkChain>,
    // TODO(nga): we don't need pointers after the heap is frozen.
    /// Pointer to the currently filled part of the chunk.
    current_ptr: Cell<NonNull<usize>>,
    /// Pointer to the end of the current chunk part.
    end_ptr: Cell<NonNull<usize>>,
}

impl Debug for ChunkAllocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkAllocator").finish_non_exhaustive()
    }
}

unsafe impl Send for ChunkAllocator {}

impl Default for ChunkAllocator {
    #[inline]
    fn default() -> ChunkAllocator {
        let chain = ChunkChain::default();
        let current_ptr = Cell::new(chain.begin());
        let end_ptr = Cell::new(chain.begin());
        ChunkAllocator {
            chain: UnsafeCell::new(chain),
            current_ptr,
            end_ptr,
        }
    }
}

impl Drop for ChunkAllocator {
    fn drop(&mut self) {
        self.chain.get_mut().clear_with(&mut thread_local_release);
    }
}

impl ChunkAllocator {
    fn replace_chain(&self, chain: ChunkChain) -> (ChunkChain, NonNull<usize>) {
        unsafe {
            let current_ptr = self.current_ptr.replace(chain.begin());
            self.end_ptr.set(chain.end());
            let chain = mem::replace(&mut *self.chain.get(), chain);
            (chain, current_ptr)
        }
    }

    fn take_chain(&self) -> (ChunkChain, NonNull<usize>) {
        self.replace_chain(ChunkChain::default())
    }

    #[inline]
    fn try_alloc_fast(&self, len: AlignedSize) -> Option<NonNull<u8>> {
        let rem = AlignedSize::ptr_diff(self.current_ptr.get(), self.end_ptr.get());
        if rem >= len {
            let ptr = self.current_ptr.get();
            unsafe {
                self.current_ptr.set(NonNull::new_unchecked(
                    ptr.as_ptr().byte_add(len.bytes() as usize),
                ));
            }
            Some(ptr.cast())
        } else {
            None
        }
    }

    #[cold]
    fn alloc_slow(&self, len: AlignedSize) -> NonNull<u8> {
        let (chain, current_ptr) = self.take_chain();
        let (rem_chain, after) = unsafe { chain.split_at_ptr(current_ptr) };
        thread_local_release(after);

        let required_len = len + ChunkChain::HEADER_SIZE;
        let next_chunk = thread_local_alloc_at_least(required_len, rem_chain.depth());

        let next_chain = ChunkChain::new(next_chunk, rem_chain);

        self.replace_chain(next_chain);

        self.try_alloc_fast(len)
            .expect("try_allow_fast must not fail in alloc_slow")
    }
}

pub(crate) struct ChunkRevIterator<'a> {
    current: &'a [MaybeUninit<u8>],
    chain: ChunkChainIterator<'a>,
}

impl<'a> Iterator for ChunkRevIterator<'a> {
    type Item = &'a [MaybeUninit<u8>];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if !self.current.is_empty() {
            // Reset to empty slice and return.
            return Some(mem::take(&mut self.current));
        }
        loop {
            let chain = self.chain.next()?;
            let data = chain.data_bytes();
            if !data.is_empty() {
                return Some(data);
            }
        }
    }
}

impl ArenaAllocator for ChunkAllocator {
    fn allocated_bytes(&self) -> usize {
        unsafe { (*self.chain.get()).allocated_bytes() }
    }

    fn remaining_capacity(&self) -> usize {
        AlignedSize::ptr_diff(self.current_ptr.get(), self.end_ptr.get()).bytes() as usize
    }

    fn allocation_overhead(&self) -> usize {
        unsafe {
            let allocated_bytes_with_metadata = (*self.chain.get()).allocated_bytes_with_metadata();
            allocated_bytes_with_metadata.saturating_sub(self.allocated_bytes())
        }
    }

    #[inline]
    fn alloc(&self, size: ValueAllocSize) -> NonNull<u8> {
        if let Some(ptr) = self.try_alloc_fast(size.size()) {
            ptr
        } else {
            self.alloc_slow(size.size())
        }
    }

    const CHUNK_ALLOCATION_DIRECTION: ChunkAllocationDirection = ChunkAllocationDirection::Up;

    type ChunkRevIterator<'a> = ChunkRevIterator<'a>;

    unsafe fn iter_allocated_chunks_rev(&self) -> ChunkRevIterator<'_> {
        let begin = (*self.chain.get()).begin();
        ChunkRevIterator {
            current: slice::from_raw_parts(
                begin.cast().as_ptr(),
                AlignedSize::ptr_diff(begin, self.current_ptr.get()).bytes() as usize,
            ),
            chain: (*self.chain.get())
                .prev()
                .map(|next| next.iter())
                .unwrap_or_default(),
        }
    }

    fn finish(&mut self) {
        let (chain, current_ptr) = self.take_chain();
        let (new_chain, rem) = unsafe { chain.split_at_ptr(current_ptr) };
        thread_local_release(rem);
        let current_ptr = new_chain.end();
        self.replace_chain(new_chain);
        self.current_ptr.set(current_ptr);
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::SmallRng;
    use rand::Rng;
    use rand::SeedableRng;

    use crate::values::layout::aligned_size::AlignedSize;
    use crate::values::layout::heap::allocator::alloc::allocator::ChunkAllocator;
    use crate::values::layout::heap::allocator::alloc::chunk::Chunk;
    use crate::values::layout::heap::allocator::api::ArenaAllocator;
    use crate::values::layout::heap::repr::AValueHeader;
    use crate::values::layout::value_alloc_size::ValueAllocSize;

    #[test]
    fn test_small() {
        let allocator = ChunkAllocator::default();
        let p0 = allocator.alloc(ValueAllocSize::new(AlignedSize::new_bytes(
            3 * AValueHeader::ALIGN,
        )));
        let p1 = allocator.alloc(ValueAllocSize::new(AlignedSize::new_bytes(
            4 * AValueHeader::ALIGN,
        )));
        let p2 = allocator.alloc(ValueAllocSize::new(AlignedSize::new_bytes(
            5 * AValueHeader::ALIGN,
        )));
        assert_eq!(
            AlignedSize::new_bytes(3 * AValueHeader::ALIGN),
            AlignedSize::ptr_diff(p0.cast(), p1.cast())
        );
        assert_eq!(
            AlignedSize::new_bytes(4 * AValueHeader::ALIGN),
            AlignedSize::ptr_diff(p1.cast(), p2.cast())
        );

        let chunks = unsafe { allocator.iter_allocated_chunks_rev().collect::<Vec<_>>() };
        assert_eq!(1, chunks.len());
        assert_eq!(
            AlignedSize::new_bytes((3 + 4 + 5) * AValueHeader::ALIGN).bytes() as usize,
            chunks[0].len()
        );
    }

    #[test]
    fn test_big() {
        let allocator = ChunkAllocator::default();
        allocator.alloc(ValueAllocSize::new(
            AlignedSize::new_bytes(128 << 10) - Chunk::HEADER_SIZE,
        ));
    }

    fn random_iteration(i: u32) {
        let mut rng = SmallRng::seed_from_u64(i as u64);

        let mut expected_total_size_bytes = 0;
        let mut allocator = ChunkAllocator::default();
        for _ in 0..i {
            let size = match rng.gen_range(0..=2) {
                0 => rng.gen_range(0..10),
                1 => rng.gen_range(0..100),
                2 => rng.gen_range(0..1000),
                _ => unreachable!(),
            };
            let Some(size) =
                ValueAllocSize::try_new(AlignedSize::new_bytes(size * AValueHeader::ALIGN))
            else {
                continue;
            };
            allocator.alloc(size);
            expected_total_size_bytes += size.bytes() as usize;
        }

        let actual_total_size_bytes = unsafe {
            allocator
                .iter_allocated_chunks_rev()
                .map(|c| c.len())
                .sum::<usize>()
        };
        assert_eq!(expected_total_size_bytes, actual_total_size_bytes);

        // And do the same assertion after finishing.
        allocator.finish();

        let actual_total_size_bytes = unsafe {
            allocator
                .iter_allocated_chunks_rev()
                .map(|c| c.len())
                .sum::<usize>()
        };
        assert_eq!(expected_total_size_bytes, actual_total_size_bytes);
    }

    #[test]
    fn test_many() {
        for i in 0..10000 {
            random_iteration(i);
        }
    }
}
