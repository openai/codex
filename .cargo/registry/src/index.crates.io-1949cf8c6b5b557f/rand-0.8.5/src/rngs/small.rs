// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A small fast RNG

use rand_core::{Error, RngCore, SeedableRng};

#[cfg(target_pointer_width = "64")]
type Rng = super::xoshiro256plusplus::Xoshiro256PlusPlus;
#[cfg(not(target_pointer_width = "64"))]
type Rng = super::xoshiro128plusplus::Xoshiro128PlusPlus;

/// A small-state, fast non-crypto PRNG
///
/// `SmallRng` may be a good choice when a PRNG with small state, cheap
/// initialization, good statistical quality and good performance are required.
/// Note that depending on the application, [`StdRng`] may be faster on many
/// modern platforms while providing higher-quality randomness. Furthermore,
/// `SmallRng` is **not** a good choice when:
/// - Security against prediction is important. Use [`StdRng`] instead.
/// - Seeds with many zeros are provided. In such cases, it takes `SmallRng`
///   about 10 samples to produce 0 and 1 bits with equal probability. Either
///   provide seeds with an approximately equal number of 0 and 1 (for example
///   by using [`SeedableRng::from_entropy`] or [`SeedableRng::seed_from_u64`]),
///   or use [`StdRng`] instead.
///
/// The algorithm is deterministic but should not be considered reproducible
/// due to dependence on platform and possible replacement in future
/// library versions. For a reproducible generator, use a named PRNG from an
/// external crate, e.g. [rand_xoshiro] or [rand_chacha].
/// Refer also to [The Book](https://rust-random.github.io/book/guide-rngs.html).
///
/// The PRNG algorithm in `SmallRng` is chosen to be efficient on the current
/// platform, without consideration for cryptography or security. The size of
/// its state is much smaller than [`StdRng`]. The current algorithm is
/// `Xoshiro256PlusPlus` on 64-bit platforms and `Xoshiro128PlusPlus` on 32-bit
/// platforms. Both are also implemented by the [rand_xoshiro] crate.
///
/// # Examples
///
/// Initializing `SmallRng` with a random seed can be done using [`SeedableRng::from_entropy`]:
///
/// ```
/// use rand::{Rng, SeedableRng};
/// use rand::rngs::SmallRng;
///
/// // Create small, cheap to initialize and fast RNG with a random seed.
/// // The randomness is supplied by the operating system.
/// let mut small_rng = SmallRng::from_entropy();
/// # let v: u32 = small_rng.gen();
/// ```
///
/// When initializing a lot of `SmallRng`'s, using [`thread_rng`] can be more
/// efficient:
///
/// ```
/// use rand::{SeedableRng, thread_rng};
/// use rand::rngs::SmallRng;
///
/// // Create a big, expensive to initialize and slower, but unpredictable RNG.
/// // This is cached and done only once per thread.
/// let mut thread_rng = thread_rng();
/// // Create small, cheap to initialize and fast RNGs with random seeds.
/// // One can generally assume this won't fail.
/// let rngs: Vec<SmallRng> = (0..10)
///     .map(|_| SmallRng::from_rng(&mut thread_rng).unwrap())
///     .collect();
/// ```
///
/// [`StdRng`]: crate::rngs::StdRng
/// [`thread_rng`]: crate::thread_rng
/// [rand_chacha]: https://crates.io/crates/rand_chacha
/// [rand_xoshiro]: https://crates.io/crates/rand_xoshiro
#[cfg_attr(doc_cfg, doc(cfg(feature = "small_rng")))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmallRng(Rng);

impl RngCore for SmallRng {
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

impl SeedableRng for SmallRng {
    type Seed = <Rng as SeedableRng>::Seed;

    #[inline(always)]
    fn from_seed(seed: Self::Seed) -> Self {
        SmallRng(Rng::from_seed(seed))
    }

    #[inline(always)]
    fn from_rng<R: RngCore>(rng: R) -> Result<Self, Error> {
        Rng::from_rng(rng).map(SmallRng)
    }
}
