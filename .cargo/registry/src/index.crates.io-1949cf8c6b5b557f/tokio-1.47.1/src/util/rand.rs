cfg_rt! {
    mod rt;
    pub(crate) use rt::RngSeedGenerator;

    cfg_unstable! {
        mod rt_unstable;
    }
}

/// A seed for random number generation.
///
/// In order to make certain functions within a runtime deterministic, a seed
/// can be specified at the time of creation.
#[allow(unreachable_pub)]
#[derive(Clone, Debug)]
pub struct RngSeed {
    s: u32,
    r: u32,
}

/// Fast random number generate.
///
/// Implement `xorshift64+`: 2 32-bit `xorshift` sequences added together.
/// Shift triplet `[17,7,16]` was calculated as indicated in Marsaglia's
/// `Xorshift` paper: <https://www.jstatsoft.org/article/view/v008i14/xorshift.pdf>
/// This generator passes the SmallCrush suite, part of TestU01 framework:
/// <http://simul.iro.umontreal.ca/testu01/tu01.html>
#[derive(Clone, Copy, Debug)]
pub(crate) struct FastRand {
    one: u32,
    two: u32,
}

impl RngSeed {
    /// Creates a random seed using loom internally.
    pub(crate) fn new() -> Self {
        Self::from_u64(crate::loom::rand::seed())
    }

    fn from_u64(seed: u64) -> Self {
        let one = (seed >> 32) as u32;
        let mut two = seed as u32;

        if two == 0 {
            // This value cannot be zero
            two = 1;
        }

        Self::from_pair(one, two)
    }

    fn from_pair(s: u32, r: u32) -> Self {
        Self { s, r }
    }
}

impl FastRand {
    /// Initialize a new fast random number generator using the default source of entropy.
    pub(crate) fn new() -> FastRand {
        FastRand::from_seed(RngSeed::new())
    }

    /// Initializes a new, thread-local, fast random number generator.
    pub(crate) fn from_seed(seed: RngSeed) -> FastRand {
        FastRand {
            one: seed.s,
            two: seed.r,
        }
    }

    #[cfg(any(
        feature = "macros",
        feature = "rt-multi-thread",
        all(feature = "sync", feature = "rt")
    ))]
    pub(crate) fn fastrand_n(&mut self, n: u32) -> u32 {
        // This is similar to fastrand() % n, but faster.
        // See https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
        let mul = (self.fastrand() as u64).wrapping_mul(n as u64);
        (mul >> 32) as u32
    }

    fn fastrand(&mut self) -> u32 {
        let mut s1 = self.one;
        let s0 = self.two;

        s1 ^= s1 << 17;
        s1 = s1 ^ s0 ^ s1 >> 7 ^ s0 >> 16;

        self.one = s0;
        self.two = s1;

        s0.wrapping_add(s1)
    }
}
