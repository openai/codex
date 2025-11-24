//! Specialized checksum code for the x86 CPU architecture, based on the efficient algorithm described
//! in the following whitepaper:
//!
//! Gopal, V., Ozturk, E., Guilford, J., Wolrich, G., Feghali, W., Dixon, M., & Karakoyunlu, D. (2009).
//! _Fast CRC computation for generic polynomials using PCLMULQDQ instruction_. Intel.
//! (Mirror link: <https://fossies.org/linux/zlib-ng/doc/crc-pclmulqdq.pdf>, accessed 2024-05-20)
//!
//! Throughout the code, this work is referred to as "the paper".

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

#[derive(Clone)]
pub struct State {
    state: u32,
}

impl State {
    #[cfg(not(feature = "std"))]
    pub fn new(state: u32) -> Option<Self> {
        if cfg!(target_feature = "pclmulqdq")
            && cfg!(target_feature = "sse2")
            && cfg!(target_feature = "sse4.1")
        {
            // SAFETY: The conditions above ensure that all
            //         required instructions are supported by the CPU.
            Some(Self { state })
        } else {
            None
        }
    }

    #[cfg(feature = "std")]
    pub fn new(state: u32) -> Option<Self> {
        if is_x86_feature_detected!("pclmulqdq")
            && is_x86_feature_detected!("sse2")
            && is_x86_feature_detected!("sse4.1")
        {
            // SAFETY: The conditions above ensure that all
            //         required instructions are supported by the CPU.
            Some(Self { state })
        } else {
            None
        }
    }

    pub fn update(&mut self, buf: &[u8]) {
        // SAFETY: The `State::new` constructor ensures that all
        //         required instructions are supported by the CPU.
        self.state = unsafe { calculate(self.state, buf) }
    }

    pub fn finalize(self) -> u32 {
        self.state
    }

    pub fn reset(&mut self) {
        self.state = 0;
    }

    pub fn combine(&mut self, other: u32, amount: u64) {
        self.state = crate::combine::combine(self.state, other, amount);
    }
}

const K1: i64 = 0x154442bd4;
const K2: i64 = 0x1c6e41596;
const K3: i64 = 0x1751997d0;
const K4: i64 = 0x0ccaa009e;
const K5: i64 = 0x163cd6124;

const P_X: i64 = 0x1DB710641;
const U_PRIME: i64 = 0x1F7011641;

