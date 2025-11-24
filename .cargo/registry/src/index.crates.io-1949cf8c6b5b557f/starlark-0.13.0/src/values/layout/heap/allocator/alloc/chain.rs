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
use std::mem;
use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::NonNull;
use std::slice;

use static_assertions::assert_eq_size;

use crate::values::layout::aligned_size::AlignedSize;
use crate::values::layout::heap::allocator::alloc::chunk_part::ChunkPart;

/// What is stored inside `ChunkChain.chunk` data.
#[repr(C, align(8))] // Must be aligned to 8 bytes because starlark values are 8 bytes aligned.
struct ChunkChainData {
    prev: ChunkChain,
    data: [MaybeUninit<u8>; 0],
}

/// Linked list of chunk parts.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct ChunkChain {
    /// Chunk part data is `ChunkChainData`.
    /// `None` means that the chain is empty.
    chunk: Option<ChunkPart>,
}

assert_eq_size!(ChunkChain, ChunkPart);

impl Drop for ChunkChain {
    fn drop(&mut self) {
        self.clear_with(&mut |_| {});
    }
}

static EMPTY_DATA: &[usize] = &[];

thread_local! {
    /// Running `test_split_at_zero` test.
    static SPLIT_AT_ZERO_TEST: Cell<bool> = const { Cell::new(false) };
}

impl ChunkChain {
    /// The header is `ChunkChain`.
    pub(crate) const HEADER_SIZE: AlignedSize = AlignedSize::of::<ChunkChainData>();

    #[inline]
    pub(crate) fn new(chunk: ChunkPart, prev: ChunkChain) -> ChunkChain {
        // Does not have to be strictly greater, but it is pointless otherwise.
        assert!(chunk.len() > ChunkChain::HEADER_SIZE);
        unsafe {
            ptr::write(
                chunk.begin().cast().as_ptr(),
                ChunkChainData { prev, data: [] },
            );
        }
        ChunkChain { chunk: Some(chunk) }
    }

    #[inline]
    pub(crate) fn prev(&self) -> Option<&ChunkChain> {
        match &self.chunk {
            Some(chunk) => unsafe {
                Some(&(*chunk.begin().cast::<ChunkChainData>().as_ptr()).prev)
            },
            None => None,
        }
    }

    /// Size of memory available for allocation in the current chunk part,
    /// that is the size of the current chunk part minus the header.
    pub(crate) fn current_chunk_available_len(&self) -> AlignedSize {
        // This code can be made branchless if we allocate the prev statically.
        match &self.chunk {
            Some(chunk) => chunk.len().unchecked_sub(ChunkChain::HEADER_SIZE),
            None => AlignedSize::ZERO,
        }
    }

    pub(crate) fn begin(&self) -> NonNull<usize> {
        match &self.chunk {
            Some(chunk) => chunk.ptr_at_offset(ChunkChain::HEADER_SIZE),
            None => NonNull::new(EMPTY_DATA.as_ptr() as *mut usize).unwrap(),
        }
    }

    pub(crate) fn end(&self) -> NonNull<usize> {
        match &self.chunk {
            Some(chunk) => chunk.end(),
            None => NonNull::new(EMPTY_DATA.as_ptr() as *mut usize).unwrap(),
        }
    }

    pub(crate) fn data_bytes(&self) -> &[MaybeUninit<u8>] {
        unsafe {
            slice::from_raw_parts(
                self.begin().cast().as_ptr(),
                self.current_chunk_available_len().bytes() as usize,
            )
        }
    }

