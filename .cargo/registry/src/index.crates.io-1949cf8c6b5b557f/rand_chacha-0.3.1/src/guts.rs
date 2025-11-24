// Copyright 2019 The CryptoCorrosion Contributors
// Copyright 2020 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The ChaCha random number generator.

use ppv_lite86::{dispatch, dispatch_light128};

pub use ppv_lite86::Machine;
use ppv_lite86::{vec128_storage, ArithOps, BitOps32, LaneWords4, MultiLane, StoreBytes, Vec4};

pub(crate) const BLOCK: usize = 64;
pub(crate) const BLOCK64: u64 = BLOCK as u64;
const LOG2_BUFBLOCKS: u64 = 2;
const BUFBLOCKS: u64 = 1 << LOG2_BUFBLOCKS;
pub(crate) const BUFSZ64: u64 = BLOCK64 * BUFBLOCKS;
pub(crate) const BUFSZ: usize = BUFSZ64 as usize;

const STREAM_PARAM_NONCE: u32 = 1;
const STREAM_PARAM_BLOCK: u32 = 0;

#[derive(Clone, PartialEq, Eq)]
pub struct ChaCha {
    pub(crate) b: vec128_storage,
    pub(crate) c: vec128_storage,
    pub(crate) d: vec128_storage,
}

#[derive(Clone)]
pub struct State<V> {
    pub(crate) a: V,
    pub(crate) b: V,
    pub(crate) c: V,
    pub(crate) d: V,
}

#[inline(always)]
pub(crate) fn round<V: ArithOps + BitOps32>(mut x: State<V>) -> State<V> {
    x.a += x.b;
    x.d = (x.d ^ x.a).rotate_each_word_right16();
    x.c += x.d;
    x.b = (x.b ^ x.c).rotate_each_word_right20();
    x.a += x.b;
    x.d = (x.d ^ x.a).rotate_each_word_right24();
    x.c += x.d;
    x.b = (x.b ^ x.c).rotate_each_word_right25();
    x
}

#[inline(always)]
pub(crate) fn diagonalize<V: LaneWords4>(mut x: State<V>) -> State<V> {
    x.b = x.b.shuffle_lane_words3012();
    x.c = x.c.shuffle_lane_words2301();
    x.d = x.d.shuffle_lane_words1230();
    x
}
#[inline(always)]
pub(crate) fn undiagonalize<V: LaneWords4>(mut x: State<V>) -> State<V> {
    x.b = x.b.shuffle_lane_words1230();
    x.c = x.c.shuffle_lane_words2301();
    x.d = x.d.shuffle_lane_words3012();
    x
}

impl ChaCha {
    #[inline(always)]
    pub fn new(key: &[u8; 32], nonce: &[u8]) -> Self {
        init_chacha(key, nonce)
    }

    #[inline(always)]
    fn pos64<M: Machine>(&self, m: M) -> u64 {
        let d: M::u32x4 = m.unpack(self.d);
        ((d.extract(1) as u64) << 32) | d.extract(0) as u64
    }

    /// Produce 4 blocks of output, advancing the state
    #[inline(always)]
    pub fn refill4(&mut self, drounds: u32, out: &mut [u8; BUFSZ]) {
        refill_wide(self, drounds, out)
    }

    #[inline(always)]
    pub fn set_block_pos(&mut self, value: u64) {
        set_stream_param(self, STREAM_PARAM_BLOCK, value)
    }

    #[inline(always)]
    pub fn get_block_pos(&self) -> u64 {
        get_stream_param(self, STREAM_PARAM_BLOCK)
    }

    #[inline(always)]
    pub fn set_nonce(&mut self, value: u64) {
        set_stream_param(self, STREAM_PARAM_NONCE, value)
    }

    #[inline(always)]
    pub fn get_nonce(&self) -> u64 {
        get_stream_param(self, STREAM_PARAM_NONCE)
    }

    #[inline(always)]
    pub fn get_seed(&self) -> [u8; 32] {
        get_seed(self)
    }
}

