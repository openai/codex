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

use std::cell::RefCell;
use std::cmp;
use std::mem;

use crate::values::layout::aligned_size::AlignedSize;
use crate::values::layout::heap::allocator::alloc::chain::ChunkChain;
use crate::values::layout::heap::allocator::alloc::chunk::Chunk;
use crate::values::layout::heap::allocator::alloc::chunk_part::ChunkPart;
use crate::values::layout::heap::arena::MIN_ALLOC;

/// Minimum usable cached allocation.
const MIN_USABLE_ALLOC: AlignedSize = AlignedSize::new_bytes(
    (
        // * All chunks are used to store chains, so we need to include chain header.
        // * We need to be able to store at least one object after the header.
        ChunkChain::HEADER_SIZE.bytes() + MIN_ALLOC.bytes()
    ) as usize,
);

#[derive(Debug, Default)]
struct PerThreadChunkCache {
    /// Keep a few last chunks.
    ///
    /// Frozen heap has two arenas: drop and non-drop. So we should keep at least two chunks.
    last_chunks: [ChunkPart; 4],
}

impl PerThreadChunkCache {
    /// Save a chunk to the thread-local cache if it is large enough.
    fn store(&mut self, mut chunk: ChunkPart) {
        for next in &mut self.last_chunks {
            // Keep the largest chunks in the pool.
            if chunk.len() > next.len() {
                mem::swap(next, &mut chunk);
            }
        }
    }

    /// Fetch a chunk from the thread-local cache if the cache has a chunk large enough.
    fn fetch(&mut self, len: AlignedSize) -> Option<ChunkPart> {
        for next in &mut self.last_chunks {
            // Pick any chunk which is large enough.
            if next.len() >= len {
                let result = mem::take(next);
                return Some(result);
            }
        }
        None
    }
}

thread_local! {
    static PER_THREAD_ALLOCATOR: RefCell<PerThreadChunkCache> = RefCell::new(PerThreadChunkCache::default());
}

fn next_chunk_size(chunk_count_in_bump: usize) -> AlignedSize {
    // Replicate `bumpalo` behavior: 512 in the first chunk, double each next,
    // but not greater than 2G.
    // TODO(nga): we should stop doubling after 1M or so.
    let size = AlignedSize::new_bytes(
        512u32
            .checked_shl(chunk_count_in_bump.try_into().unwrap())
            .unwrap() as usize,
    );
    if size.bytes() == 0 {
        AlignedSize::new_bytes(1 << 31)
    } else {
        size
    }
}

/// Allocate chunk which is large enough for given number of words.
pub(crate) fn thread_local_alloc_at_least(
    len: AlignedSize,
    chunk_count_in_bump: usize,
) -> ChunkPart {
    let chunk = if let Some(chunk) =
        PER_THREAD_ALLOCATOR.with_borrow_mut(|allocator| allocator.fetch(len))
    {
        chunk
    } else {
        let next_chunk_size = next_chunk_size(chunk_count_in_bump) - Chunk::HEADER_SIZE;
        let len = cmp::max(len, next_chunk_size);
        ChunkPart::alloc_at_least(len)
    };
    debug_assert!(chunk.len() >= len);
    chunk
}

/// Release chunk part to thread-local pool.
#[allow(clippy::if_same_then_else)]
#[inline]
pub(crate) fn thread_local_release(chunk: ChunkPart) {
    if chunk.is_full() {
        // Chunk part is the full chunk. Better return it to malloc.
        drop(chunk)
    } else if chunk.len() < MIN_USABLE_ALLOC {
        // It is not reusable.
        drop(chunk)
    } else if chunk.chunk_ref_count() == 1 {
        // We could reuse the chunk, but since it is not shared,
        // better return it to malloc.
        drop(chunk);
    } else {
        PER_THREAD_ALLOCATOR.with_borrow_mut(|allocator| allocator.store(chunk));
    }
}

#[cfg(test)]
mod tests {
    use crate::values::layout::aligned_size::AlignedSize;
    use crate::values::layout::heap::allocator::alloc::chunk_part::ChunkPart;
    use crate::values::layout::heap::allocator::alloc::per_thread::PerThreadChunkCache;
    use crate::values::layout::heap::repr::AValueHeader;

    #[test]
    fn test_release_partial() {
        let mut allocator = PerThreadChunkCache::default();
        let chunk = ChunkPart::alloc_at_least(AlignedSize::new_bytes(10 * AValueHeader::ALIGN));
        let (a, b) = chunk.split_at_offset(AlignedSize::new_bytes(5 * AValueHeader::ALIGN));
        let old_a_ptr = a.begin().as_ptr();
        let old_b_ptr = b.begin().as_ptr();
        allocator.store(a);
        allocator.store(b);
        let a = allocator
            .fetch(AlignedSize::new_bytes(3 * AValueHeader::ALIGN))
            .unwrap();
        let b = allocator
            .fetch(AlignedSize::new_bytes(3 * AValueHeader::ALIGN))
            .unwrap();
        assert!(old_a_ptr == a.begin().as_ptr() || old_a_ptr == b.begin().as_ptr());
        assert!(old_b_ptr == a.begin().as_ptr() || old_b_ptr == b.begin().as_ptr());
    }
}
