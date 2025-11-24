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

use std::ptr::NonNull;

use dupe::Dupe;

use crate::values::layout::aligned_size::AlignedSize;
use crate::values::layout::heap::allocator::alloc::chunk::Chunk;

/// Chunk is shared by multiple `ChunkPart`s.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct ChunkPart {
    allocation: Chunk,
    /// Offset from the chunk data.
    begin: AlignedSize,
    /// Offset from the chunk data.
    end: AlignedSize,
}

impl ChunkPart {
    /// Create a chunk part from a whole chunk.
    #[inline]
    pub(crate) fn new(allocation: Chunk) -> ChunkPart {
        let len = allocation.len();
        ChunkPart::new_subslice(allocation, AlignedSize::ZERO, len)
    }

    #[inline]
    pub(crate) fn new_subslice(
        allocation: Chunk,
        begin: AlignedSize,
        end: AlignedSize,
    ) -> ChunkPart {
        assert!(begin <= end);
        assert!(end <= allocation.len());
        ChunkPart {
            allocation,
            begin,
            end,
        }
    }

    /// Allocate a chunk part to store at least `len`.
    #[inline]
    pub(crate) fn alloc_at_least(len: AlignedSize) -> ChunkPart {
        ChunkPart::new(Chunk::alloc_at_least(len))
    }

    #[inline]
    pub(crate) fn len(&self) -> AlignedSize {
        self.end.unchecked_sub(self.begin)
    }

    #[inline]
    pub(crate) fn begin(&self) -> NonNull<usize> {
        self.allocation.ptr_at_offset(self.begin)
    }

    #[inline]
    pub(crate) fn ptr_at_offset(&self, offset: AlignedSize) -> NonNull<usize> {
        debug_assert!(offset <= self.len());
        self.allocation.ptr_at_offset(self.begin + offset)
    }

    #[inline]
    pub(crate) fn end(&self) -> NonNull<usize> {
        self.allocation.ptr_at_offset(self.end)
    }

    pub(crate) fn allocated_bytes_with_metadata(&self) -> usize {
        if self.chunk_ref_count() == 1 {
            self.allocation.allocated_bytes_with_metadata()
        } else {
            // We cannot know for sure, so try the best to estimate.
            (self.len().bytes() + Chunk::HEADER_SIZE.bytes() / self.chunk_ref_count()) as usize
        }
    }

    /// Does this chunk part occupy the whole chunk?
    #[inline]
    pub(crate) fn is_full(&self) -> bool {
        self.len() == self.allocation.len()
    }

    pub(crate) fn split_at_offset(self, offset: AlignedSize) -> (ChunkPart, ChunkPart) {
        if offset == AlignedSize::ZERO {
            (ChunkPart::default(), self)
        } else if offset == self.len() {
            (self, ChunkPart::default())
        } else {
            assert!(offset <= self.len());
            let offset_relative_to_chunk = self.begin + offset;
            (
                ChunkPart::new_subslice(
                    self.allocation.dupe(),
                    self.begin,
                    offset_relative_to_chunk,
                ),
                ChunkPart::new_subslice(self.allocation, offset_relative_to_chunk, self.end),
            )
        }
    }

    #[inline]
    pub(crate) fn chunk_ref_count(&self) -> u32 {
        self.allocation.ref_count()
    }

    #[cfg(test)]
    pub(crate) fn chunk_ptr_eq(&self, other: &ChunkPart) -> bool {
        self.allocation.ptr_eq(&other.allocation)
    }
}

#[cfg(test)]
mod tests {
    use crate::values::layout::aligned_size::AlignedSize;
    use crate::values::layout::heap::allocator::alloc::chunk::Chunk;
    use crate::values::layout::heap::allocator::alloc::chunk_part::ChunkPart;
    use crate::values::layout::heap::repr::AValueHeader;

    #[test]
    fn test_split_at() {
        let chunk_part = ChunkPart::new(Chunk::alloc_at_least(AlignedSize::new_bytes(
            100 * AValueHeader::ALIGN,
        )));
        let (a, b) = chunk_part.split_at_offset(AlignedSize::new_bytes(50 * AValueHeader::ALIGN));
        assert_eq!(a.allocation, b.allocation);
        assert_eq!(2, a.allocation.ref_count());
        assert_eq!(a.end, b.begin);
        drop(a);
        assert_eq!(1, b.allocation.ref_count());
    }

    #[test]
    fn test_split_at_zero() {
        let chunk_part = ChunkPart::new(Chunk::alloc_at_least(AlignedSize::new_bytes(
            100 * AValueHeader::ALIGN,
        )));
        let len = chunk_part.len();
        let (a, b) = chunk_part.split_at_offset(AlignedSize::ZERO);
        assert_eq!(AlignedSize::ZERO, a.len());
        assert_eq!(0, a.chunk_ref_count());
        assert_eq!(len, b.len());
        assert_eq!(1, b.chunk_ref_count());
    }

    #[test]
    fn test_is_full() {
        let chunk_part = ChunkPart::new(Chunk::alloc_at_least(AlignedSize::new_bytes(
            100 * AValueHeader::ALIGN,
        )));
        assert!(chunk_part.is_full());
    }
}
