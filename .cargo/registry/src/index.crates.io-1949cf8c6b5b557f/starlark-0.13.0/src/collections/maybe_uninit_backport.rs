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

// Copy-paste from std.
#![allow(clippy::needless_range_loop)]

use std::mem;
use std::mem::MaybeUninit;
use std::ptr;

/// Copy-paste `MaybeUninit::write_slice_cloned`. Replace back when stabilized.
pub(crate) fn maybe_uninit_write_slice_cloned<'a, T>(
    this: &'a mut [MaybeUninit<T>],
    src: &[T],
) -> &'a mut [T]
where
    T: Clone,
{
    // unlike copy_from_slice this does not call clone_from_slice on the slice
    // this is because `MaybeUninit<T: Clone>` does not implement Clone.

    struct Guard<'a, T> {
        slice: &'a mut [MaybeUninit<T>],
        initialized: usize,
    }

    impl<'a, T> Drop for Guard<'a, T> {
        fn drop(&mut self) {
            let initialized_part = &mut self.slice[..self.initialized];
            // SAFETY: this raw slice will contain only initialized objects
            // that's why, it is allowed to drop it.
            unsafe {
                ptr::drop_in_place(&mut *(initialized_part as *mut [MaybeUninit<T>] as *mut [T]));
            }
        }
    }

    assert_eq!(
        this.len(),
        src.len(),
        "destination and source slices have different lengths"
    );
    // NOTE: We need to explicitly slice them to the same length
    // for bounds checking to be elided, and the optimizer will
    // generate memcpy for simple cases (for example T = u8).
    let len = this.len();
    let src = &src[..len];

    // guard is needed b/c panic might happen during a clone
    let mut guard = Guard {
        slice: this,
        initialized: 0,
    };

    for i in 0..len {
        guard.slice[i].write(src[i].clone());
        guard.initialized += 1;
    }

    mem::forget(guard);

    // SAFETY: Valid elements have just been written into `this` so it is initialized
    unsafe { &mut *(this as *mut [MaybeUninit<T>] as *mut [T]) }
}

/// Copy-paste `MaybeUninit::write_slice`. Replace back when stabilized.
pub(crate) fn maybe_uninit_write_slice<'a, T>(
    this: &'a mut [MaybeUninit<T>],
    src: &[T],
) -> &'a mut [T]
where
    T: Copy,
{
    // SAFETY: &[T] and &[MaybeUninit<T>] have the same layout
    let uninit_src: &[MaybeUninit<T>] = unsafe { mem::transmute(src) };

    this.copy_from_slice(uninit_src);

    // SAFETY: Valid elements have just been copied into `this` so it is initialized
    unsafe { &mut *(this as *mut [MaybeUninit<T>] as *mut [T]) }
}
