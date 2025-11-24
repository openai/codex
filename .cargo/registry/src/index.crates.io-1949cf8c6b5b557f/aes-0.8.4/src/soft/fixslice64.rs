//! Fixsliced implementations of AES-128, AES-192 and AES-256 (64-bit)
//! adapted from the C implementation.
//!
//! All implementations are fully bitsliced and do not rely on any
//! Look-Up Table (LUT).
//!
//! See the paper at <https://eprint.iacr.org/2020/1123.pdf> for more details.
//!
//! # Author (original C code)
//!
//! Alexandre Adomnicai, Nanyang Technological University, Singapore
//! <alexandre.adomnicai@ntu.edu.sg>
//!
//! Originally licensed MIT. Relicensed as Apache 2.0+MIT with permission.

#![allow(clippy::unreadable_literal)]

use crate::Block;
use cipher::{consts::U4, generic_array::GenericArray};

/// AES block batch size for this implementation
pub(crate) type FixsliceBlocks = U4;

pub(crate) type BatchBlocks = GenericArray<Block, FixsliceBlocks>;

/// AES-128 round keys
pub(crate) type FixsliceKeys128 = [u64; 88];

/// AES-192 round keys
pub(crate) type FixsliceKeys192 = [u64; 104];

/// AES-256 round keys
pub(crate) type FixsliceKeys256 = [u64; 120];

/// 512-bit internal state
pub(crate) type State = [u64; 8];

/// Fully bitsliced AES-128 key schedule to match the fully-fixsliced representation.
pub(crate) fn aes128_key_schedule(key: &[u8; 16]) -> FixsliceKeys128 {
    let mut rkeys = [0u64; 88];

    bitslice(&mut rkeys[..8], key, key, key, key);

    let mut rk_off = 0;
    for rcon in 0..10 {
        memshift32(&mut rkeys, rk_off);
        rk_off += 8;

        sub_bytes(&mut rkeys[rk_off..(rk_off + 8)]);
        sub_bytes_nots(&mut rkeys[rk_off..(rk_off + 8)]);

        if rcon < 8 {
            add_round_constant_bit(&mut rkeys[rk_off..(rk_off + 8)], rcon);
        } else {
            add_round_constant_bit(&mut rkeys[rk_off..(rk_off + 8)], rcon - 8);
            add_round_constant_bit(&mut rkeys[rk_off..(rk_off + 8)], rcon - 7);
            add_round_constant_bit(&mut rkeys[rk_off..(rk_off + 8)], rcon - 5);
            add_round_constant_bit(&mut rkeys[rk_off..(rk_off + 8)], rcon - 4);
        }

        xor_columns(&mut rkeys, rk_off, 8, ror_distance(1, 3));
    }

    // Adjust to match fixslicing format
    #[cfg(aes_compact)]
    {
        for i in (8..88).step_by(16) {
            inv_shift_rows_1(&mut rkeys[i..(i + 8)]);
        }
    }
    #[cfg(not(aes_compact))]
    {
        for i in (8..72).step_by(32) {
            inv_shift_rows_1(&mut rkeys[i..(i + 8)]);
            inv_shift_rows_2(&mut rkeys[(i + 8)..(i + 16)]);
            inv_shift_rows_3(&mut rkeys[(i + 16)..(i + 24)]);
        }
        inv_shift_rows_1(&mut rkeys[72..80]);
    }

    // Account for NOTs removed from sub_bytes
    for i in 1..11 {
        sub_bytes_nots(&mut rkeys[(i * 8)..(i * 8 + 8)]);
    }

    rkeys
}

/// Fully bitsliced AES-192 key schedule to match the fully-fixsliced representation.
pub(crate) fn aes192_key_schedule(key: &[u8; 24]) -> FixsliceKeys192 {
    let mut rkeys = [0u64; 104];
    let mut tmp = [0u64; 8];

    bitslice(
        &mut rkeys[..8],
        &key[..16],
        &key[..16],
        &key[..16],
        &key[..16],
    );
    bitslice(&mut tmp, &key[8..], &key[8..], &key[8..], &key[8..]);

    let mut rcon = 0;
    let mut rk_off = 8;

    loop {
        for i in 0..8 {
            rkeys[rk_off + i] = (0x00ff00ff00ff00ff & (tmp[i] >> 8))
                | (0xff00ff00ff00ff00 & (rkeys[(rk_off - 8) + i] << 8));
        }

        sub_bytes(&mut tmp);
        sub_bytes_nots(&mut tmp);

        add_round_constant_bit(&mut tmp, rcon);
        rcon += 1;

        for i in 0..8 {
            let mut ti = rkeys[rk_off + i];
            ti ^= 0x0f000f000f000f00 & ror(tmp[i], ror_distance(1, 1));
            ti ^= 0xf000f000f000f000 & (ti << 4);
            tmp[i] = ti;
        }
        rkeys[rk_off..(rk_off + 8)].copy_from_slice(&tmp);
        rk_off += 8;

        for i in 0..8 {
            let ui = tmp[i];
            let mut ti = (0x00ff00ff00ff00ff & (rkeys[(rk_off - 16) + i] >> 8))
                | (0xff00ff00ff00ff00 & (ui << 8));
            ti ^= 0x000f000f000f000f & (ui >> 12);
            tmp[i] = ti
                ^ (0xfff0fff0fff0fff0 & (ti << 4))
                ^ (0xff00ff00ff00ff00 & (ti << 8))
                ^ (0xf000f000f000f000 & (ti << 12));
        }
        rkeys[rk_off..(rk_off + 8)].copy_from_slice(&tmp);
        rk_off += 8;

        sub_bytes(&mut tmp);
        sub_bytes_nots(&mut tmp);

        add_round_constant_bit(&mut tmp, rcon);
        rcon += 1;

        for i in 0..8 {
            let mut ti = (0x00ff00ff00ff00ff & (rkeys[(rk_off - 16) + i] >> 8))
                | (0xff00ff00ff00ff00 & (rkeys[(rk_off - 8) + i] << 8));
            ti ^= 0x000f000f000f000f & ror(tmp[i], ror_distance(1, 3));
            rkeys[rk_off + i] = ti
                ^ (0xfff0fff0fff0fff0 & (ti << 4))
                ^ (0xff00ff00ff00ff00 & (ti << 8))
                ^ (0xf000f000f000f000 & (ti << 12));
        }
        rk_off += 8;

        if rcon >= 8 {
            break;
        }

        for i in 0..8 {
            let ui = rkeys[(rk_off - 8) + i];
            let mut ti = rkeys[(rk_off - 16) + i];
            ti ^= 0x0f000f000f000f00 & (ui >> 4);
            ti ^= 0xf000f000f000f000 & (ti << 4);
            tmp[i] = ti;
        }
    }

    // Adjust to match fixslicing format
    #[cfg(aes_compact)]
    {
        for i in (8..104).step_by(16) {
            inv_shift_rows_1(&mut rkeys[i..(i + 8)]);
        }
    }
    #[cfg(not(aes_compact))]
    {
        for i in (0..96).step_by(32) {
            inv_shift_rows_1(&mut rkeys[(i + 8)..(i + 16)]);
            inv_shift_rows_2(&mut rkeys[(i + 16)..(i + 24)]);
            inv_shift_rows_3(&mut rkeys[(i + 24)..(i + 32)]);
        }
    }

    // Account for NOTs removed from sub_bytes
    for i in 1..13 {
        sub_bytes_nots(&mut rkeys[(i * 8)..(i * 8 + 8)]);
    }

    rkeys
}

