// Copyright 2015-2025 Brian Smith.
// Copyright 2016 Simon Sapin.
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

use super::{
    sha2::{
        fallback::{ch, maj, Word},
        State32,
    },
    BlockLen, OutputLen,
};
use crate::polyfill::slice::{self, AsChunks};
use core::{mem::size_of, num::Wrapping};

pub(super) const BLOCK_LEN: BlockLen = BlockLen::_512;
pub const CHAINING_LEN: usize = 160 / 8;
pub(super) const OUTPUT_LEN: OutputLen = OutputLen::_160;
const CHAINING_WORDS: usize = CHAINING_LEN / 4;

type W32 = Wrapping<u32>;

// FIPS 180-4 4.1.1
#[inline]
fn parity(x: W32, y: W32, z: W32) -> W32 {
    x ^ y ^ z
}

type State = [W32; CHAINING_WORDS];
const ROUNDS: usize = 80;

pub fn sha1_block_data_order(state: &mut State32, data: AsChunks<u8, { BLOCK_LEN.into() }>) {
    // The unwrap won't fail because `CHAINING_WORDS` is smaller than the
    // length.
    let state: &mut State = (&mut state[..CHAINING_WORDS]).try_into().unwrap();
    // SAFETY: The caller guarantees that this is called with data pointing to `num`
    // `BLOCK_LEN`-long blocks.
    *state = block_data_order(*state, data)
}

#[inline]
#[rustfmt::skip]
fn block_data_order(
    mut H: [W32; CHAINING_WORDS],
    M: AsChunks<u8, { BLOCK_LEN.into() }>,
) -> [W32; CHAINING_WORDS]
{
    for M in M {
        let (M, remainder): (AsChunks<u8, {size_of::<W32>()}>, &[u8]) = slice::as_chunks(M);
        debug_assert!(remainder.is_empty());

        // FIPS 180-4 6.1.2 Step 1
        let mut W: [W32; ROUNDS] = [W32::ZERO; ROUNDS];
        W.iter_mut().zip(M).for_each(|(Wt, Mt)| {
            *Wt = W32::from_be_bytes(*Mt);
        });
        for t in 16..ROUNDS {
            let wt = W[t - 3] ^ W[t - 8] ^ W[t - 14] ^ W[t - 16];
            W[t] = rotl(wt, 1);
        }

        // FIPS 180-4 6.1.2 Step 2
        let [a, b, c, d, e] = H;

        // FIPS 180-4 6.1.2 Step 3 with constants and functions from FIPS 180-4 {4.1.1, 4.2.1}
        let (a, b, c, d, e) = step3(a, b, c, d, e, &W, 0, Wrapping(0x5a827999), ch);
        let (a, b, c, d, e) = step3(a, b, c, d, e, &W, 20, Wrapping(0x6ed9eba1), parity);
        let (a, b, c, d, e) = step3(a, b, c, d, e, &W, 40, Wrapping(0x8f1bbcdc), maj);
        let (a, b, c, d, e) = step3(a, b, c, d, e, &W, 60, Wrapping(0xca62c1d6), parity);

        // FIPS 180-4 6.1.2 Step 4
        H[0] += a;
        H[1] += b;
        H[2] += c;
        H[3] += d;
        H[4] += e;
    }

    H
}

#[inline(always)]
fn step3(
    mut a: W32,
    mut b: W32,
    mut c: W32,
    mut d: W32,
    mut e: W32,
    W: &[W32; 80],
    t: usize,
    k: W32,
    f: impl Fn(W32, W32, W32) -> W32,
) -> (W32, W32, W32, W32, W32) {
    let W = &W[t..(t + 20)];
    for W_t in W.iter() {
        let T = rotl(a, 5) + f(b, c, d) + e + k + W_t;
        e = d;
        d = c;
        c = rotl(b, 30);
        b = a;
        a = T;
    }
    (a, b, c, d, e)
}

#[inline(always)]
fn rotl(x: W32, n: u32) -> W32 {
    Wrapping(x.0.rotate_left(n))
}
