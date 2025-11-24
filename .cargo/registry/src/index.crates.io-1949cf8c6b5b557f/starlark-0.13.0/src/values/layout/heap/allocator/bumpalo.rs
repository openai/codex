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

use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::slice;

use bumpalo::Bump;
use bumpalo::ChunkRawIter;

use crate::values::layout::heap::allocator::api::ArenaAllocator;
use crate::values::layout::heap::allocator::api::ChunkAllocationDirection;
use crate::values::layout::value_alloc_size::ValueAllocSize;

pub(crate) struct ChunkIteratorWrapper<'a> {
    iter: ChunkRawIter<'a>,
}

impl<'a> Iterator for ChunkIteratorWrapper<'a> {
    type Item = &'a [MaybeUninit<u8>];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|(ptr, len)| unsafe { slice::from_raw_parts(ptr as *mut MaybeUninit<u8>, len) })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl ArenaAllocator for Bump {
    fn allocated_bytes(&self) -> usize {
        Bump::allocated_bytes(self)
    }

    fn remaining_capacity(&self) -> usize {
        Bump::chunk_capacity(self)
    }

    fn allocation_overhead(&self) -> usize {
        // Need this to estimate correctly:
        // https://github.com/fitzgen/bumpalo/pull/184
        // For now we can either ignore it or hardcode the bumpalo footer size.
        0
    }

    #[inline]
    fn alloc(&self, size: ValueAllocSize) -> NonNull<u8> {
        Bump::alloc_layout(self, size.layout())
    }

    const CHUNK_ALLOCATION_DIRECTION: ChunkAllocationDirection = ChunkAllocationDirection::Down;

    type ChunkRevIterator<'a> = ChunkIteratorWrapper<'a>;

    unsafe fn iter_allocated_chunks_rev(&self) -> Self::ChunkRevIterator<'_> {
        ChunkIteratorWrapper {
            iter: Bump::iter_allocated_chunks_raw(self),
        }
    }

    fn finish(&mut self) {}
}