/// Fully bitsliced AES-256 key schedule to match the fully-fixsliced representation.
pub(crate) fn aes256_key_schedule(key: &[u8; 32]) -> FixsliceKeys256 {
    let mut rkeys = [0u64; 120];

    bitslice(
        &mut rkeys[..8],
        &key[..16],
        &key[..16],
        &key[..16],
        &key[..16],
    );
    bitslice(
        &mut rkeys[8..16],
        &key[16..],
        &key[16..],
        &key[16..],
        &key[16..],
    );

    let mut rk_off = 8;

    let mut rcon = 0;
    loop {
        memshift32(&mut rkeys, rk_off);
        rk_off += 8;

        sub_bytes(&mut rkeys[rk_off..(rk_off + 8)]);
        sub_bytes_nots(&mut rkeys[rk_off..(rk_off + 8)]);

        add_round_constant_bit(&mut rkeys[rk_off..(rk_off + 8)], rcon);
        xor_columns(&mut rkeys, rk_off, 16, ror_distance(1, 3));
        rcon += 1;

        if rcon == 7 {
            break;
        }

        memshift32(&mut rkeys, rk_off);
        rk_off += 8;

        sub_bytes(&mut rkeys[rk_off..(rk_off + 8)]);
        sub_bytes_nots(&mut rkeys[rk_off..(rk_off + 8)]);

        xor_columns(&mut rkeys, rk_off, 16, ror_distance(0, 3));
    }

    // Adjust to match fixslicing format
    #[cfg(aes_compact)]
    {
        for i in (8..120).step_by(16) {
            inv_shift_rows_1(&mut rkeys[i..(i + 8)]);
        }
    }
    #[cfg(not(aes_compact))]
    {
        for i in (8..104).step_by(32) {
            inv_shift_rows_1(&mut rkeys[i..(i + 8)]);
            inv_shift_rows_2(&mut rkeys[(i + 8)..(i + 16)]);
            inv_shift_rows_3(&mut rkeys[(i + 16)..(i + 24)]);
        }
        inv_shift_rows_1(&mut rkeys[104..112]);
    }

    // Account for NOTs removed from sub_bytes
    for i in 1..15 {
        sub_bytes_nots(&mut rkeys[(i * 8)..(i * 8 + 8)]);
    }

    rkeys
}

/// Fully-fixsliced AES-128 decryption (the InvShiftRows is completely omitted).
///
/// Decrypts four blocks in-place and in parallel.
pub(crate) fn aes128_decrypt(rkeys: &FixsliceKeys128, blocks: &BatchBlocks) -> BatchBlocks {
    let mut state = State::default();

    bitslice(&mut state, &blocks[0], &blocks[1], &blocks[2], &blocks[3]);

    add_round_key(&mut state, &rkeys[80..]);
    inv_sub_bytes(&mut state);

    #[cfg(not(aes_compact))]
    {
        inv_shift_rows_2(&mut state);
    }

    let mut rk_off = 72;
    loop {
        #[cfg(aes_compact)]
        {
            inv_shift_rows_2(&mut state);
        }

        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        inv_mix_columns_1(&mut state);
        inv_sub_bytes(&mut state);
        rk_off -= 8;

        if rk_off == 0 {
            break;
        }

        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        inv_mix_columns_0(&mut state);
        inv_sub_bytes(&mut state);
        rk_off -= 8;

        #[cfg(not(aes_compact))]
        {
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            inv_mix_columns_3(&mut state);
            inv_sub_bytes(&mut state);
            rk_off -= 8;

            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            inv_mix_columns_2(&mut state);
            inv_sub_bytes(&mut state);
            rk_off -= 8;
        }
    }

    add_round_key(&mut state, &rkeys[..8]);

    inv_bitslice(&state)
}

/// Fully-fixsliced AES-128 encryption (the ShiftRows is completely omitted).
///
/// Encrypts four blocks in-place and in parallel.
pub(crate) fn aes128_encrypt(rkeys: &FixsliceKeys128, blocks: &BatchBlocks) -> BatchBlocks {
    let mut state = State::default();

    bitslice(&mut state, &blocks[0], &blocks[1], &blocks[2], &blocks[3]);

    add_round_key(&mut state, &rkeys[..8]);

    let mut rk_off = 8;
    loop {
        sub_bytes(&mut state);
        mix_columns_1(&mut state);
        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        rk_off += 8;

        #[cfg(aes_compact)]
        {
            shift_rows_2(&mut state);
        }

        if rk_off == 80 {
            break;
        }

        #[cfg(not(aes_compact))]
        {
            sub_bytes(&mut state);
            mix_columns_2(&mut state);
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            rk_off += 8;

            sub_bytes(&mut state);
            mix_columns_3(&mut state);
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            rk_off += 8;
        }

        sub_bytes(&mut state);
        mix_columns_0(&mut state);
        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        rk_off += 8;
    }

    #[cfg(not(aes_compact))]
    {
        shift_rows_2(&mut state);
    }

    sub_bytes(&mut state);
    add_round_key(&mut state, &rkeys[80..]);

    inv_bitslice(&state)
}

/// Fully-fixsliced AES-192 decryption (the InvShiftRows is completely omitted).
///
/// Decrypts four blocks in-place and in parallel.
pub(crate) fn aes192_decrypt(rkeys: &FixsliceKeys192, blocks: &BatchBlocks) -> BatchBlocks {
    let mut state = State::default();

    bitslice(&mut state, &blocks[0], &blocks[1], &blocks[2], &blocks[3]);

    add_round_key(&mut state, &rkeys[96..]);
    inv_sub_bytes(&mut state);

    let mut rk_off = 88;
    loop {
        #[cfg(aes_compact)]
        {
            inv_shift_rows_2(&mut state);
        }
        #[cfg(not(aes_compact))]
        {
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            inv_mix_columns_3(&mut state);
            inv_sub_bytes(&mut state);
            rk_off -= 8;

            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            inv_mix_columns_2(&mut state);
            inv_sub_bytes(&mut state);
            rk_off -= 8;
        }

        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        inv_mix_columns_1(&mut state);
        inv_sub_bytes(&mut state);
        rk_off -= 8;

        if rk_off == 0 {
            break;
        }

        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        inv_mix_columns_0(&mut state);
        inv_sub_bytes(&mut state);
        rk_off -= 8;
    }

    add_round_key(&mut state, &rkeys[..8]);

    inv_bitslice(&state)
}

