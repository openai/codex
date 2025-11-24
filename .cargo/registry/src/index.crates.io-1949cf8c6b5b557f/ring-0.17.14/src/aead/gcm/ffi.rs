// Copyright 2018 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use crate::{
    bb,
    polyfill::{slice::AsChunks, ArraySplitMap},
};

pub(in super::super) const BLOCK_LEN: usize = 16;
pub(in super::super) type Block = [u8; BLOCK_LEN];
pub(super) const ZERO_BLOCK: Block = [0u8; BLOCK_LEN];

#[cfg(any(
    all(target_arch = "aarch64", target_endian = "little"),
    all(target_arch = "arm", target_endian = "little"),
    target_arch = "x86",
    target_arch = "x86_64"
))]
macro_rules! htable_new {
    ( $name:ident, $value:expr $(,)? ) => {{
        use crate::aead::gcm::ffi::HTable;
        prefixed_extern! {
            fn $name(HTable: &mut HTable, h: &[u64; 2]);
        }
        HTable::new($name, $value)
    }};
}

/// SAFETY:
///  * The function `$name` must meet the contract of the `f` paramweter of
///    `ghash()`.
#[cfg(any(
    all(target_arch = "aarch64", target_endian = "little"),
    all(target_arch = "arm", target_endian = "little"),
    target_arch = "x86",
    target_arch = "x86_64"
))]
macro_rules! ghash {
    ( $name:ident, $xi:expr, $h_table:expr, $input:expr $(,)? ) => {{
        use crate::aead::gcm::ffi::{HTable, Xi};
        prefixed_extern! {
            fn $name(
                xi: &mut Xi,
                Htable: &HTable,
                inp: *const u8,
                len: crate::c::NonZero_size_t,
            );
        }
        $h_table.ghash($name, $xi, $input)
    }};
}

pub(in super::super) struct KeyValue([u64; 2]);

impl KeyValue {
    pub(in super::super) fn new(value: Block) -> Self {
        Self(value.array_split_map(u64::from_be_bytes))
    }

    pub(super) fn into_inner(self) -> [u64; 2] {
        self.0
    }
}

/// SAFETY:
///   * `f` must read `len` bytes from `inp`; it may assume
///     that `len` is a (non-zero) multiple of `BLOCK_LEN`.
///   * `f` may inspect CPU features.
#[cfg(any(
    all(target_arch = "aarch64", target_endian = "little"),
    all(target_arch = "arm", target_endian = "little"),
    target_arch = "x86",
    target_arch = "x86_64"
))]
impl HTable {
    pub(super) unsafe fn new(
        init: unsafe extern "C" fn(HTable: &mut HTable, &[u64; 2]),
        value: KeyValue,
    ) -> Self {
        let mut r = Self {
            Htable: [U128 { hi: 0, lo: 0 }; HTABLE_LEN],
        };
        unsafe { init(&mut r, &value.0) };
        r
    }

    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        all(target_arch = "arm", target_endian = "little")
    ))]
    pub(super) unsafe fn gmult(
        &self,
        f: unsafe extern "C" fn(xi: &mut Xi, h_table: &HTable),
        xi: &mut Xi,
    ) {
        unsafe { f(xi, self) }
    }

    pub(super) unsafe fn ghash(
        &self,
        f: unsafe extern "C" fn(
            xi: &mut Xi,
            Htable: &HTable,
            inp: *const u8,
            len: crate::c::NonZero_size_t,
        ),
        xi: &mut Xi,
        input: AsChunks<u8, BLOCK_LEN>,
    ) {
        use core::num::NonZeroUsize;

        let input = input.as_flattened();

        let input_len = match NonZeroUsize::new(input.len()) {
            Some(len) => len,
            None => {
                return;
            }
        };

        // SAFETY:
        //  * There are `input_len: NonZeroUsize` bytes available at `input` for
        //    `f` to read.
        unsafe {
            f(xi, self, input.as_ptr(), input_len);
        }
    }
}

// The alignment is required by some assembly code, such as `ghash-ssse3-*`.
#[derive(Clone)]
#[repr(C, align(16))]
pub(in super::super) struct HTable {
    Htable: [U128; HTABLE_LEN],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(super) struct U128 {
    pub(super) hi: u64,
    pub(super) lo: u64,
}

const HTABLE_LEN: usize = 16;

#[repr(transparent)]
pub(in super::super) struct Xi(pub(super) Block);

impl Xi {
    #[inline]
    pub(super) fn bitxor_assign(&mut self, a: Block) {
        self.0 = bb::xor_16(self.0, a)
    }
}
