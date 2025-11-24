use crate::Adler32;
use std::ops::{AddAssign, MulAssign, RemAssign};

impl Adler32 {
    pub(crate) fn compute(&mut self, bytes: &[u8]) {
        // The basic algorithm is, for every byte:
        //   a = (a + byte) % MOD
        //   b = (b + a) % MOD
        // where MOD = 65521.
        //
        // For efficiency, we can defer the `% MOD` operations as long as neither a nor b overflows:
        // - Between calls to `write`, we ensure that a and b are always in range 0..MOD.
        // - We use 32-bit arithmetic in this function.
        // - Therefore, a and b must not increase by more than 2^32-MOD without performing a `% MOD`
        //   operation.
        //
        // According to Wikipedia, b is calculated as follows for non-incremental checksumming:
        //   b = n×D1 + (n−1)×D2 + (n−2)×D3 + ... + Dn + n*1 (mod 65521)
        // Where n is the number of bytes and Di is the i-th Byte. We need to change this to account
        // for the previous values of a and b, as well as treat every input Byte as being 255:
        //   b_inc = n×255 + (n-1)×255 + ... + 255 + n*65520
        // Or in other words:
        //   b_inc = n*65520 + n(n+1)/2*255
        // The max chunk size is thus the largest value of n so that b_inc <= 2^32-65521.
        //   2^32-65521 = n*65520 + n(n+1)/2*255
        // Plugging this into an equation solver since I can't math gives n = 5552.18..., so 5552.
        //
        // On top of the optimization outlined above, the algorithm can also be parallelized with a
        // bit more work:
        //
        // Note that b is a linear combination of a vector of input bytes (D1, ..., Dn).
        //
        // If we fix some value k<N and rewrite indices 1, ..., N as
        //
        //   1_1, 1_2, ..., 1_k, 2_1, ..., 2_k, ..., (N/k)_k,
        //
        // then we can express a and b in terms of sums of smaller sequences kb and ka:
        //
        //   ka(j) := D1_j + D2_j + ... + D(N/k)_j where j <= k
        //   kb(j) := (N/k)*D1_j + (N/k-1)*D2_j + ... + D(N/k)_j where j <= k
        //
        //  a = ka(1) + ka(2) + ... + ka(k) + 1
        //  b = k*(kb(1) + kb(2) + ... + kb(k)) - 1*ka(2) - ...  - (k-1)*ka(k) + N
        //
        // We use this insight to unroll the main loop and process k=4 bytes at a time.
        // The resulting code is highly amenable to SIMD acceleration, although the immediate speedups
        // stem from increased pipeline parallelism rather than auto-vectorization.
        //
        // This technique is described in-depth (here:)[https://software.intel.com/content/www/us/\
        // en/develop/articles/fast-computation-of-fletcher-checksums.html]

        const MOD: u32 = 65521;
        const CHUNK_SIZE: usize = 5552 * 4;

        let mut a = u32::from(self.a);
        let mut b = u32::from(self.b);
        let mut a_vec = U32X4([0; 4]);
        let mut b_vec = a_vec;

        let (bytes, remainder) = bytes.split_at(bytes.len() - bytes.len() % 4);

        // iterate over 4 bytes at a time
        let chunk_iter = bytes.chunks_exact(CHUNK_SIZE);
        let remainder_chunk = chunk_iter.remainder();
        for chunk in chunk_iter {
            for byte_vec in chunk.chunks_exact(4) {
                let val = U32X4::from(byte_vec);
                a_vec += val;
                b_vec += a_vec;
            }

            b += CHUNK_SIZE as u32 * a;
            a_vec %= MOD;
            b_vec %= MOD;
            b %= MOD;
        }
        // special-case the final chunk because it may be shorter than the rest
        for byte_vec in remainder_chunk.chunks_exact(4) {
            let val = U32X4::from(byte_vec);
            a_vec += val;
            b_vec += a_vec;
        }
        b += remainder_chunk.len() as u32 * a;
        a_vec %= MOD;
        b_vec %= MOD;
        b %= MOD;

        // combine the sub-sum results into the main sum
        b_vec *= 4;
        b_vec.0[1] += MOD - a_vec.0[1];
        b_vec.0[2] += (MOD - a_vec.0[2]) * 2;
        b_vec.0[3] += (MOD - a_vec.0[3]) * 3;
        for &av in a_vec.0.iter() {
            a += av;
        }
        for &bv in b_vec.0.iter() {
            b += bv;
        }

        // iterate over the remaining few bytes in serial
        for &byte in remainder.iter() {
            a += u32::from(byte);
            b += a;
        }

        self.a = (a % MOD) as u16;
        self.b = (b % MOD) as u16;
    }
}

#[derive(Copy, Clone)]
struct U32X4([u32; 4]);

impl U32X4 {
    #[inline]
    fn from(bytes: &[u8]) -> Self {
        U32X4([
            u32::from(bytes[0]),
            u32::from(bytes[1]),
            u32::from(bytes[2]),
            u32::from(bytes[3]),
        ])
    }
}

impl AddAssign<Self> for U32X4 {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        // Implement this in a primitive manner to help out the compiler a bit.
        self.0[0] += other.0[0];
        self.0[1] += other.0[1];
        self.0[2] += other.0[2];
        self.0[3] += other.0[3];
    }
}

impl RemAssign<u32> for U32X4 {
    #[inline]
    fn rem_assign(&mut self, quotient: u32) {
        self.0[0] %= quotient;
        self.0[1] %= quotient;
        self.0[2] %= quotient;
        self.0[3] %= quotient;
    }
}

impl MulAssign<u32> for U32X4 {
    #[inline]
    fn mul_assign(&mut self, rhs: u32) {
        self.0[0] *= rhs;
        self.0[1] *= rhs;
        self.0[2] *= rhs;
        self.0[3] *= rhs;
    }
}