/// Fully-fixsliced AES-192 encryption (the ShiftRows is completely omitted).
///
/// Encrypts four blocks in-place and in parallel.
pub(crate) fn aes192_encrypt(rkeys: &FixsliceKeys192, blocks: &BatchBlocks) -> BatchBlocks {
    let mut state = State::default();

    bitslice(&mut state, &blocks[0], &blocks[1], &blocks[2], &blocks[3]);

    add_round_key(&mut state, &rkeys[..8]);

    let mut rk_off = 8;
    loop {
        sub_bytes(&mut state);
        mix_columns_1(&mut state);
        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        rk_off += 8;

        #[cfg(aes_compact)]
        {
            shift_rows_2(&mut state);
        }
        #[cfg(not(aes_compact))]
        {
            sub_bytes(&mut state);
            mix_columns_2(&mut state);
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            rk_off += 8;

            sub_bytes(&mut state);
            mix_columns_3(&mut state);
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            rk_off += 8;
        }

        if rk_off == 96 {
            break;
        }

        sub_bytes(&mut state);
        mix_columns_0(&mut state);
        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        rk_off += 8;
    }

    sub_bytes(&mut state);
    add_round_key(&mut state, &rkeys[96..]);

    inv_bitslice(&state)
}

/// Fully-fixsliced AES-256 decryption (the InvShiftRows is completely omitted).
///
/// Decrypts four blocks in-place and in parallel.
pub(crate) fn aes256_decrypt(rkeys: &FixsliceKeys256, blocks: &BatchBlocks) -> BatchBlocks {
    let mut state = State::default();

    bitslice(&mut state, &blocks[0], &blocks[1], &blocks[2], &blocks[3]);

    add_round_key(&mut state, &rkeys[112..]);
    inv_sub_bytes(&mut state);

    #[cfg(not(aes_compact))]
    {
        inv_shift_rows_2(&mut state);
    }

    let mut rk_off = 104;
    loop {
        #[cfg(aes_compact)]
        {
            inv_shift_rows_2(&mut state);
        }

        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        inv_mix_columns_1(&mut state);
        inv_sub_bytes(&mut state);
        rk_off -= 8;

        if rk_off == 0 {
            break;
        }

        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        inv_mix_columns_0(&mut state);
        inv_sub_bytes(&mut state);
        rk_off -= 8;

        #[cfg(not(aes_compact))]
        {
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            inv_mix_columns_3(&mut state);
            inv_sub_bytes(&mut state);
            rk_off -= 8;

            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            inv_mix_columns_2(&mut state);
            inv_sub_bytes(&mut state);
            rk_off -= 8;
        }
    }

    add_round_key(&mut state, &rkeys[..8]);

    inv_bitslice(&state)
}

/// Fully-fixsliced AES-256 encryption (the ShiftRows is completely omitted).
///
/// Encrypts four blocks in-place and in parallel.
pub(crate) fn aes256_encrypt(rkeys: &FixsliceKeys256, blocks: &BatchBlocks) -> BatchBlocks {
    let mut state = State::default();

    bitslice(&mut state, &blocks[0], &blocks[1], &blocks[2], &blocks[3]);

    add_round_key(&mut state, &rkeys[..8]);

    let mut rk_off = 8;
    loop {
        sub_bytes(&mut state);
        mix_columns_1(&mut state);
        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        rk_off += 8;

        #[cfg(aes_compact)]
        {
            shift_rows_2(&mut state);
        }

        if rk_off == 112 {
            break;
        }

        #[cfg(not(aes_compact))]
        {
            sub_bytes(&mut state);
            mix_columns_2(&mut state);
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            rk_off += 8;

            sub_bytes(&mut state);
            mix_columns_3(&mut state);
            add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
            rk_off += 8;
        }

        sub_bytes(&mut state);
        mix_columns_0(&mut state);
        add_round_key(&mut state, &rkeys[rk_off..(rk_off + 8)]);
        rk_off += 8;
    }

    #[cfg(not(aes_compact))]
    {
        shift_rows_2(&mut state);
    }

    sub_bytes(&mut state);
    add_round_key(&mut state, &rkeys[112..]);

    inv_bitslice(&state)
}

