// Copyright 2021 Brian Smith.
// Portions Copyright (c) 2014, Google Inc.
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
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE. */
// Adapted from the public domain, estream code by D. Bernstein.
// Adapted from the BoringSSL crypto/chacha/chacha.c.

use super::{super::overlapping::IndexError, Counter, Key, Overlapping, BLOCK_LEN};
use crate::{bb, polyfill::sliceutil};
use core::mem::size_of;

pub(super) fn ChaCha20_ctr32(key: &Key, counter: Counter, mut in_out: Overlapping<'_>) {
    const SIGMA: [u32; 4] = [
        u32::from_le_bytes(*b"expa"),
        u32::from_le_bytes(*b"nd 3"),
        u32::from_le_bytes(*b"2-by"),
        u32::from_le_bytes(*b"te k"),
    ];

    let key = key.words_less_safe();
    let counter = counter.into_words_less_safe();

    let mut state = [
        SIGMA[0], SIGMA[1], SIGMA[2], SIGMA[3], key[0], key[1], key[2], key[3], key[4], key[5],
        key[6], key[7], counter[0], counter[1], counter[2], counter[3],
    ];

    let mut in_out_len = in_out.len();

    let mut buf = [0u8; BLOCK_LEN];
    while in_out_len > 0 {
        chacha_core(&mut buf, &state);
        state[12] += 1;

        debug_assert_eq!(in_out_len, in_out.len());

        // Both branches do the same thing, but the duplication helps the
        // compiler optimize (vectorize) the `BLOCK_LEN` case.
        if in_out_len >= BLOCK_LEN {
            in_out = in_out
                .split_first_chunk::<BLOCK_LEN>(|in_out| {
                    bb::xor_assign_at_start(&mut buf, in_out.input());
                    sliceutil::overwrite_at_start(in_out.into_unwritten_output(), &buf);
                })
                .unwrap_or_else(|IndexError { .. }| {
                    // Since `in_out_len == in_out.len() && in_out_len >= BLOCK_LEN`.
                    unreachable!()
                });
        } else {
            bb::xor_assign_at_start(&mut buf, in_out.input());
            sliceutil::overwrite_at_start(in_out.into_unwritten_output(), &buf);
            break;
        }

        in_out_len -= BLOCK_LEN;
    }
}

// Performs 20 rounds of ChaCha on `input`, storing the result in `output`.
#[inline(always)]
fn chacha_core(output: &mut [u8; BLOCK_LEN], input: &State) {
    let mut x = *input;

    for _ in (0..20).step_by(2) {
        quarterround(&mut x, 0, 4, 8, 12);
        quarterround(&mut x, 1, 5, 9, 13);
        quarterround(&mut x, 2, 6, 10, 14);
        quarterround(&mut x, 3, 7, 11, 15);
        quarterround(&mut x, 0, 5, 10, 15);
        quarterround(&mut x, 1, 6, 11, 12);
        quarterround(&mut x, 2, 7, 8, 13);
        quarterround(&mut x, 3, 4, 9, 14);
    }

    for (x, input) in x.iter_mut().zip(input.iter()) {
        *x = x.wrapping_add(*input);
    }

    output
        .chunks_exact_mut(size_of::<u32>())
        .zip(x.iter())
        .for_each(|(output, &x)| output.copy_from_slice(&x.to_le_bytes()));
}

#[inline(always)]
fn quarterround(x: &mut State, a: usize, b: usize, c: usize, d: usize) {
    #[inline(always)]
    fn step(x: &mut State, a: usize, b: usize, c: usize, rotation: u32) {
        x[a] = x[a].wrapping_add(x[b]);
        x[c] = (x[c] ^ x[a]).rotate_left(rotation);
    }
    step(x, a, b, d, 16);
    step(x, c, d, b, 12);
    step(x, a, b, d, 8);
    step(x, c, d, b, 7);
}

type State = [u32; BLOCK_LEN / 4];
