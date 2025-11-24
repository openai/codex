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

use core::slice;
use std::alloc;
use std::alloc::Layout;
use std::fmt;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::NonNull;
use std::sync::atomic;
use std::sync::atomic::AtomicU32;

use dupe::Dupe;

use crate::util::rtabort::rtabort;
use crate::values::layout::aligned_size::AlignedSize;

#[repr(C)]
struct ChunkData {
    ref_count: AtomicU32,
    /// Data length in words. Does not include `ChunkData` header.
    len: AlignedSize,
    data: [MaybeUninit<usize>; 0],
}

/// Identical to `ChunkData`, but does not have `UnsafeCell`, so it is statically allocated.
#[repr(C)]
struct ChunkDataEmpty {
    /// Zero.
    ref_count: u32,
    /// Zero.
    len_words: AlignedSize,
    data: [MaybeUninit<usize>; 0],
}

const _: () = assert!(mem::size_of::<ChunkDataEmpty>() == mem::size_of::<ChunkData>());
const _: () = assert!(mem::align_of::<ChunkDataEmpty>() == mem::align_of::<ChunkData>());
const _: () = assert!(mem::size_of::<ChunkData>() % mem::size_of::<usize>() == 0);

static EMPTY_ALLOC: ChunkDataEmpty = ChunkDataEmpty {
    ref_count: 0,
    len_words: AlignedSize::ZERO,
    data: [],
};

impl fmt::Debug for ChunkData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkData")
            .field("ref_count", &self.ref_count)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl ChunkData {
    fn layout_for_len(len: AlignedSize) -> Layout {
        // We put `ChunkData` in the beginning of the allocated memory.
        let alloc_len = Chunk::HEADER_SIZE + len;
        alloc_len.layout()
    }

    fn alloc_ref_count_1(len: AlignedSize) -> NonNull<ChunkData> {
        assert!(len > AlignedSize::ZERO);

        let layout = Self::layout_for_len(len);
        let ptr = unsafe { alloc::alloc(layout) };
        let ptr = ptr as *mut ChunkData;
        let ptr = match NonNull::new(ptr) {
            None => alloc::handle_alloc_error(layout),
            Some(ptr) => ptr,
        };
        unsafe {
            ptr::write(
                ptr.as_ptr(),
                ChunkData {
                    ref_count: AtomicU32::new(1),
                    len,
                    data: [],
                },
            );
        }
        let data: &mut [MaybeUninit<u8>] = unsafe {
            slice::from_raw_parts_mut(
                ptr.as_ref().begin().cast::<MaybeUninit<u8>>().as_ptr(),
                len.bytes() as usize,
            )
        };
        if cfg!(miri) {
            // Tell Miri that the memory is uninitialized.
            data.fill(MaybeUninit::uninit());
        } else if cfg!(test) {
            // Initialize memory with garbage in tests to catch bugs.
            data.fill(MaybeUninit::new(0x17));
        }
        ptr
    }

    #[inline]
    fn begin(&self) -> NonNull<usize> {
        unsafe { NonNull::new_unchecked(self.data.as_ptr() as *mut usize) }
    }
}

/// Refcounted chunk of memory.
#[derive(PartialEq, Eq)]
pub(crate) struct Chunk {
    ptr: NonNull<ChunkData>,
}

impl Default for Chunk {
    #[inline]
    fn default() -> Chunk {
        // This does not allocate anything, just returns a pointer to the static memory.
        Chunk::alloc_at_least(AlignedSize::ZERO)
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chunk").field("ptr", self.data()).finish()
    }
}

impl Chunk {
    pub(crate) const HEADER_SIZE: AlignedSize = AlignedSize::of::<ChunkData>();

    /// Allocate chunk which can hold at least `len_words` words.
    #[inline]
    pub(crate) fn alloc_at_least(len: AlignedSize) -> Chunk {
        if len == AlignedSize::ZERO {
            Chunk {
                ptr: NonNull::<ChunkDataEmpty>::from(&EMPTY_ALLOC).cast::<ChunkData>(),
            }
        } else {
            Self::alloc_at_least_not_empty(len)
        }
    }