/// Note that the 4 bitwise NOT (^= 0xffffffffffffffff) are accounted for here so that it is a true
/// inverse of 'sub_bytes'.
fn inv_sub_bytes(state: &mut [u64]) {
    debug_assert_eq!(state.len(), 8);

    // Scheduled using https://github.com/Ko-/aes-armcortexm/tree/public/scheduler
    // Inline "stack" comments reflect suggested stores and loads (ARM Cortex-M3 and M4)

    let u7 = state[0];
    let u6 = state[1];
    let u5 = state[2];
    let u4 = state[3];
    let u3 = state[4];
    let u2 = state[5];
    let u1 = state[6];
    let u0 = state[7];

    let t23 = u0 ^ u3;
    let t8 = u1 ^ t23;
    let m2 = t23 & t8;
    let t4 = u4 ^ t8;
    let t22 = u1 ^ u3;
    let t2 = u0 ^ u1;
    let t1 = u3 ^ u4;
    // t23 -> stack
    let t9 = u7 ^ t1;
    // t8 -> stack
    let m7 = t22 & t9;
    // t9 -> stack
    let t24 = u4 ^ u7;
    // m7 -> stack
    let t10 = t2 ^ t24;
    // u4 -> stack
    let m14 = t2 & t10;
    let r5 = u6 ^ u7;
    // m2 -> stack
    let t3 = t1 ^ r5;
    // t2 -> stack
    let t13 = t2 ^ r5;
    let t19 = t22 ^ r5;
    // t3 -> stack
    let t17 = u2 ^ t19;
    // t4 -> stack
    let t25 = u2 ^ t1;
    let r13 = u1 ^ u6;
    // t25 -> stack
    let t20 = t24 ^ r13;
    // t17 -> stack
    let m9 = t20 & t17;
    // t20 -> stack
    let r17 = u2 ^ u5;
    // t22 -> stack
    let t6 = t22 ^ r17;
    // t13 -> stack
    let m1 = t13 & t6;
    let y5 = u0 ^ r17;
    let m4 = t19 & y5;
    let m5 = m4 ^ m1;
    let m17 = m5 ^ t24;
    let r18 = u5 ^ u6;
    let t27 = t1 ^ r18;
    let t15 = t10 ^ t27;
    // t6 -> stack
    let m11 = t1 & t15;
    let m15 = m14 ^ m11;
    let m21 = m17 ^ m15;
    // t1 -> stack
    // t4 <- stack
    let m12 = t4 & t27;
    let m13 = m12 ^ m11;
    let t14 = t10 ^ r18;
    let m3 = t14 ^ m1;
    // m2 <- stack
    let m16 = m3 ^ m2;
    let m20 = m16 ^ m13;
    // u4 <- stack
    let r19 = u2 ^ u4;
    let t16 = r13 ^ r19;
    // t3 <- stack
    let t26 = t3 ^ t16;
    let m6 = t3 & t16;
    let m8 = t26 ^ m6;
    // t10 -> stack
    // m7 <- stack
    let m18 = m8 ^ m7;
    let m22 = m18 ^ m13;
    let m25 = m22 & m20;
    let m26 = m21 ^ m25;
    let m10 = m9 ^ m6;
    let m19 = m10 ^ m15;
    // t25 <- stack
    let m23 = m19 ^ t25;
    let m28 = m23 ^ m25;
    let m24 = m22 ^ m23;
    let m30 = m26 & m24;
    let m39 = m23 ^ m30;
    let m48 = m39 & y5;
    let m57 = m39 & t19;
    // m48 -> stack
    let m36 = m24 ^ m25;
    let m31 = m20 & m23;
    let m27 = m20 ^ m21;
    let m32 = m27 & m31;
    let m29 = m28 & m27;
    let m37 = m21 ^ m29;
    // m39 -> stack
    let m42 = m37 ^ m39;
    let m52 = m42 & t15;
    // t27 -> stack
    // t1 <- stack
    let m61 = m42 & t1;
    let p0 = m52 ^ m61;
    let p16 = m57 ^ m61;
    // m57 -> stack
    // t20 <- stack
    let m60 = m37 & t20;
    // p16 -> stack
    // t17 <- stack
    let m51 = m37 & t17;
    let m33 = m27 ^ m25;
    let m38 = m32 ^ m33;
    let m43 = m37 ^ m38;
    let m49 = m43 & t16;
    let p6 = m49 ^ m60;
    let p13 = m49 ^ m51;
    let m58 = m43 & t3;
    // t9 <- stack
    let m50 = m38 & t9;
    // t22 <- stack
    let m59 = m38 & t22;
    // p6 -> stack
    let p1 = m58 ^ m59;
    let p7 = p0 ^ p1;
    let m34 = m21 & m22;
    let m35 = m24 & m34;
    let m40 = m35 ^ m36;
    let m41 = m38 ^ m40;
    let m45 = m42 ^ m41;
    // t27 <- stack
    let m53 = m45 & t27;
    let p8 = m50 ^ m53;
    let p23 = p7 ^ p8;
    // t4 <- stack
    let m62 = m45 & t4;
    let p14 = m49 ^ m62;
    let s6 = p14 ^ p23;
    // t10 <- stack
    let m54 = m41 & t10;
    let p2 = m54 ^ m62;
    let p22 = p2 ^ p7;
    let s0 = p13 ^ p22;
    let p17 = m58 ^ p2;
    let p15 = m54 ^ m59;
    // t2 <- stack
    let m63 = m41 & t2;
    // m39 <- stack
    let m44 = m39 ^ m40;
    // p17 -> stack
    // t6 <- stack
    let m46 = m44 & t6;
    let p5 = m46 ^ m51;
    // p23 -> stack
    let p18 = m63 ^ p5;
    let p24 = p5 ^ p7;
    // m48 <- stack
    let p12 = m46 ^ m48;
    let s3 = p12 ^ p22;
    // t13 <- stack
    let m55 = m44 & t13;
    let p9 = m55 ^ m63;
    // p16 <- stack
    let s7 = p9 ^ p16;
    // t8 <- stack
    let m47 = m40 & t8;
    let p3 = m47 ^ m50;
    let p19 = p2 ^ p3;
    let s5 = p19 ^ p24;
    let p11 = p0 ^ p3;
    let p26 = p9 ^ p11;
    // t23 <- stack
    let m56 = m40 & t23;
    let p4 = m48 ^ m56;
    // p6 <- stack
    let p20 = p4 ^ p6;
    let p29 = p15 ^ p20;
    let s1 = p26 ^ p29;
    // m57 <- stack
    let p10 = m57 ^ p4;
    let p27 = p10 ^ p18;
    // p23 <- stack
    let s4 = p23 ^ p27;
    let p25 = p6 ^ p10;
    let p28 = p11 ^ p25;
    // p17 <- stack
    let s2 = p17 ^ p28;

    state[0] = s7;
    state[1] = s6;
    state[2] = s5;
    state[3] = s4;
    state[4] = s3;
    state[5] = s2;
    state[6] = s1;
    state[7] = s0;
}