    /// Split current chunk in the chain at the given offset.
    pub(crate) fn split_at(mut self, offset: AlignedSize) -> (ChunkChain, ChunkPart) {
        let chunk = mem::take(&mut self.chunk);
        match chunk {
            None => {
                assert_eq!(AlignedSize::ZERO, offset);
                (ChunkChain::default(), ChunkPart::default())
            }
            Some(chunk) => {
                debug_assert!(chunk.len() > ChunkChain::HEADER_SIZE);

                let (before, after) = chunk.split_at_offset(offset + ChunkChain::HEADER_SIZE);
                assert!(before.len() >= ChunkChain::HEADER_SIZE);
                if before.len() == ChunkChain::HEADER_SIZE {
                    // This branch is only taken in tests of `ChunkChain`,
                    // because real allocator never finishes with an empty last chunk part.
                    // For that reason we don't care about somewhat unoptimal code below:
                    // we could extend the `after` chunk with the `before` chunk.
                    assert!(cfg!(test) && SPLIT_AT_ZERO_TEST.get());

                    unsafe {
                        // We are abandoning `before` chunk,
                        // and it won't run the destructor of `ChunkChainData`,
                        // so it is safe to read the data.
                        let prev_part: ChunkChainData = ptr::read(before.begin().cast().as_ptr());
                        (prev_part.prev, after)
                    }
                } else {
                    (
                        ChunkChain {
                            chunk: Some(before),
                        },
                        after,
                    )
                }
            }
        }
    }

    pub(crate) unsafe fn split_at_ptr(self, ptr: NonNull<usize>) -> (ChunkChain, ChunkPart) {
        debug_assert!(ptr >= self.begin());
        debug_assert!(ptr <= self.end());
        let offset =
            AlignedSize::new_bytes(ptr.as_ptr().byte_offset_from(self.begin().as_ptr()) as usize);
        self.split_at(offset)
    }

    /// Clear the content invoking provided callback to release the chunks.
    pub(crate) fn clear_with(&mut self, chunk_drop: &mut impl FnMut(ChunkPart)) {
        if let Some(chunk) = mem::take(&mut self.chunk) {
            assert!(chunk.len() >= ChunkChain::HEADER_SIZE);
            let mut prev_chain = chunk.begin().cast::<ChunkChainData>();
            unsafe {
                prev_chain.as_mut().prev.clear_with(chunk_drop);

                debug_assert!(prev_chain.as_ref().prev.chunk.is_none());
                // This is no-op, because `clear_with` has replaced the chunk with `None`.
                ptr::drop_in_place::<ChunkChainData>(prev_chain.as_ptr());
            }
            chunk_drop(chunk);
        }
    }

    pub(crate) fn iter(&self) -> ChunkChainIterator {
        ChunkChainIterator { next: Some(self) }
    }

    pub(crate) fn depth(&self) -> usize {
        self.iter().count().saturating_sub(1)
    }

    pub(crate) fn allocated_bytes(&self) -> usize {
        let mut allocated_bytes = 0;
        for chain in self.iter() {
            allocated_bytes += chain.current_chunk_available_len().bytes() as usize;
        }
        allocated_bytes
    }

    /// Returns the total size of the allocation backing this chunk,
    /// including any overhead used by the allocator itself.
    pub(crate) fn allocated_bytes_with_metadata(&self) -> usize {
        let mut allocation_overhead = 0;
        for chain in self.iter() {
            if let Some(chunk) = &chain.chunk {
                allocation_overhead += chunk.allocated_bytes_with_metadata();
            }
        }
        allocation_overhead
    }
}

/// Iterator over `ChunkChain` elements.
#[derive(Default)]
pub(crate) struct ChunkChainIterator<'a> {
    next: Option<&'a ChunkChain>,
}

impl<'a> Iterator for ChunkChainIterator<'a> {
    type Item = &'a ChunkChain;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next?;
        self.next = next.prev();
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use crate::values::layout::aligned_size::AlignedSize;
    use crate::values::layout::heap::allocator::alloc::chain::ChunkChain;
    use crate::values::layout::heap::allocator::alloc::chain::SPLIT_AT_ZERO_TEST;
    use crate::values::layout::heap::allocator::alloc::chunk_part::ChunkPart;
    use crate::values::layout::heap::repr::AValueHeader;

    #[test]
    fn test_default() {
        let chain = ChunkChain::default();
        assert_eq!(AlignedSize::ZERO, chain.current_chunk_available_len());
    }