#[allow(clippy::many_single_char_names)]
#[inline(always)]
fn refill_wide_impl<Mach: Machine>(
    m: Mach, state: &mut ChaCha, drounds: u32, out: &mut [u8; BUFSZ],
) {
    let k = m.vec([0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
    let mut pos = state.pos64(m);
    let d0: Mach::u32x4 = m.unpack(state.d);
    pos = pos.wrapping_add(1);
    let d1 = d0.insert((pos >> 32) as u32, 1).insert(pos as u32, 0);
    pos = pos.wrapping_add(1);
    let d2 = d0.insert((pos >> 32) as u32, 1).insert(pos as u32, 0);
    pos = pos.wrapping_add(1);
    let d3 = d0.insert((pos >> 32) as u32, 1).insert(pos as u32, 0);

    let b = m.unpack(state.b);
    let c = m.unpack(state.c);
    let mut x = State {
        a: Mach::u32x4x4::from_lanes([k, k, k, k]),
        b: Mach::u32x4x4::from_lanes([b, b, b, b]),
        c: Mach::u32x4x4::from_lanes([c, c, c, c]),
        d: m.unpack(Mach::u32x4x4::from_lanes([d0, d1, d2, d3]).into()),
    };
    for _ in 0..drounds {
        x = round(x);
        x = undiagonalize(round(diagonalize(x)));
    }
    let mut pos = state.pos64(m);
    let d0: Mach::u32x4 = m.unpack(state.d);
    pos = pos.wrapping_add(1);
    let d1 = d0.insert((pos >> 32) as u32, 1).insert(pos as u32, 0);
    pos = pos.wrapping_add(1);
    let d2 = d0.insert((pos >> 32) as u32, 1).insert(pos as u32, 0);
    pos = pos.wrapping_add(1);
    let d3 = d0.insert((pos >> 32) as u32, 1).insert(pos as u32, 0);
    pos = pos.wrapping_add(1);
    let d4 = d0.insert((pos >> 32) as u32, 1).insert(pos as u32, 0);

    let (a, b, c, d) = (
        x.a.to_lanes(),
        x.b.to_lanes(),
        x.c.to_lanes(),
        x.d.to_lanes(),
    );
    let sb = m.unpack(state.b);
    let sc = m.unpack(state.c);
    let sd = [m.unpack(state.d), d1, d2, d3];
    state.d = d4.into();
    let mut words = out.chunks_exact_mut(16);
    for ((((&a, &b), &c), &d), &sd) in a.iter().zip(&b).zip(&c).zip(&d).zip(&sd) {
        (a + k).write_le(words.next().unwrap());
        (b + sb).write_le(words.next().unwrap());
        (c + sc).write_le(words.next().unwrap());
        (d + sd).write_le(words.next().unwrap());
    }
}

dispatch!(m, Mach, {
    fn refill_wide(state: &mut ChaCha, drounds: u32, out: &mut [u8; BUFSZ]) {
        refill_wide_impl(m, state, drounds, out);
    }
});

// Single-block, rounds-only; shared by try_apply_keystream for tails shorter than BUFSZ
// and XChaCha's setup step.
dispatch!(m, Mach, {
    fn refill_narrow_rounds(state: &mut ChaCha, drounds: u32) -> State<vec128_storage> {
        let k: Mach::u32x4 = m.vec([0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
        let mut x = State {
            a: k,
            b: m.unpack(state.b),
            c: m.unpack(state.c),
            d: m.unpack(state.d),
        };
        for _ in 0..drounds {
            x = round(x);
            x = undiagonalize(round(diagonalize(x)));
        }
        State {
            a: x.a.into(),
            b: x.b.into(),
            c: x.c.into(),
            d: x.d.into(),
        }
    }
});

dispatch_light128!(m, Mach, {
    fn set_stream_param(state: &mut ChaCha, param: u32, value: u64) {
        let d: Mach::u32x4 = m.unpack(state.d);
        state.d = d
            .insert((value >> 32) as u32, (param << 1) | 1)
            .insert(value as u32, param << 1)
            .into();
    }
});

dispatch_light128!(m, Mach, {
    fn get_stream_param(state: &ChaCha, param: u32) -> u64 {
        let d: Mach::u32x4 = m.unpack(state.d);
        ((d.extract((param << 1) | 1) as u64) << 32) | d.extract(param << 1) as u64
    }
});

dispatch_light128!(m, Mach, {
    fn get_seed(state: &ChaCha) -> [u8; 32] {
        let b: Mach::u32x4 = m.unpack(state.b);
        let c: Mach::u32x4 = m.unpack(state.c);
        let mut key = [0u8; 32];
        b.write_le(&mut key[..16]);
        c.write_le(&mut key[16..]);
        key
    }
});

fn read_u32le(xs: &[u8]) -> u32 {
    assert_eq!(xs.len(), 4);
    u32::from(xs[0]) | (u32::from(xs[1]) << 8) | (u32::from(xs[2]) << 16) | (u32::from(xs[3]) << 24)
}

dispatch_light128!(m, Mach, {
    fn init_chacha(key: &[u8; 32], nonce: &[u8]) -> ChaCha {
        let ctr_nonce = [
            0,
            if nonce.len() == 12 {
                read_u32le(&nonce[0..4])
            } else {
                0
            },
            read_u32le(&nonce[nonce.len() - 8..nonce.len() - 4]),
            read_u32le(&nonce[nonce.len() - 4..]),
        ];
        let key0: Mach::u32x4 = m.read_le(&key[..16]);
        let key1: Mach::u32x4 = m.read_le(&key[16..]);
        ChaCha {
            b: key0.into(),
            c: key1.into(),
            d: ctr_nonce.into(),
        }
    }
});

dispatch_light128!(m, Mach, {
    fn init_chacha_x(key: &[u8; 32], nonce: &[u8; 24], rounds: u32) -> ChaCha {
        let key0: Mach::u32x4 = m.read_le(&key[..16]);
        let key1: Mach::u32x4 = m.read_le(&key[16..]);
        let nonce0: Mach::u32x4 = m.read_le(&nonce[..16]);
        let mut state = ChaCha {
            b: key0.into(),
            c: key1.into(),
            d: nonce0.into(),
        };
        let x = refill_narrow_rounds(&mut state, rounds);
        let ctr_nonce1 = [0, 0, read_u32le(&nonce[16..20]), read_u32le(&nonce[20..24])];
        state.b = x.a;
        state.c = x.d;
        state.d = ctr_nonce1.into();
        state
    }
});