/// Bitsliced implementation of the AES Sbox based on Boyar, Peralta and Calik.
///
/// See: <http://www.cs.yale.edu/homes/peralta/CircuitStuff/SLP_AES_113.txt>
///
/// Note that the 4 bitwise NOT (^= 0xffffffffffffffff) are moved to the key schedule.
fn sub_bytes(state: &mut [u64]) {
    debug_assert_eq!(state.len(), 8);

    // Scheduled using https://github.com/Ko-/aes-armcortexm/tree/public/scheduler
    // Inline "stack" comments reflect suggested stores and loads (ARM Cortex-M3 and M4)

    let u7 = state[0];
    let u6 = state[1];
    let u5 = state[2];
    let u4 = state[3];
    let u3 = state[4];
    let u2 = state[5];
    let u1 = state[6];
    let u0 = state[7];

    let y14 = u3 ^ u5;
    let y13 = u0 ^ u6;
    let y12 = y13 ^ y14;
    let t1 = u4 ^ y12;
    let y15 = t1 ^ u5;
    let t2 = y12 & y15;
    let y6 = y15 ^ u7;
    let y20 = t1 ^ u1;
    // y12 -> stack
    let y9 = u0 ^ u3;
    // y20 -> stack
    let y11 = y20 ^ y9;
    // y9 -> stack
    let t12 = y9 & y11;
    // y6 -> stack
    let y7 = u7 ^ y11;
    let y8 = u0 ^ u5;
    let t0 = u1 ^ u2;
    let y10 = y15 ^ t0;
    // y15 -> stack
    let y17 = y10 ^ y11;
    // y14 -> stack
    let t13 = y14 & y17;
    let t14 = t13 ^ t12;
    // y17 -> stack
    let y19 = y10 ^ y8;
    // y10 -> stack
    let t15 = y8 & y10;
    let t16 = t15 ^ t12;
    let y16 = t0 ^ y11;
    // y11 -> stack
    let y21 = y13 ^ y16;
    // y13 -> stack
    let t7 = y13 & y16;
    // y16 -> stack
    let y18 = u0 ^ y16;
    let y1 = t0 ^ u7;
    let y4 = y1 ^ u3;
    // u7 -> stack
    let t5 = y4 & u7;
    let t6 = t5 ^ t2;
    let t18 = t6 ^ t16;
    let t22 = t18 ^ y19;
    let y2 = y1 ^ u0;
    let t10 = y2 & y7;
    let t11 = t10 ^ t7;
    let t20 = t11 ^ t16;
    let t24 = t20 ^ y18;
    let y5 = y1 ^ u6;
    let t8 = y5 & y1;
    let t9 = t8 ^ t7;
    let t19 = t9 ^ t14;
    let t23 = t19 ^ y21;
    let y3 = y5 ^ y8;
    // y6 <- stack
    let t3 = y3 & y6;
    let t4 = t3 ^ t2;
    // y20 <- stack
    let t17 = t4 ^ y20;
    let t21 = t17 ^ t14;
    let t26 = t21 & t23;
    let t27 = t24 ^ t26;
    let t31 = t22 ^ t26;
    let t25 = t21 ^ t22;
    // y4 -> stack
    let t28 = t25 & t27;
    let t29 = t28 ^ t22;
    let z14 = t29 & y2;
    let z5 = t29 & y7;
    let t30 = t23 ^ t24;
    let t32 = t31 & t30;
    let t33 = t32 ^ t24;
    let t35 = t27 ^ t33;
    let t36 = t24 & t35;
    let t38 = t27 ^ t36;
    let t39 = t29 & t38;
    let t40 = t25 ^ t39;
    let t43 = t29 ^ t40;
    // y16 <- stack
    let z3 = t43 & y16;
    let tc12 = z3 ^ z5;
    // tc12 -> stack
    // y13 <- stack
    let z12 = t43 & y13;
    let z13 = t40 & y5;
    let z4 = t40 & y1;
    let tc6 = z3 ^ z4;
    let t34 = t23 ^ t33;
    let t37 = t36 ^ t34;
    let t41 = t40 ^ t37;
    // y10 <- stack
    let z8 = t41 & y10;
    let z17 = t41 & y8;
    let t44 = t33 ^ t37;
    // y15 <- stack
    let z0 = t44 & y15;
    // z17 -> stack
    // y12 <- stack
    let z9 = t44 & y12;
    let z10 = t37 & y3;
    let z1 = t37 & y6;
    let tc5 = z1 ^ z0;
    let tc11 = tc6 ^ tc5;
    // y4 <- stack
    let z11 = t33 & y4;
    let t42 = t29 ^ t33;
    let t45 = t42 ^ t41;
    // y17 <- stack
    let z7 = t45 & y17;
    let tc8 = z7 ^ tc6;
    // y14 <- stack
    let z16 = t45 & y14;
    // y11 <- stack
    let z6 = t42 & y11;
    let tc16 = z6 ^ tc8;
    // z14 -> stack
    // y9 <- stack
    let z15 = t42 & y9;
    let tc20 = z15 ^ tc16;
    let tc1 = z15 ^ z16;
    let tc2 = z10 ^ tc1;
    let tc21 = tc2 ^ z11;
    let tc3 = z9 ^ tc2;
    let s0 = tc3 ^ tc16;
    let s3 = tc3 ^ tc11;
    let s1 = s3 ^ tc16;
    let tc13 = z13 ^ tc1;
    // u7 <- stack
    let z2 = t33 & u7;
    let tc4 = z0 ^ z2;
    let tc7 = z12 ^ tc4;
    let tc9 = z8 ^ tc7;
    let tc10 = tc8 ^ tc9;
    // z14 <- stack
    let tc17 = z14 ^ tc10;
    let s5 = tc21 ^ tc17;
    let tc26 = tc17 ^ tc20;
    // z17 <- stack
    let s2 = tc26 ^ z17;
    // tc12 <- stack
    let tc14 = tc4 ^ tc12;
    let tc18 = tc13 ^ tc14;
    let s6 = tc10 ^ tc18;
    let s7 = z12 ^ tc18;
    let s4 = tc14 ^ s3;

    state[0] = s7;
    state[1] = s6;
    state[2] = s5;
    state[3] = s4;
    state[4] = s3;
    state[5] = s2;
    state[6] = s1;
    state[7] = s0;
}

/// NOT operations that are omitted in S-box
#[inline]
fn sub_bytes_nots(state: &mut [u64]) {
    debug_assert_eq!(state.len(), 8);
    state[0] ^= 0xffffffffffffffff;
    state[1] ^= 0xffffffffffffffff;
    state[5] ^= 0xffffffffffffffff;
    state[6] ^= 0xffffffffffffffff;
}

/// Computation of the MixColumns transformation in the fixsliced representation, with different
/// rotations used according to the round number mod 4.
///
/// Based on Käsper-Schwabe, similar to https://github.com/Ko-/aes-armcortexm.
macro_rules! define_mix_columns {
    (
        $name:ident,
        $name_inv:ident,
        $first_rotate:path,
        $second_rotate:path
    ) => {
        #[rustfmt::skip]
        fn $name(state: &mut State) {
            let (a0, a1, a2, a3, a4, a5, a6, a7) = (
                state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7]
            );
            let (b0, b1, b2, b3, b4, b5, b6, b7) = (
                $first_rotate(a0),
                $first_rotate(a1),
                $first_rotate(a2),
                $first_rotate(a3),
                $first_rotate(a4),
                $first_rotate(a5),
                $first_rotate(a6),
                $first_rotate(a7),
            );
            let (c0, c1, c2, c3, c4, c5, c6, c7) = (
                a0 ^ b0,
                a1 ^ b1,
                a2 ^ b2,
                a3 ^ b3,
                a4 ^ b4,
                a5 ^ b5,
                a6 ^ b6,
                a7 ^ b7,
            );
            state[0] = b0      ^ c7 ^ $second_rotate(c0);
            state[1] = b1 ^ c0 ^ c7 ^ $second_rotate(c1);
            state[2] = b2 ^ c1      ^ $second_rotate(c2);
            state[3] = b3 ^ c2 ^ c7 ^ $second_rotate(c3);
            state[4] = b4 ^ c3 ^ c7 ^ $second_rotate(c4);
            state[5] = b5 ^ c4      ^ $second_rotate(c5);
            state[6] = b6 ^ c5      ^ $second_rotate(c6);
            state[7] = b7 ^ c6      ^ $second_rotate(c7);
        }

        #[rustfmt::skip]
        fn $name_inv(state: &mut State) {
            let (a0, a1, a2, a3, a4, a5, a6, a7) = (
                state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7]
            );
            let (b0, b1, b2, b3, b4, b5, b6, b7) = (
                $first_rotate(a0),
                $first_rotate(a1),
                $first_rotate(a2),
                $first_rotate(a3),
                $first_rotate(a4),
                $first_rotate(a5),
                $first_rotate(a6),
                $first_rotate(a7),
            );
            let (c0, c1, c2, c3, c4, c5, c6, c7) = (
                a0 ^ b0,
                a1 ^ b1,
                a2 ^ b2,
                a3 ^ b3,
                a4 ^ b4,
                a5 ^ b5,
                a6 ^ b6,
                a7 ^ b7,
            );
            let (d0, d1, d2, d3, d4, d5, d6, d7) = (
                a0      ^ c7,
                a1 ^ c0 ^ c7,
                a2 ^ c1,
                a3 ^ c2 ^ c7,
                a4 ^ c3 ^ c7,
                a5 ^ c4,
                a6 ^ c5,
                a7 ^ c6,
            );
            let (e0, e1, e2, e3, e4, e5, e6, e7) = (
                c0      ^ d6,
                c1      ^ d6 ^ d7,
                c2 ^ d0      ^ d7,
                c3 ^ d1 ^ d6,
                c4 ^ d2 ^ d6 ^ d7,
                c5 ^ d3      ^ d7,
                c6 ^ d4,
                c7 ^ d5,
            );
            state[0] = d0 ^ e0 ^ $second_rotate(e0);
            state[1] = d1 ^ e1 ^ $second_rotate(e1);
            state[2] = d2 ^ e2 ^ $second_rotate(e2);
            state[3] = d3 ^ e3 ^ $second_rotate(e3);
            state[4] = d4 ^ e4 ^ $second_rotate(e4);
            state[5] = d5 ^ e5 ^ $second_rotate(e5);
            state[6] = d6 ^ e6 ^ $second_rotate(e6);
            state[7] = d7 ^ e7 ^ $second_rotate(e7);
        }
    }
}