    fn alloc_at_least_not_empty(len: AlignedSize) -> Chunk {
        debug_assert!(len > AlignedSize::ZERO);

        let alloc_len = Chunk::HEADER_SIZE + len;
        // Round up to power of two to avoid spacing in allocation.
        // We don't have to use power of two according to jemalloc docs
        // (https://jemalloc.net/jemalloc.3.html, see "Size classes")
        // but power of two is easier to compute,
        // and it is OK to allocate larger chunk than requested.
        let alloc_len = alloc_len.checked_next_power_of_two().unwrap();
        let len = alloc_len - Chunk::HEADER_SIZE;
        Chunk {
            ptr: ChunkData::alloc_ref_count_1(len),
        }
    }

    #[inline]
    pub(crate) fn ref_count(&self) -> u32 {
        self.data().ref_count.load(atomic::Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn ptr_eq(&self, other: &Chunk) -> bool {
        self.ptr == other.ptr
    }

    #[inline]
    fn data(&self) -> &ChunkData {
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub(crate) fn len(&self) -> AlignedSize {
        self.data().len
    }

    #[inline]
    pub(crate) fn allocated_bytes_with_metadata(&self) -> usize {
        if self.is_empty() {
            // Allocated statically.
            0
        } else {
            ChunkData::layout_for_len(self.len()).size()
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        // Faster than checking the length
        // because it doesn't require dereferencing the pointer.
        ptr::eq(
            self.ptr.as_ptr(),
            &EMPTY_ALLOC as *const ChunkDataEmpty as *mut ChunkData,
        )
    }

    #[inline]
    pub(crate) fn begin(&self) -> NonNull<usize> {
        self.data().begin()
    }

    #[inline]
    pub(crate) fn ptr_at_offset(&self, offset: AlignedSize) -> NonNull<usize> {
        unsafe { NonNull::new_unchecked(self.begin().as_ptr().byte_add(offset.bytes() as usize)) }
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        if self.is_empty() {
            return;
        }

        unsafe {
            if self.data().ref_count.fetch_sub(1, atomic::Ordering::SeqCst) == 1 {
                let layout = ChunkData::layout_for_len(self.data().len);
                alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

impl Clone for Chunk {
    #[inline]
    fn clone(&self) -> Self {
        if self.is_empty() {
            return Chunk::default();
        }

        #[cold]
        fn counter_overflow() -> ! {
            rtabort!("Refcount overflow")
        }

        let prev = self
            .data()
            .ref_count
            .fetch_add(1, atomic::Ordering::Relaxed);
        if prev > i32::MAX as u32 {
            counter_overflow();
        }
        Chunk { ptr: self.ptr }
    }
}

impl Dupe for Chunk {}

#[cfg(test)]
mod tests {
    use crate::values::layout::aligned_size::AlignedSize;
    use crate::values::layout::heap::allocator::alloc::chunk::Chunk;
    use crate::values::layout::heap::repr::AValueHeader;

    #[test]
    fn test_empty() {
        let chunk = Chunk::default();
        assert!(chunk.is_empty());
        assert_eq!(AlignedSize::ZERO, chunk.len());
        assert_eq!(0, chunk.ref_count());
    }

    #[test]
    fn test_alloc_release() {
        let chunk = Chunk::alloc_at_least(AlignedSize::new_bytes(100 * AValueHeader::ALIGN));
        assert_eq!(
            AlignedSize::new_bytes(128 * AValueHeader::ALIGN) - Chunk::HEADER_SIZE,
            chunk.len()
        );
        assert_eq!(1, chunk.ref_count());
        let chunk2 = chunk.clone();
        assert_eq!(2, chunk.ref_count());
        drop(chunk);
        assert_eq!(1, chunk2.ref_count());
    }
}
