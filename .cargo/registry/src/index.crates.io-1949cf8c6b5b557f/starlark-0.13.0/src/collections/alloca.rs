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

use std::alloc::alloc;
use std::alloc::dealloc;
use std::alloc::Layout;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::NonNull;
use std::slice;

use crate::collections::maybe_uninit_backport::maybe_uninit_write_slice_cloned;
use crate::hint::likely;
use crate::hint::unlikely;

/// Holds the allocation.
struct Buffer {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl Buffer {
    fn alloc(layout: Layout) -> Buffer {
        let ptr = unsafe { alloc(layout) };
        let ptr = NonNull::new(ptr).unwrap();
        Buffer { ptr, layout }
    }

    fn ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    fn end(&self) -> *mut u8 {
        unsafe { self.ptr.as_ptr().add(self.layout.size()) }
    }

    fn size_words(&self) -> usize {
        self.layout.size() / ALIGN
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe { dealloc(self.ptr.as_ptr().cast(), self.layout) }
    }
}

/// We'd love to use the real `alloca`, but don't want to blow through the stack space,
/// so define our own wrapper.
/// We use a single continuous buffer. When it needs upgrading, we double it and keep the old one around.
pub(crate) struct Alloca {
    // An alternative design would be to bake the <T> into the type, so all allocations are of the same type.
    // Benchmarking that, even if the most optimistic scenario (just reallocating a single element)
    // the performance difference is only 2%, so keep the flexibility of not needing to predeclare the type.
    alloc: Cell<*mut Align>,
    end: Cell<*mut Align>,
    buffers: RefCell<Vec<Buffer>>,
}

const INITIAL_SIZE: usize = 1000000; // ~ 1Mb

type Align = u64;
const ALIGN: usize = mem::size_of::<Align>();

impl Alloca {
    pub fn new() -> Self {
        Self::with_capacity(INITIAL_SIZE)
    }

    pub fn with_capacity(size_bytes: usize) -> Self {
        let size_words = (size_bytes + ALIGN - 1) / ALIGN;
        let layout = Layout::array::<Align>(size_words).unwrap();
        let buffer = Buffer::alloc(layout);
        Self {
            alloc: Cell::new(buffer.ptr().cast()),
            end: Cell::new(buffer.end().cast()),
            buffers: RefCell::new(vec![buffer]),
        }
    }

    fn assert_state(&self) {
        unsafe {
            debug_assert!(self.end.get().offset_from(self.alloc.get()) >= 0);
            debug_assert!(
                self.end.get().offset_from(self.alloc.get()) as usize
                    <= self.buffers.borrow().last().unwrap().size_words()
            );
        }
    }

    #[cold]
    #[inline(never)]
    fn allocate_more(&self, len: usize, one: Layout) {
        let want =
            Layout::from_size_align(one.size().checked_mul(len).unwrap(), one.align()).unwrap();
        assert!(want.align() <= mem::size_of::<Align>());
        let size_words =
            self.buffers.borrow().last().unwrap().size_words() * 2 + want.size() / ALIGN;
        let layout = Layout::array::<Align>(size_words).unwrap();
        let buffer = Buffer::alloc(layout);
        let pointer = buffer.ptr().cast();
        let end = buffer.end().cast();
        self.buffers.borrow_mut().push(buffer);
        self.alloc.set(pointer);
        self.end.set(end);
    }

    /// Convert remaining capacity in words to remaining capacity in `T`.
    #[inline(always)]
    fn rem_in_words_to_rem_in_t<T>(rem_in_words: usize) -> usize {
        assert!(mem::align_of::<T>() <= ALIGN);
        rem_in_words * ALIGN / mem::size_of::<T>()
    }

    #[inline(always)]
    fn len_in_to_to_len_in_words<T>(len: usize) -> usize {
        if mem::size_of::<T>() % ALIGN == 0 {
            // Special case to make common case fast:
            // https://rust.godbolt.org/z/adh3nzdzs
            len * (mem::size_of::<T>() / ALIGN)
        } else {
            // Multiplication does not overflow because we know that `[T; len]`
            // is within the size of the current buffer, which is smaller than `isize::MAX`.
            (len * mem::size_of::<T>() + ALIGN - 1) / ALIGN
        }
    }

    /// Note that the `Drop` for the `T` will not be called. That's safe if there is no `Drop`,
    /// or you call it yourself.
    #[inline(always)]
    pub fn alloca_uninit<T, R>(&self, len: usize, k: impl FnOnce(&mut [MaybeUninit<T>]) -> R) -> R {
        self.assert_state();

        assert!(mem::align_of::<T>() <= ALIGN);

        let mut start = self.alloc.get();

        let rem_words = unsafe { self.end.get().offset_from(start) as usize };
        let rem_in_t = Self::rem_in_words_to_rem_in_t::<T>(rem_words);
        if unlikely(len > rem_in_t) {
            self.allocate_more(len, Layout::new::<T>());
            start = self.alloc.get();
        }

        let size_words = Self::len_in_to_to_len_in_words::<T>(len);

        let stop = start.wrapping_add(size_words);
        let old = start;
        self.alloc.set(stop);
        let data = start as *mut MaybeUninit<T>;
        let slice = unsafe { slice::from_raw_parts_mut(data, len) };
        let res = k(slice);

        // If the pointer changed, it means a callback called alloca again,
        // which allocated a new buffer. So we are abandoning the current allocation here,
        // and new allocations will use the new buffer even if the current buffer has space.
        if likely(self.alloc.get() == stop) {
            self.alloc.set(old);
        }

        self.assert_state();

        res
    }

