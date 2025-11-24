// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The standard RNG

use crate::{CryptoRng, Error, RngCore, SeedableRng};

pub(crate) use rand_chacha::ChaCha12Core as Core;

use rand_chacha::ChaCha12Rng as Rng;

/// The standard RNG. The PRNG algorithm in `StdRng` is chosen to be efficient
/// on the current platform, to be statistically strong and unpredictable
/// (meaning a cryptographically secure PRNG).
///
/// The current algorithm used is the ChaCha block cipher with 12 rounds. Please
/// see this relevant [rand issue] for the discussion. This may change as new 
/// evidence of cipher security and performance becomes available.
///
/// The algorithm is deterministic but should not be considered reproducible
/// due to dependence on configuration and possible replacement in future
/// library versions. For a secure reproducible generator, we recommend use of
/// the [rand_chacha] crate directly.
///
/// [rand_chacha]: https://crates.io/crates/rand_chacha
/// [rand issue]: https://github.com/rust-random/rand/issues/932
#[cfg_attr(doc_cfg, doc(cfg(feature = "std_rng")))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StdRng(Rng);

impl RngCore for StdRng {
    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    #[inline(always)]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest);
    }

    #[inline(always)]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.0.try_fill_bytes(dest)
    }
}

impl SeedableRng for StdRng {
    type Seed = <Rng as SeedableRng>::Seed;

    #[inline(always)]
    fn from_seed(seed: Self::Seed) -> Self {
        StdRng(Rng::from_seed(seed))
    }

    #[inline(always)]
    fn from_rng<R: RngCore>(rng: R) -> Result<Self, Error> {
        Rng::from_rng(rng).map(StdRng)
    }
}

impl CryptoRng for StdRng {}


#[cfg(test)]
mod test {
    use crate::rngs::StdRng;
    use crate::{RngCore, SeedableRng};

    #[test]
    fn test_stdrng_construction() {
        // Test value-stability of StdRng. This is expected to break any time
        // the algorithm is changed.
        #[rustfmt::skip]
        let seed = [1,0,0,0, 23,0,0,0, 200,1,0,0, 210,30,0,0,
                    0,0,0,0, 0,0,0,0, 0,0,0,0, 0,0,0,0];

        let target = [10719222850664546238, 14064965282130556830];

        let mut rng0 = StdRng::from_seed(seed);
        let x0 = rng0.next_u64();

        let mut rng1 = StdRng::from_rng(rng0).unwrap();
        let x1 = rng1.next_u64();

        assert_eq!([x0, x1], target);
    }
}
