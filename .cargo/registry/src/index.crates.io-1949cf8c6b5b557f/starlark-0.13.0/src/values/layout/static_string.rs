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

//! Statically allocated strings.

use std::mem;
use std::sync::atomic::AtomicU32;

use crate::values::layout::avalue::VALUE_STR_A_VALUE_PTR;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::string::str_type::StarlarkStr;
use crate::values::string::str_type::StarlarkStrN;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;

/// A constant string that can be converted to a [`FrozenValue`].
///
/// **Note** `N` is length in words, not length in bytes.
#[repr(C)] // Must match this layout on the heap
pub struct StarlarkStrNRepr<const N: usize> {
    repr: AValueRepr<StarlarkStrN<N>>,
}

impl<const N: usize> StarlarkStrNRepr<N> {
    /// Create a new [`StarlarkStrNRepr`] given a string of length greater than 1.
    /// `N` must be the length of the string in words, otherwise this function will fail.
    ///
    /// This function is used internally, use `const_frozen_string` macro
    /// to statically allocate strings.
    pub const fn new(s: &str) -> Self {
        assert!(
            s.len() > 1,
            "static strings of length <= 1 cannot be created from outside of the crate"
        );
        Self::new_unchecked(s)
    }

    const fn new_unchecked(s: &str) -> Self {
        assert!(N == StarlarkStr::payload_len_for_len(s.len()));
        assert!(s.len() as u32 as usize == s.len());
        let mut payload = [0usize; N];

        // This can be as simple as:
        // ```
        // unsafe { ptr::copy_nonoverlapping(s.as_ptr(), payload.as_mut_ptr() as *mut u8, s.len()) };
        // ```
        // when `const_mut_refs` is stabilized (https://github.com/rust-lang/rust/issues/57349).
        // Anyway, this code is only called at compile time.
        let mut i = 0;
        while i != s.len() {
            payload[i / mem::size_of::<usize>()] |=
                usize::from_le((s.as_bytes()[i] as usize) << (8 * (i % mem::size_of::<usize>())));
            i += 1;
        }

        Self {
            repr: AValueRepr {
                header: VALUE_STR_A_VALUE_PTR,
                payload: StarlarkStrN {
                    len: s.len() as u32,
                    hash: AtomicU32::new(0),
                    body: payload,
                },
            },
        }
    }

    /// Obtain the [`FrozenValue`] for a [`StarlarkStrNRepr`].
    pub fn unpack(&'static self) -> FrozenValue {
        FrozenValue::new_ptr(&self.repr.header, true)
    }

    /// Erase the type parameter, giving a slightly nicer user experience.
    pub fn erase(&'static self) -> FrozenStringValue {
        unsafe { FrozenStringValue::new_unchecked(self.unpack()) }
    }
}

pub(crate) static VALUE_EMPTY_STRING: StarlarkStrNRepr<0> = StarlarkStrNRepr::new_unchecked("");

#[doc(hidden)] // Use `const_frozen_string!` macro instead.
#[inline(always)]
pub fn constant_string(x: &str) -> Option<FrozenStringValue> {
    if x.len() > 1 {
        None
    } else if x.is_empty() {
        Some(VALUE_EMPTY_STRING.erase())
    } else {
        // If the string is 1 byte long there can only be up to the first 128 characters present
        // therefore this index will be total
        Some(VALUE_BYTE_STRINGS[x.as_bytes()[0] as usize].erase())
    }
}

