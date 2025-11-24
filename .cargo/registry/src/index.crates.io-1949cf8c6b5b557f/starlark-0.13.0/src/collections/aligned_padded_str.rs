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

use std::marker::PhantomData;
use std::mem;

use dupe::Dupe;

/// String which is `usize` aligned with zeros padding in the end.
#[derive(Copy, Clone, Dupe)]
pub(crate) struct AlignedPaddedStr<'a> {
    /// In bytes.
    len: usize,
    /// Data containing `len` bytes and zero padding in the end.
    data: *const usize,
    _marker: PhantomData<&'a str>,
}

impl<'a> AlignedPaddedStr<'a> {
    #[inline]
    pub(crate) unsafe fn new(len: usize, data: *const usize) -> AlignedPaddedStr<'a> {
        AlignedPaddedStr {
            len,
            data,
            _marker: PhantomData,
        }
    }

    /// Len of string in words.
    #[inline]
    fn len_words(self) -> usize {
        (self.len + mem::size_of::<usize>() - 1) / mem::size_of::<usize>()
    }
}

impl<'a> PartialEq for AlignedPaddedStr<'a> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }

        // We know strings are aligned, zero-padded and short,
        // so we can do better than generic SIMD-optimized `memcmp`
        // https://rust.godbolt.org/z/cdscb37Yd
        let len_words = self.len_words();
        for i in 0..len_words {
            if unsafe { *self.data.add(i) != *other.data.add(i) } {
                return false;
            }
        }
        true
    }
}