    #[inline(always)]
    pub fn alloca_init<T, R>(
        &self,
        len: usize,
        mut init: impl FnMut() -> T,
        k: impl FnOnce(&mut [T]) -> R,
    ) -> R {
        self.alloca_uninit(len, |data| {
            for x in data.iter_mut() {
                x.write(init());
            }
            let data = unsafe { &mut *(data as *mut [MaybeUninit<T>] as *mut [T]) };
            k(data)
        })
    }

    #[allow(dead_code)] // Dead, but morally a sensible API to provide, and useful for testing
    #[inline(always)]
    pub fn alloca_fill<T: Copy, R>(&self, len: usize, fill: T, k: impl FnOnce(&mut [T]) -> R) -> R {
        self.alloca_init(len, || fill, k)
    }

    fn alloca_concat_slow<T: Clone, R, F>(&self, x: &[T], y: &[T], k: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        // We clone the input into a slice allocated in `Alloca`,
        // slice does not invoke `Drop`, so we do drop explicitly.
        struct DropSliceGuard<A>(*mut [A]);
        impl<A> Drop for DropSliceGuard<A> {
            fn drop(&mut self) {
                unsafe {
                    ptr::drop_in_place(self.0);
                }
            }
        }

        self.alloca_uninit(x.len() + y.len(), |xy| {
            let (x_uninit, y_uninit) = xy.split_at_mut(x.len());
            let x = maybe_uninit_write_slice_cloned(x_uninit, x);
            let _x_drop_guard = DropSliceGuard(x);
            let y = maybe_uninit_write_slice_cloned(y_uninit, y);
            let _y_drop_guard = DropSliceGuard(y);
            let xy = unsafe { &mut *(xy as *mut [MaybeUninit<T>] as *mut [T]) };
            k(xy)
        })
    }

    /// Concat two slices and invoke the callback with the result.
    /// Use either slice as is if the other is empty,
    /// otherwise clone the elements into a temporary slice.
    #[inline(always)]
    pub(crate) fn alloca_concat<T: Clone, R, F>(&self, x: &[T], y: &[T], k: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        if x.is_empty() {
            k(y)
        } else if y.is_empty() {
            k(x)
        } else {
            self.alloca_concat_slow(x, y, k)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::identity_op)]
    #[test]
    fn test_rem_in_words_to_rem_in_t() {
        assert_eq!(10, Alloca::rem_in_words_to_rem_in_t::<Align>(10));
        assert_eq!(
            1 * ALIGN / mem::size_of::<u32>(),
            Alloca::rem_in_words_to_rem_in_t::<u32>(1)
        );
        assert_eq!(
            2 * ALIGN / mem::size_of::<u32>(),
            Alloca::rem_in_words_to_rem_in_t::<u32>(2)
        );
        assert_eq!(1, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN - 1]>(1));
        assert_eq!(1, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN]>(1));
        assert_eq!(0, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN + 1]>(1));
        assert_eq!(2, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN - 1]>(2));
        assert_eq!(2, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN]>(2));
        assert_eq!(1, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN + 1]>(2));
    }

    #[test]
    fn test_len_in_t_to_len_in_words() {
        assert_eq!(10, Alloca::len_in_to_to_len_in_words::<Align>(10));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<u8>(1));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<u8>(ALIGN - 1));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<u8>(ALIGN));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<u8>(ALIGN + 1));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<u8>(2 * ALIGN - 1));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<u8>(2 * ALIGN));
        assert_eq!(3, Alloca::len_in_to_to_len_in_words::<u8>(2 * ALIGN + 1));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN - 1]>(1));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN]>(1));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN + 1]>(1));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN - 1]>(2));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN]>(2));
        assert_eq!(3, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN + 1]>(2));
    }

    #[test]
    fn test_alloca() {
        // Use a small capacity to encourage overflow behaviour
        let a = Alloca::with_capacity(100);
        a.alloca_fill(3, 8usize, |xs| {
            xs[0] = 5;
            xs[2] = xs[0] + xs[1] + 15;
            a.alloca_fill(200, 18usize, |ys| {
                assert_eq!(xs[2], 8 + 5 + 15);
                assert_eq!(ys[0], 18);
                assert_eq!(ys[200 - 1], 18);
            });
            a.alloca_fill(3, 1u64, |_| {});
            assert_eq!(xs[2], 8 + 5 + 15);
        })
    }

    #[test]
    fn trigger_bug() {
        let a = Alloca::with_capacity(100);
        for _ in 0..100 {
            a.alloca_fill(10, 17usize, |_| {
                a.alloca_fill(1000, 19usize, |_| {});
            });
        }

        assert_eq!(2, a.buffers.borrow().len());
    }

    #[test]
    fn test_alloca_bug_not_aligned() {
        let a = Alloca::with_capacity(100);
        a.alloca_fill(1, 17u8, |xs| {
            // Bug was triggered because the end of first allocation
            // was rounded down instead of up.
            a.alloca_fill(1, 19u8, |ys| {
                assert_eq!([17], xs);
                assert_eq!([19], ys);
            });
        })
    }

    #[test]
    fn test_alloca_concat() {
        let a = Alloca::new();
        let x = vec!["ab".to_owned()];
        let y = vec!["cd".to_owned()];
        a.alloca_concat(&x, &[], |xy| {
            // No copy here.
            assert_eq!(xy.as_ptr(), x.as_ptr());
        });
        a.alloca_concat(&[], &x, |xy| {
            // No copy here.
            assert_eq!(xy.as_ptr(), x.as_ptr());
        });
        a.alloca_concat(&x, &y, |xy| {
            assert_eq!(&["ab".to_owned(), "cd".to_owned()], xy);
        });
    }
}
