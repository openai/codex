// Copyright 2018-2025 Brian Smith.
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

#![cfg(target_arch = "x86_64")]

use super::{ffi::KeyValue, HTable, UpdateBlock, Xi};
use crate::{
    aead::gcm::ffi::BLOCK_LEN,
    cpu::intel::{Avx2, VAesClmul},
    polyfill::slice::AsChunks,
};

#[derive(Clone)]
pub struct Key {
    h_table: HTable,
}

impl Key {
    pub(in super::super) fn new(value: KeyValue, _cpu: (Avx2, VAesClmul)) -> Self {
        Self {
            h_table: unsafe { htable_new!(gcm_init_vpclmulqdq_avx2, value) },
        }
    }

    pub(super) fn inner(&self) -> &HTable {
        &self.h_table
    }
}

impl UpdateBlock for Key {
    fn update_block(&self, xi: &mut Xi, a: [u8; BLOCK_LEN]) {
        let input: AsChunks<u8, BLOCK_LEN> = (&a).into();
        unsafe { ghash!(gcm_ghash_vpclmulqdq_avx2_1, xi, &self.h_table, input) }
    }
}