define_mix_columns!(
    mix_columns_0,
    inv_mix_columns_0,
    rotate_rows_1,
    rotate_rows_2
);

define_mix_columns!(
    mix_columns_1,
    inv_mix_columns_1,
    rotate_rows_and_columns_1_1,
    rotate_rows_and_columns_2_2
);

#[cfg(not(aes_compact))]
define_mix_columns!(
    mix_columns_2,
    inv_mix_columns_2,
    rotate_rows_and_columns_1_2,
    rotate_rows_2
);

#[cfg(not(aes_compact))]
define_mix_columns!(
    mix_columns_3,
    inv_mix_columns_3,
    rotate_rows_and_columns_1_3,
    rotate_rows_and_columns_2_2
);

#[inline]
fn delta_swap_1(a: &mut u64, shift: u32, mask: u64) {
    let t = (*a ^ ((*a) >> shift)) & mask;
    *a ^= t ^ (t << shift);
}

#[inline]
fn delta_swap_2(a: &mut u64, b: &mut u64, shift: u32, mask: u64) {
    let t = (*a ^ ((*b) >> shift)) & mask;
    *a ^= t;
    *b ^= t << shift;
}

/// Applies ShiftRows once on an AES state (or key).
#[cfg(any(not(aes_compact), feature = "hazmat"))]
#[inline]
fn shift_rows_1(state: &mut [u64]) {
    debug_assert_eq!(state.len(), 8);
    for x in state.iter_mut() {
        delta_swap_1(x, 8, 0x00f000ff000f0000);
        delta_swap_1(x, 4, 0x0f0f00000f0f0000);
    }
}

/// Applies ShiftRows twice on an AES state (or key).
#[inline]
fn shift_rows_2(state: &mut [u64]) {
    debug_assert_eq!(state.len(), 8);
    for x in state.iter_mut() {
        delta_swap_1(x, 8, 0x00ff000000ff0000);
    }
}

/// Applies ShiftRows three times on an AES state (or key).
#[inline]
fn shift_rows_3(state: &mut [u64]) {
    debug_assert_eq!(state.len(), 8);
    for x in state.iter_mut() {
        delta_swap_1(x, 8, 0x000f00ff00f00000);
        delta_swap_1(x, 4, 0x0f0f00000f0f0000);
    }
}

#[inline(always)]
fn inv_shift_rows_1(state: &mut [u64]) {
    shift_rows_3(state);
}

#[inline(always)]
fn inv_shift_rows_2(state: &mut [u64]) {
    shift_rows_2(state);
}

#[cfg(not(aes_compact))]
#[inline(always)]
fn inv_shift_rows_3(state: &mut [u64]) {
    shift_rows_1(state);
}

/// XOR the columns after the S-box during the key schedule round function.
///
/// The `idx_xor` parameter refers to the index of the previous round key that is
/// involved in the XOR computation (should be 8 and 16 for AES-128 and AES-256,
/// respectively).
///
/// The `idx_ror` parameter refers to the rotation value, which varies between the
/// different key schedules.
fn xor_columns(rkeys: &mut [u64], offset: usize, idx_xor: usize, idx_ror: u32) {
    for i in 0..8 {
        let off_i = offset + i;
        let rk = rkeys[off_i - idx_xor] ^ (0x000f000f000f000f & ror(rkeys[off_i], idx_ror));
        rkeys[off_i] = rk
            ^ (0xfff0fff0fff0fff0 & (rk << 4))
            ^ (0xff00ff00ff00ff00 & (rk << 8))
            ^ (0xf000f000f000f000 & (rk << 12));
    }
}

