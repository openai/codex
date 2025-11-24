//! Xorshift RNG used for tests. Based on the `rand_xorshift` crate.
use core::num::Wrapping;

/// Initial RNG state used in tests.
// chosen by fair dice roll. guaranteed to be random.
pub(crate) const RNG: XorShiftRng = XorShiftRng {
    x: Wrapping(0x0787_3B4A),
    y: Wrapping(0xFAAB_8FFE),
    z: Wrapping(0x1745_980F),
    w: Wrapping(0xB0AD_B4F3),
};

/// Xorshift RNG instance/
pub(crate) struct XorShiftRng {
    x: Wrapping<u32>,
    y: Wrapping<u32>,
    z: Wrapping<u32>,
    w: Wrapping<u32>,
}

impl XorShiftRng {
    pub(crate) fn fill(&mut self, buf: &mut [u8; 1024]) {
        for chunk in buf.chunks_exact_mut(4) {
            chunk.copy_from_slice(&self.next_u32().to_le_bytes());
        }
    }

    fn next_u32(&mut self) -> u32 {
        let x = self.x;
        let t = x ^ (x << 11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        let w = self.w;
        self.w = w ^ (w >> 19) ^ (t ^ (t >> 8));
        self.w.0
    }
}
