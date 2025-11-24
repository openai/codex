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

use std::fmt;
use std::fmt::Debug;
use std::intrinsics::copy_nonoverlapping;
use std::mem;
use std::slice;
use std::str;

use allocative::Allocative;
use starlark_derive::Trace;
use starlark_map::Hashed;
use starlark_map::StarlarkHashValue;

use crate as starlark;
use crate::coerce::Coerce;
use crate::collections::aligned_padded_str::AlignedPaddedStr;

/// A pre-hashed string used for efficient dictionary lookup.
#[derive(Clone, Trace, Allocative)]
pub(crate) struct Symbol {
    hash: u64,
    len: u32,
    payload: Box<[usize]>,
    small_hash: StarlarkHashValue,
}

unsafe impl Coerce<Symbol> for Symbol {}

impl Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }

        let p1 = &*self.payload;
        let p2 = &*other.payload;
        // Important to use the payload len, which is in u64 units, rather than len which is in u8
        for i in 0..self.payload.len() {
            // Safe because we checked the lengths at the start
            if unsafe { p1.get_unchecked(i) != p2.get_unchecked(i) } {
                return false;
            }
        }
        true
    }
}

impl Eq for Symbol {}

impl Symbol {
    pub(crate) fn new(x: &str) -> Self {
        Self::new_hashed(Hashed::new(x))
    }

    pub(crate) fn new_hashed(x: Hashed<&str>) -> Self {
        let small_hash = x.hash();
        let hash = small_hash.promote();
        let len = x.key().len();
        let len_words = (len + mem::size_of::<usize>() - 1) / mem::size_of::<usize>();
        let mut payload = vec![0; len_words]; // 0 pad it at the end
        unsafe {
            copy_nonoverlapping(x.key().as_ptr(), payload.as_mut_ptr() as *mut u8, len);
        }
        Self {
            hash,
            len: len.try_into().unwrap(),
            payload: payload.into_boxed_slice(),
            small_hash,
        }
    }

    #[inline]
    pub(crate) fn hash(&self) -> u64 {
        self.hash
    }

    pub(crate) fn as_str(&self) -> &str {
        // All safe because we promise we started out with a str
        unsafe {
            let s = slice::from_raw_parts(self.payload.as_ptr() as *const u8, self.len as usize);
            str::from_utf8_unchecked(s)
        }
    }

    #[inline]
    pub(crate) fn as_aligned_padded_str(&self) -> AlignedPaddedStr {
        unsafe { AlignedPaddedStr::new(self.len as usize, self.payload.as_ptr()) }
    }

    pub(crate) fn as_str_hashed(&self) -> Hashed<&str> {
        Hashed::new_unchecked(self.small_hash, self.as_str())
    }

    pub(crate) fn small_hash(&self) -> StarlarkHashValue {
        self.small_hash
    }
}