/// Bitslice four 128-bit input blocks input0, input1, input2, input3 into a 512-bit internal state.
fn bitslice(output: &mut [u64], input0: &[u8], input1: &[u8], input2: &[u8], input3: &[u8]) {
    debug_assert_eq!(output.len(), 8);
    debug_assert_eq!(input0.len(), 16);
    debug_assert_eq!(input1.len(), 16);
    debug_assert_eq!(input2.len(), 16);
    debug_assert_eq!(input3.len(), 16);

    // Bitslicing is a bit index manipulation. 512 bits of data means each bit is positioned at a
    // 9-bit index. AES data is 4 blocks, each one a 4x4 column-major matrix of bytes, so the
    // index is initially ([b]lock, [c]olumn, [r]ow, [p]osition):
    //     b1 b0 c1 c0 r1 r0 p2 p1 p0
    //
    // The desired bitsliced data groups first by bit position, then row, column, block:
    //     p2 p1 p0 r1 r0 c1 c0 b1 b0

    #[rustfmt::skip]
    fn read_reordered(input: &[u8]) -> u64 {
        (u64::from(input[0x0])        ) |
        (u64::from(input[0x1]) << 0x10) |
        (u64::from(input[0x2]) << 0x20) |
        (u64::from(input[0x3]) << 0x30) |
        (u64::from(input[0x8]) << 0x08) |
        (u64::from(input[0x9]) << 0x18) |
        (u64::from(input[0xa]) << 0x28) |
        (u64::from(input[0xb]) << 0x38)
    }

    // Reorder each block's bytes on input
    //     __ __ c1 c0 r1 r0 __ __ __ => __ __ c0 r1 r0 c1 __ __ __
    // Reorder by relabeling (note the order of input)
    //     b1 b0 c0 __ __ __ __ __ __ => c0 b1 b0 __ __ __ __ __ __
    let mut t0 = read_reordered(&input0[0x00..0x0c]);
    let mut t4 = read_reordered(&input0[0x04..0x10]);
    let mut t1 = read_reordered(&input1[0x00..0x0c]);
    let mut t5 = read_reordered(&input1[0x04..0x10]);
    let mut t2 = read_reordered(&input2[0x00..0x0c]);
    let mut t6 = read_reordered(&input2[0x04..0x10]);
    let mut t3 = read_reordered(&input3[0x00..0x0c]);
    let mut t7 = read_reordered(&input3[0x04..0x10]);

    // Bit Index Swap 6 <-> 0:
    //     __ __ b0 __ __ __ __ __ p0 => __ __ p0 __ __ __ __ __ b0
    let m0 = 0x5555555555555555;
    delta_swap_2(&mut t1, &mut t0, 1, m0);
    delta_swap_2(&mut t3, &mut t2, 1, m0);
    delta_swap_2(&mut t5, &mut t4, 1, m0);
    delta_swap_2(&mut t7, &mut t6, 1, m0);

    // Bit Index Swap 7 <-> 1:
    //     __ b1 __ __ __ __ __ p1 __ => __ p1 __ __ __ __ __ b1 __
    let m1 = 0x3333333333333333;
    delta_swap_2(&mut t2, &mut t0, 2, m1);
    delta_swap_2(&mut t3, &mut t1, 2, m1);
    delta_swap_2(&mut t6, &mut t4, 2, m1);
    delta_swap_2(&mut t7, &mut t5, 2, m1);

    // Bit Index Swap 8 <-> 2:
    //     c0 __ __ __ __ __ p2 __ __ => p2 __ __ __ __ __ c0 __ __
    let m2 = 0x0f0f0f0f0f0f0f0f;
    delta_swap_2(&mut t4, &mut t0, 4, m2);
    delta_swap_2(&mut t5, &mut t1, 4, m2);
    delta_swap_2(&mut t6, &mut t2, 4, m2);
    delta_swap_2(&mut t7, &mut t3, 4, m2);

    // Final bitsliced bit index, as desired:
    //     p2 p1 p0 r1 r0 c1 c0 b1 b0
    output[0] = t0;
    output[1] = t1;
    output[2] = t2;
    output[3] = t3;
    output[4] = t4;
    output[5] = t5;
    output[6] = t6;
    output[7] = t7;
}

/// Un-bitslice a 512-bit internal state into four 128-bit blocks of output.
fn inv_bitslice(input: &[u64]) -> BatchBlocks {
    debug_assert_eq!(input.len(), 8);

    // Unbitslicing is a bit index manipulation. 512 bits of data means each bit is positioned at
    // a 9-bit index. AES data is 4 blocks, each one a 4x4 column-major matrix of bytes, so the
    // desired index for the output is ([b]lock, [c]olumn, [r]ow, [p]osition):
    //     b1 b0 c1 c0 r1 r0 p2 p1 p0
    //
    // The initially bitsliced data groups first by bit position, then row, column, block:
    //     p2 p1 p0 r1 r0 c1 c0 b1 b0

    let mut t0 = input[0];
    let mut t1 = input[1];
    let mut t2 = input[2];
    let mut t3 = input[3];
    let mut t4 = input[4];
    let mut t5 = input[5];
    let mut t6 = input[6];
    let mut t7 = input[7];

    // TODO: these bit index swaps are identical to those in 'packing'

    // Bit Index Swap 6 <-> 0:
    //     __ __ p0 __ __ __ __ __ b0 => __ __ b0 __ __ __ __ __ p0
    let m0 = 0x5555555555555555;
    delta_swap_2(&mut t1, &mut t0, 1, m0);
    delta_swap_2(&mut t3, &mut t2, 1, m0);
    delta_swap_2(&mut t5, &mut t4, 1, m0);
    delta_swap_2(&mut t7, &mut t6, 1, m0);

    // Bit Index Swap 7 <-> 1:
    //     __ p1 __ __ __ __ __ b1 __ => __ b1 __ __ __ __ __ p1 __
    let m1 = 0x3333333333333333;
    delta_swap_2(&mut t2, &mut t0, 2, m1);
    delta_swap_2(&mut t3, &mut t1, 2, m1);
    delta_swap_2(&mut t6, &mut t4, 2, m1);
    delta_swap_2(&mut t7, &mut t5, 2, m1);

    // Bit Index Swap 8 <-> 2:
    //     p2 __ __ __ __ __ c0 __ __ => c0 __ __ __ __ __ p2 __ __
    let m2 = 0x0f0f0f0f0f0f0f0f;
    delta_swap_2(&mut t4, &mut t0, 4, m2);
    delta_swap_2(&mut t5, &mut t1, 4, m2);
    delta_swap_2(&mut t6, &mut t2, 4, m2);
    delta_swap_2(&mut t7, &mut t3, 4, m2);

    #[rustfmt::skip]
    fn write_reordered(columns: u64, output: &mut [u8]) {
        output[0x0] = (columns        ) as u8;
        output[0x1] = (columns >> 0x10) as u8;
        output[0x2] = (columns >> 0x20) as u8;
        output[0x3] = (columns >> 0x30) as u8;
        output[0x8] = (columns >> 0x08) as u8;
        output[0x9] = (columns >> 0x18) as u8;
        output[0xa] = (columns >> 0x28) as u8;
        output[0xb] = (columns >> 0x38) as u8;
    }

    let mut output = BatchBlocks::default();
    // Reorder by relabeling (note the order of output)
    //     c0 b1 b0 __ __ __ __ __ __ => b1 b0 c0 __ __ __ __ __ __
    // Reorder each block's bytes on output
    //     __ __ c0 r1 r0 c1 __ __ __ => __ __ c1 c0 r1 r0 __ __ __
    write_reordered(t0, &mut output[0][0x00..0x0c]);
    write_reordered(t4, &mut output[0][0x04..0x10]);
    write_reordered(t1, &mut output[1][0x00..0x0c]);
    write_reordered(t5, &mut output[1][0x04..0x10]);
    write_reordered(t2, &mut output[2][0x00..0x0c]);
    write_reordered(t6, &mut output[2][0x04..0x10]);
    write_reordered(t3, &mut output[3][0x00..0x0c]);
    write_reordered(t7, &mut output[3][0x04..0x10]);

    // Final AES bit index, as desired:
    //     b1 b0 c1 c0 r1 r0 p2 p1 p0
    output
}

