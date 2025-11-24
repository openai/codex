// Copyright 2015-2025 Brian Smith.
// Portions Copyright (c) 2014, 2015, Google Inc.
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

#![cfg(all(target_arch = "arm", target_endian = "little"))]

use super::{Key, Tag, KEY_LEN, TAG_LEN};
use crate::{c, cpu::arm::Neon};
use core::num::NonZeroUsize;

// XXX/TODO(MSRV): change to `pub(super)`.
pub(in super::super) struct State {
    state: poly1305_state_st,
    neon: Neon,
}

// TODO: Is 16 enough?
#[repr(C, align(16))]
struct poly1305_state_st {
    r: fe1305x2,
    h: fe1305x2,
    c: fe1305x2,
    precomp: [fe1305x2; 2],

    data: [u8; data_len()],

    buf: [u8; 32],
    buf_used: c::size_t,
    key: [u8; 16],
}

const fn data_len() -> usize {
    128
}

#[derive(Clone, Copy)]
#[repr(C)]
struct fe1305x2 {
    v: [u32; 12], // for alignment; only using 10
}

impl State {
    pub(super) fn new_context(Key { key_and_nonce }: Key, neon: Neon) -> super::Context {
        prefixed_extern! {
            fn CRYPTO_poly1305_init_neon(state: &mut poly1305_state_st, key: &[u8; KEY_LEN]);
        }
        let mut r = Self {
            state: poly1305_state_st {
                r: fe1305x2 { v: [0; 12] },
                h: fe1305x2 { v: [0; 12] },
                c: fe1305x2 { v: [0; 12] },
                precomp: [fe1305x2 { v: [0; 12] }; 2],

                data: [0u8; data_len()],
                buf: Default::default(),
                buf_used: 0,
                key: [0u8; 16],
            },
            neon,
        };
        unsafe { CRYPTO_poly1305_init_neon(&mut r.state, &key_and_nonce) }
        super::Context::ArmNeon(r)
    }

    pub(super) fn update_internal(&mut self, input: &[u8]) {
        prefixed_extern! {
            fn CRYPTO_poly1305_update_neon(
                st: &mut poly1305_state_st,
                input: *const u8,
                in_len: c::NonZero_size_t);
        }
        if let Some(len) = NonZeroUsize::new(input.len()) {
            let _: Neon = self.neon;
            let input = input.as_ptr();
            unsafe { CRYPTO_poly1305_update_neon(&mut self.state, input, len) }
        }
    }

    pub(super) fn finish(mut self) -> Tag {
        prefixed_extern! {
            fn CRYPTO_poly1305_finish_neon(st: &mut poly1305_state_st, mac: &mut [u8; TAG_LEN]);
        }
        let mut tag = Tag([0u8; TAG_LEN]);
        unsafe { CRYPTO_poly1305_finish_neon(&mut self.state, &mut tag.0) }
        tag
    }
}