pub(crate) static VALUE_BYTE_STRINGS: [StarlarkStrNRepr<1>; 128] = [
    StarlarkStrNRepr::new_unchecked("\x00"),
    StarlarkStrNRepr::new_unchecked("\x01"),
    StarlarkStrNRepr::new_unchecked("\x02"),
    StarlarkStrNRepr::new_unchecked("\x03"),
    StarlarkStrNRepr::new_unchecked("\x04"),
    StarlarkStrNRepr::new_unchecked("\x05"),
    StarlarkStrNRepr::new_unchecked("\x06"),
    StarlarkStrNRepr::new_unchecked("\x07"),
    StarlarkStrNRepr::new_unchecked("\x08"),
    StarlarkStrNRepr::new_unchecked("\x09"),
    StarlarkStrNRepr::new_unchecked("\x0A"),
    StarlarkStrNRepr::new_unchecked("\x0B"),
    StarlarkStrNRepr::new_unchecked("\x0C"),
    StarlarkStrNRepr::new_unchecked("\x0D"),
    StarlarkStrNRepr::new_unchecked("\x0E"),
    StarlarkStrNRepr::new_unchecked("\x0F"),
    StarlarkStrNRepr::new_unchecked("\x10"),
    StarlarkStrNRepr::new_unchecked("\x11"),
    StarlarkStrNRepr::new_unchecked("\x12"),
    StarlarkStrNRepr::new_unchecked("\x13"),
    StarlarkStrNRepr::new_unchecked("\x14"),
    StarlarkStrNRepr::new_unchecked("\x15"),
    StarlarkStrNRepr::new_unchecked("\x16"),
    StarlarkStrNRepr::new_unchecked("\x17"),
    StarlarkStrNRepr::new_unchecked("\x18"),
    StarlarkStrNRepr::new_unchecked("\x19"),
    StarlarkStrNRepr::new_unchecked("\x1A"),
    StarlarkStrNRepr::new_unchecked("\x1B"),
    StarlarkStrNRepr::new_unchecked("\x1C"),
    StarlarkStrNRepr::new_unchecked("\x1D"),
    StarlarkStrNRepr::new_unchecked("\x1E"),
    StarlarkStrNRepr::new_unchecked("\x1F"),
    StarlarkStrNRepr::new_unchecked("\x20"),
    StarlarkStrNRepr::new_unchecked("\x21"),
    StarlarkStrNRepr::new_unchecked("\x22"),
    StarlarkStrNRepr::new_unchecked("\x23"),
    StarlarkStrNRepr::new_unchecked("\x24"),
    StarlarkStrNRepr::new_unchecked("\x25"),
    StarlarkStrNRepr::new_unchecked("\x26"),
    StarlarkStrNRepr::new_unchecked("\x27"),
    StarlarkStrNRepr::new_unchecked("\x28"),
    StarlarkStrNRepr::new_unchecked("\x29"),
    StarlarkStrNRepr::new_unchecked("\x2A"),
    StarlarkStrNRepr::new_unchecked("\x2B"),
    StarlarkStrNRepr::new_unchecked("\x2C"),
    StarlarkStrNRepr::new_unchecked("\x2D"),
    StarlarkStrNRepr::new_unchecked("\x2E"),
    StarlarkStrNRepr::new_unchecked("\x2F"),
    StarlarkStrNRepr::new_unchecked("\x30"),
    StarlarkStrNRepr::new_unchecked("\x31"),
    StarlarkStrNRepr::new_unchecked("\x32"),
    StarlarkStrNRepr::new_unchecked("\x33"),
    StarlarkStrNRepr::new_unchecked("\x34"),
    StarlarkStrNRepr::new_unchecked("\x35"),
    StarlarkStrNRepr::new_unchecked("\x36"),
    StarlarkStrNRepr::new_unchecked("\x37"),
    StarlarkStrNRepr::new_unchecked("\x38"),
    StarlarkStrNRepr::new_unchecked("\x39"),
    StarlarkStrNRepr::new_unchecked("\x3A"),
    StarlarkStrNRepr::new_unchecked("\x3B"),
    StarlarkStrNRepr::new_unchecked("\x3C"),
    StarlarkStrNRepr::new_unchecked("\x3D"),
    StarlarkStrNRepr::new_unchecked("\x3E"),
    StarlarkStrNRepr::new_unchecked("\x3F"),
    StarlarkStrNRepr::new_unchecked("\x40"),
    StarlarkStrNRepr::new_unchecked("\x41"),
    StarlarkStrNRepr::new_unchecked("\x42"),
    StarlarkStrNRepr::new_unchecked("\x43"),
    StarlarkStrNRepr::new_unchecked("\x44"),
    StarlarkStrNRepr::new_unchecked("\x45"),
    StarlarkStrNRepr::new_unchecked("\x46"),
    StarlarkStrNRepr::new_unchecked("\x47"),
    StarlarkStrNRepr::new_unchecked("\x48"),
    StarlarkStrNRepr::new_unchecked("\x49"),
    StarlarkStrNRepr::new_unchecked("\x4A"),
    StarlarkStrNRepr::new_unchecked("\x4B"),
    StarlarkStrNRepr::new_unchecked("\x4C"),
    StarlarkStrNRepr::new_unchecked("\x4D"),
    StarlarkStrNRepr::new_unchecked("\x4E"),
    StarlarkStrNRepr::new_unchecked("\x4F"),
    StarlarkStrNRepr::new_unchecked("\x50"),
    StarlarkStrNRepr::new_unchecked("\x51"),
    StarlarkStrNRepr::new_unchecked("\x52"),
    StarlarkStrNRepr::new_unchecked("\x53"),
    StarlarkStrNRepr::new_unchecked("\x54"),
    StarlarkStrNRepr::new_unchecked("\x55"),
    StarlarkStrNRepr::new_unchecked("\x56"),
    StarlarkStrNRepr::new_unchecked("\x57"),
    StarlarkStrNRepr::new_unchecked("\x58"),
    StarlarkStrNRepr::new_unchecked("\x59"),
    StarlarkStrNRepr::new_unchecked("\x5A"),
    StarlarkStrNRepr::new_unchecked("\x5B"),
    StarlarkStrNRepr::new_unchecked("\x5C"),
    StarlarkStrNRepr::new_unchecked("\x5D"),
    StarlarkStrNRepr::new_unchecked("\x5E"),
    StarlarkStrNRepr::new_unchecked("\x5F"),
    StarlarkStrNRepr::new_unchecked("\x60"),
    StarlarkStrNRepr::new_unchecked("\x61"),
    StarlarkStrNRepr::new_unchecked("\x62"),
    StarlarkStrNRepr::new_unchecked("\x63"),
    StarlarkStrNRepr::new_unchecked("\x64"),
    StarlarkStrNRepr::new_unchecked("\x65"),
    StarlarkStrNRepr::new_unchecked("\x66"),
    StarlarkStrNRepr::new_unchecked("\x67"),
    StarlarkStrNRepr::new_unchecked("\x68"),
    StarlarkStrNRepr::new_unchecked("\x69"),
    StarlarkStrNRepr::new_unchecked("\x6A"),
    StarlarkStrNRepr::new_unchecked("\x6B"),
    StarlarkStrNRepr::new_unchecked("\x6C"),
    StarlarkStrNRepr::new_unchecked("\x6D"),
    StarlarkStrNRepr::new_unchecked("\x6E"),
    StarlarkStrNRepr::new_unchecked("\x6F"),
    StarlarkStrNRepr::new_unchecked("\x70"),
    StarlarkStrNRepr::new_unchecked("\x71"),
    StarlarkStrNRepr::new_unchecked("\x72"),
    StarlarkStrNRepr::new_unchecked("\x73"),
    StarlarkStrNRepr::new_unchecked("\x74"),
    StarlarkStrNRepr::new_unchecked("\x75"),
    StarlarkStrNRepr::new_unchecked("\x76"),
    StarlarkStrNRepr::new_unchecked("\x77"),
    StarlarkStrNRepr::new_unchecked("\x78"),
    StarlarkStrNRepr::new_unchecked("\x79"),
    StarlarkStrNRepr::new_unchecked("\x7A"),
    StarlarkStrNRepr::new_unchecked("\x7B"),
    StarlarkStrNRepr::new_unchecked("\x7C"),
    StarlarkStrNRepr::new_unchecked("\x7D"),
    StarlarkStrNRepr::new_unchecked("\x7E"),
    StarlarkStrNRepr::new_unchecked("\x7F"),
];
