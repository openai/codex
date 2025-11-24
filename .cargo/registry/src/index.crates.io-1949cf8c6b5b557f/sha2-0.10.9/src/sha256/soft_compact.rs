use crate::consts::K32;

fn to_u32s(block: &[u8; 64]) -> [u32; 16] {
    use core::convert::TryInto;
    let mut res = [0u32; 16];
    for i in 0..16 {
        let chunk = block[4 * i..][..4].try_into().unwrap();
        res[i] = u32::from_be_bytes(chunk);
    }
    res
}

fn compress_u32(state: &mut [u32; 8], block: [u32; 16]) {
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;

    let mut w = [0; 64];
    w[..16].copy_from_slice(&block);

    for i in 16..64 {
        let w15 = w[i - 15];
        let s0 = (w15.rotate_right(7)) ^ (w15.rotate_right(18)) ^ (w15 >> 3);
        let w2 = w[i - 2];
        let s1 = (w2.rotate_right(17)) ^ (w2.rotate_right(19)) ^ (w2 >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = s1
            .wrapping_add(ch)
            .wrapping_add(K32[i])
            .wrapping_add(w[i])
            .wrapping_add(h);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

pub fn compress(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
    for block in blocks.iter() {
        compress_u32(state, to_u32s(block));
    }
}