#[target_feature(enable = "pclmulqdq", enable = "sse2", enable = "sse4.1")]
unsafe fn calculate(crc: u32, mut data: &[u8]) -> u32 {
    // In theory we can accelerate smaller chunks too, but for now just rely on
    // the fallback implementation as it's too much hassle and doesn't seem too
    // beneficial.
    if data.len() < 128 {
        return crate::baseline::update_fast_16(crc, data);
    }

    // Step 1: fold by 4 loop
    let mut x3 = get(&mut data);
    let mut x2 = get(&mut data);
    let mut x1 = get(&mut data);
    let mut x0 = get(&mut data);

    // fold in our initial value, part of the incremental crc checksum
    x3 = arch::_mm_xor_si128(x3, arch::_mm_cvtsi32_si128(!crc as i32));

    let k1k2 = arch::_mm_set_epi64x(K2, K1);
    while data.len() >= 64 {
        x3 = reduce128(x3, get(&mut data), k1k2);
        x2 = reduce128(x2, get(&mut data), k1k2);
        x1 = reduce128(x1, get(&mut data), k1k2);
        x0 = reduce128(x0, get(&mut data), k1k2);
    }

    let k3k4 = arch::_mm_set_epi64x(K4, K3);
    let mut x = reduce128(x3, x2, k3k4);
    x = reduce128(x, x1, k3k4);
    x = reduce128(x, x0, k3k4);

    // Step 2: fold by 1 loop
    while data.len() >= 16 {
        x = reduce128(x, get(&mut data), k3k4);
    }

    // Perform step 3, reduction from 128 bits to 64 bits. This is
    // significantly different from the paper and basically doesn't follow it
    // at all. It's not really clear why, but implementations of this algorithm
    // in Chrome/Linux diverge in the same way. It is beyond me why this is
    // different than the paper, maybe the paper has like errata or something?
    // Unclear.
    //
    // It's also not clear to me what's actually happening here and/or why, but
    // algebraically what's happening is:
    //
    // x = (x[0:63] • K4) ^ x[64:127]           // 96 bit result
    // x = ((x[0:31] as u64) • K5) ^ x[32:95]   // 64 bit result
    //
    // It's... not clear to me what's going on here. The paper itself is pretty
    // vague on this part but definitely uses different constants at least.
    // It's not clear to me, reading the paper, where the xor operations are
    // happening or why things are shifting around. This implementation...
    // appears to work though!
    let x = arch::_mm_xor_si128(
        arch::_mm_clmulepi64_si128(x, k3k4, 0x10),
        arch::_mm_srli_si128(x, 8),
    );
    let x = arch::_mm_xor_si128(
        arch::_mm_clmulepi64_si128(
            arch::_mm_and_si128(x, arch::_mm_set_epi32(0, 0, 0, !0)),
            arch::_mm_set_epi64x(0, K5),
            0x00,
        ),
        arch::_mm_srli_si128(x, 4),
    );

    // Perform a Barrett reduction from our now 64 bits to 32 bits. The
    // algorithm for this is described at the end of the paper, and note that
    // this also implements the "bit reflected input" variant.
    let pu = arch::_mm_set_epi64x(U_PRIME, P_X);

    // T1(x) = ⌊(R(x) % x^32)⌋ • μ
    let t1 = arch::_mm_clmulepi64_si128(
        arch::_mm_and_si128(x, arch::_mm_set_epi32(0, 0, 0, !0)),
        pu,
        0x10,
    );
    // T2(x) = ⌊(T1(x) % x^32)⌋ • P(x)
    let t2 = arch::_mm_clmulepi64_si128(
        arch::_mm_and_si128(t1, arch::_mm_set_epi32(0, 0, 0, !0)),
        pu,
        0x00,
    );
    // We're doing the bit-reflected variant, so get the upper 32-bits of the
    // 64-bit result instead of the lower 32-bits.
    //
    // C(x) = R(x) ^ T2(x) / x^32
    let c = arch::_mm_extract_epi32(arch::_mm_xor_si128(x, t2), 1) as u32;

    if !data.is_empty() {
        crate::baseline::update_fast_16(!c, data)
    } else {
        !c
    }
}

unsafe fn reduce128(a: arch::__m128i, b: arch::__m128i, keys: arch::__m128i) -> arch::__m128i {
    let t1 = arch::_mm_clmulepi64_si128(a, keys, 0x00);
    let t2 = arch::_mm_clmulepi64_si128(a, keys, 0x11);
    arch::_mm_xor_si128(arch::_mm_xor_si128(b, t1), t2)
}

unsafe fn get(a: &mut &[u8]) -> arch::__m128i {
    debug_assert!(a.len() >= 16);
    let r = arch::_mm_loadu_si128(a.as_ptr() as *const arch::__m128i);
    *a = &a[16..];
    r
}

#[cfg(test)]
mod test {
    quickcheck::quickcheck! {
        fn check_against_baseline(init: u32, chunks: Vec<(Vec<u8>, usize)>) -> bool {
            let mut baseline = super::super::super::baseline::State::new(init);
            let mut pclmulqdq = super::State::new(init).expect("not supported");
            for (chunk, mut offset) in chunks {
                // simulate random alignments by offsetting the slice by up to 15 bytes
                offset &= 0xF;
                if chunk.len() <= offset {
                    baseline.update(&chunk);
                    pclmulqdq.update(&chunk);
                } else {
                    baseline.update(&chunk[offset..]);
                    pclmulqdq.update(&chunk[offset..]);
                }
            }
            pclmulqdq.finalize() == baseline.finalize()
        }
    }
}
