use rand::RngCore;

pub(crate) fn get_rng() -> impl RngCore {
    XorShiftStar {
        a: 0x0123_4567_89AB_CDEF,
    }
}

/// Simple `Rng` for benchmarking without additional dependencies
struct XorShiftStar {
    a: u64,
}

impl RngCore for XorShiftStar {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        // https://en.wikipedia.org/wiki/Xorshift#xorshift*
        self.a ^= self.a >> 12;
        self.a ^= self.a << 25;
        self.a ^= self.a >> 27;
        self.a.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            let slice = &bytes[..chunk.len()];
            chunk.copy_from_slice(slice)
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        Ok(self.fill_bytes(dest))
    }
}
