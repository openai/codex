use core::arch::aarch64 as arch;

#[derive(Clone)]
pub struct State {
    state: u32,
}

impl State {
    #[cfg(not(feature = "std"))]
    pub fn new(state: u32) -> Option<Self> {
        if cfg!(target_feature = "crc") {
            // SAFETY: The conditions above ensure that all
            //         required instructions are supported by the CPU.
            Some(Self { state })
        } else {
            None
        }
    }

    #[cfg(feature = "std")]
    pub fn new(state: u32) -> Option<Self> {
        if std::arch::is_aarch64_feature_detected!("crc") {
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

// target_feature is necessary to allow rustc to inline the crc32* wrappers
#[target_feature(enable = "crc")]
pub unsafe fn calculate(crc: u32, data: &[u8]) -> u32 {
    let mut c32 = !crc;
    let (pre_quad, quads, post_quad) = data.align_to::<u64>();

    c32 = pre_quad.iter().fold(c32, |acc, &b| arch::__crc32b(acc, b));

    // unrolling increases performance by a lot
    let mut quad_iter = quads.chunks_exact(8);
    for chunk in &mut quad_iter {
        c32 = arch::__crc32d(c32, chunk[0]);
        c32 = arch::__crc32d(c32, chunk[1]);
        c32 = arch::__crc32d(c32, chunk[2]);
        c32 = arch::__crc32d(c32, chunk[3]);
        c32 = arch::__crc32d(c32, chunk[4]);
        c32 = arch::__crc32d(c32, chunk[5]);
        c32 = arch::__crc32d(c32, chunk[6]);
        c32 = arch::__crc32d(c32, chunk[7]);
    }
    c32 = quad_iter
        .remainder()
        .iter()
        .fold(c32, |acc, &q| arch::__crc32d(acc, q));

    c32 = post_quad.iter().fold(c32, |acc, &b| arch::__crc32b(acc, b));

    !c32
}

#[cfg(test)]
mod test {
    quickcheck::quickcheck! {
        fn check_against_baseline(init: u32, chunks: Vec<(Vec<u8>, usize)>) -> bool {
            let mut baseline = super::super::super::baseline::State::new(init);
            let mut aarch64 = super::State::new(init).expect("not supported");
            for (chunk, mut offset) in chunks {
                // simulate random alignments by offsetting the slice by up to 15 bytes
                offset &= 0xF;
                if chunk.len() <= offset {
                    baseline.update(&chunk);
                    aarch64.update(&chunk);
                } else {
                    baseline.update(&chunk[offset..]);
                    aarch64.update(&chunk[offset..]);
                }
            }
            aarch64.finalize() == baseline.finalize()
        }
    }
}