/// Copy 32-bytes within the provided slice to an 8-byte offset
fn memshift32(buffer: &mut [u64], src_offset: usize) {
    debug_assert_eq!(src_offset % 8, 0);

    let dst_offset = src_offset + 8;
    debug_assert!(dst_offset + 8 <= buffer.len());

    for i in (0..8).rev() {
        buffer[dst_offset + i] = buffer[src_offset + i];
    }
}

/// XOR the round key to the internal state. The round keys are expected to be
/// pre-computed and to be packed in the fixsliced representation.
#[inline]
fn add_round_key(state: &mut State, rkey: &[u64]) {
    debug_assert_eq!(rkey.len(), 8);
    for (a, b) in state.iter_mut().zip(rkey) {
        *a ^= b;
    }
}

#[inline(always)]
fn add_round_constant_bit(state: &mut [u64], bit: usize) {
    state[bit] ^= 0x00000000f0000000;
}

#[inline(always)]
fn ror(x: u64, y: u32) -> u64 {
    x.rotate_right(y)
}

#[inline(always)]
fn ror_distance(rows: u32, cols: u32) -> u32 {
    (rows << 4) + (cols << 2)
}

#[inline(always)]
fn rotate_rows_1(x: u64) -> u64 {
    ror(x, ror_distance(1, 0))
}

#[inline(always)]
fn rotate_rows_2(x: u64) -> u64 {
    ror(x, ror_distance(2, 0))
}

#[inline(always)]
#[rustfmt::skip]
fn rotate_rows_and_columns_1_1(x: u64) -> u64 {
    (ror(x, ror_distance(1, 1)) & 0x0fff0fff0fff0fff) |
    (ror(x, ror_distance(0, 1)) & 0xf000f000f000f000)
}

#[cfg(not(aes_compact))]
#[inline(always)]
#[rustfmt::skip]
fn rotate_rows_and_columns_1_2(x: u64) -> u64 {
    (ror(x, ror_distance(1, 2)) & 0x00ff00ff00ff00ff) |
    (ror(x, ror_distance(0, 2)) & 0xff00ff00ff00ff00)
}

#[cfg(not(aes_compact))]
#[inline(always)]
#[rustfmt::skip]
fn rotate_rows_and_columns_1_3(x: u64) -> u64 {
    (ror(x, ror_distance(1, 3)) & 0x000f000f000f000f) |
    (ror(x, ror_distance(0, 3)) & 0xfff0fff0fff0fff0)
}

#[inline(always)]
#[rustfmt::skip]
fn rotate_rows_and_columns_2_2(x: u64) -> u64 {
    (ror(x, ror_distance(2, 2)) & 0x00ff00ff00ff00ff) |
    (ror(x, ror_distance(1, 2)) & 0xff00ff00ff00ff00)
}

/// Low-level "hazmat" AES functions.
///
/// Note: this isn't actually used in the `Aes128`/`Aes192`/`Aes256`
/// implementations in this crate, but instead provides raw access to
/// the AES round function gated under the `hazmat` crate feature.
#[cfg(feature = "hazmat")]
pub(crate) mod hazmat {
    use super::{
        bitslice, inv_bitslice, inv_mix_columns_0, inv_shift_rows_1, inv_sub_bytes, mix_columns_0,
        shift_rows_1, sub_bytes, sub_bytes_nots, State,
    };
    use crate::{Block, Block8};

    /// XOR the `src` block into the `dst` block in-place.
    fn xor_in_place(dst: &mut Block, src: &Block) {
        for (a, b) in dst.iter_mut().zip(src.as_slice()) {
            *a ^= *b;
        }
    }

    /// Perform a bitslice operation, loading a single block.
    fn bitslice_block(block: &Block) -> State {
        let mut state = State::default();
        bitslice(&mut state, block, block, block, block);
        state
    }

    /// Perform an inverse bitslice operation, extracting a single block.
    fn inv_bitslice_block(block: &mut Block, state: &State) {
        block.copy_from_slice(&inv_bitslice(state)[0]);
    }

    /// AES cipher (encrypt) round function.
    #[inline]
    pub(crate) fn cipher_round(block: &mut Block, round_key: &Block) {
        let mut state = bitslice_block(block);
        sub_bytes(&mut state);
        sub_bytes_nots(&mut state);
        shift_rows_1(&mut state);
        mix_columns_0(&mut state);
        inv_bitslice_block(block, &state);
        xor_in_place(block, round_key);
    }

    /// AES cipher (encrypt) round function: parallel version.
    #[inline]
    pub(crate) fn cipher_round_par(blocks: &mut Block8, round_keys: &Block8) {
        for (chunk, keys) in blocks.chunks_exact_mut(4).zip(round_keys.chunks_exact(4)) {
            let mut state = State::default();
            bitslice(&mut state, &chunk[0], &chunk[1], &chunk[2], &chunk[3]);
            sub_bytes(&mut state);
            sub_bytes_nots(&mut state);
            shift_rows_1(&mut state);
            mix_columns_0(&mut state);
            let res = inv_bitslice(&state);

            for i in 0..4 {
                chunk[i] = res[i];
                xor_in_place(&mut chunk[i], &keys[i]);
            }
        }
    }

    /// AES cipher (encrypt) round function.
    #[inline]
    pub(crate) fn equiv_inv_cipher_round(block: &mut Block, round_key: &Block) {
        let mut state = State::default();
        bitslice(&mut state, block, block, block, block);
        sub_bytes_nots(&mut state);
        inv_sub_bytes(&mut state);
        inv_shift_rows_1(&mut state);
        inv_mix_columns_0(&mut state);
        inv_bitslice_block(block, &state);
        xor_in_place(block, round_key);
    }

    /// AES cipher (encrypt) round function: parallel version.
    #[inline]
    pub(crate) fn equiv_inv_cipher_round_par(blocks: &mut Block8, round_keys: &Block8) {
        for (chunk, keys) in blocks.chunks_exact_mut(4).zip(round_keys.chunks_exact(4)) {
            let mut state = State::default();
            bitslice(&mut state, &chunk[0], &chunk[1], &chunk[2], &chunk[3]);
            sub_bytes_nots(&mut state);
            inv_sub_bytes(&mut state);
            inv_shift_rows_1(&mut state);
            inv_mix_columns_0(&mut state);
            let res = inv_bitslice(&state);

            for i in 0..4 {
                chunk[i] = res[i];
                xor_in_place(&mut chunk[i], &keys[i]);
            }
        }
    }

    /// AES mix columns function.
    #[inline]
    pub(crate) fn mix_columns(block: &mut Block) {
        let mut state = bitslice_block(block);
        mix_columns_0(&mut state);
        inv_bitslice_block(block, &state);
    }

    /// AES inverse mix columns function.
    #[inline]
    pub(crate) fn inv_mix_columns(block: &mut Block) {
        let mut state = bitslice_block(block);
        inv_mix_columns_0(&mut state);
        inv_bitslice_block(block, &state);
    }
}
