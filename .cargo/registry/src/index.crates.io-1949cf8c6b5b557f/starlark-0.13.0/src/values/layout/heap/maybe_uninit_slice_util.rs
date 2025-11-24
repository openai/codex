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
use std::slice;

/// Populate a slice with values from iterator, handle panic by writing provided fallback value.
///
/// buck2 is compiled without unwinding, but other users of starlark-rust may.
/// It's hard to say how dangerous is to leave for example a list containing
/// uninitialized memory in heap in case of panic. So let's just initialize
/// the slice with valid values on panic.
#[inline]
pub(crate) fn maybe_uninit_write_from_exact_size_iter<T: Copy>(
    slice: &mut [MaybeUninit<T>],
    iter: impl IntoIterator<Item = T>,
    write_on_panic: T,
) {
    /// On drop, populate the iterator with the provided value.
    struct WriteRemOnDrop<'a, T: Copy>(slice::IterMut<'a, MaybeUninit<T>>, T);

    impl<'a, T: Copy> Drop for WriteRemOnDrop<'a, T> {
        fn drop(&mut self) {
            for elem in &mut self.0 {
                elem.write(self.1);
            }
        }
    }

    let mut slice = WriteRemOnDrop(slice.iter_mut(), write_on_panic);
    let mut iter = iter.into_iter();

    for (elem_place, elem) in slice.0.by_ref().zip(iter.by_ref()) {
        elem_place.write(elem);
    }

    assert!(
        slice.0.next().is_none() && iter.next().is_none(),
        "iterator provided size_hint incorrectly",
    );

    // Should live to this point.
    drop(slice);
}