    #[test]
    fn test_new_drop() {
        let chunk_part =
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(10 * AValueHeader::ALIGN));
        let chunk_len = chunk_part.len();
        let mut chain = ChunkChain::new(chunk_part, ChunkChain::default());
        assert_eq!(
            chunk_len,
            chain.current_chunk_available_len() + ChunkChain::HEADER_SIZE
        );
        let mut drop_called = false;
        chain.clear_with(&mut |_| {
            assert!(!drop_called);
            drop_called = true;
        });
        assert!(drop_called);
    }

    #[test]
    fn test_new_drop_many() {
        let chain = ChunkChain::new(
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(10 * AValueHeader::ALIGN)),
            ChunkChain::default(),
        );
        let chain = ChunkChain::new(
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(10 * AValueHeader::ALIGN)),
            chain,
        );
        let mut chain = ChunkChain::new(
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(10 * AValueHeader::ALIGN)),
            chain,
        );
        let mut drop_count = 0;
        chain.clear_with(&mut |_| {
            drop_count += 1;
        });
        assert_eq!(3, drop_count);
    }

    #[test]
    fn test_split_at() {
        let chunk_part =
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(20 * AValueHeader::ALIGN));
        let chunk_len = chunk_part.len();
        let chain = ChunkChain::new(chunk_part, ChunkChain::default());

        let (new_chain, chunk) = chain.split_at(AlignedSize::new_bytes(3 * AValueHeader::ALIGN));
        assert_eq!(
            AlignedSize::new_bytes(3 * AValueHeader::ALIGN),
            new_chain.current_chunk_available_len()
        );
        assert_eq!(
            chunk_len - AlignedSize::new_bytes(3 * AValueHeader::ALIGN) - ChunkChain::HEADER_SIZE,
            chunk.len()
        );

        // After split, the chain first chunk should be the same as split off chunk.
        assert!(new_chain.chunk.as_ref().unwrap().chunk_ptr_eq(&chunk));
        assert_eq!(2, chunk.chunk_ref_count());
    }

    #[test]
    fn test_split_at_len() {
        let chunk_part =
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(20 * AValueHeader::ALIGN));
        let chain = ChunkChain::new(chunk_part, ChunkChain::default());
        let chain_len = chain.current_chunk_available_len();

        let (new_chain, rem) = chain.split_at(chain_len);
        assert_eq!(chain_len, new_chain.current_chunk_available_len());
        assert_eq!(AlignedSize::ZERO, rem.len());

        assert_eq!(0, rem.chunk_ref_count(), "statically allocated empty chunk");
        assert_eq!(1, new_chain.chunk.as_ref().unwrap().chunk_ref_count());
    }

    #[test]
    fn test_split_at_zero() {
        struct ResetSplitAtZeroTest;
        impl Drop for ResetSplitAtZeroTest {
            fn drop(&mut self) {
                assert!(SPLIT_AT_ZERO_TEST.get());
                SPLIT_AT_ZERO_TEST.set(false);
            }
        }
        assert!(!SPLIT_AT_ZERO_TEST.get());
        SPLIT_AT_ZERO_TEST.set(true);

        let _reset = ResetSplitAtZeroTest;

        let chunk_part =
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(20 * AValueHeader::ALIGN));
        let chain = ChunkChain::new(chunk_part, ChunkChain::default());
        let chain_len = chain.current_chunk_available_len();
        let (new_chain, rem) = chain.split_at(AlignedSize::ZERO);
        assert!(
            new_chain.chunk.is_none(),
            "Should be replaced with underlying chain"
        );
        assert_eq!(chain_len, rem.len());
    }

    #[test]
    fn test_depth() {
        let chain = ChunkChain::default();
        assert_eq!(0, chain.depth());

        let chain = ChunkChain::new(
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(10 * AValueHeader::ALIGN)),
            chain,
        );
        assert_eq!(1, chain.depth());

        let chain = ChunkChain::new(
            ChunkPart::alloc_at_least(AlignedSize::new_bytes(20 * AValueHeader::ALIGN)),
            chain,
        );
        assert_eq!(2, chain.depth());
    }
}
