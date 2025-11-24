// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A small fast RNG

use rand_core::{RngCore, SeedableRng};

#[cfg(any(target_pointer_width = "32", target_pointer_width = "16"))]
type Rng = super::xoshiro128plusplus::Xoshiro128PlusPlus;
#[cfg(target_pointer_width = "64")]
type Rng = super::xoshiro256plusplus::Xoshiro256PlusPlus;

/// A small-state, fast, non-crypto, non-portable PRNG
///
/// This is the "standard small" RNG, a generator with the following properties:
///
/// - Non-[portable]: any future library version may replace the algorithm
///   and results may be platform-dependent.
///   (For a small portable generator, use the [rand_pcg] or [rand_xoshiro] crate.)
/// - Non-cryptographic: output is easy to predict (insecure)
/// - [Quality]: statistically good quality
/// - Fast: the RNG is fast for both bulk generation and single values, with
///   consistent cost of method calls
/// - Fast initialization
/// - Small state: little memory usage (current state size is 16-32 bytes
///   depending on platform)
///
/// The current algorithm is
/// `Xoshiro256PlusPlus` on 64-bit platforms and `Xoshiro128PlusPlus` on 32-bit
/// platforms. Both are also implemented by the [rand_xoshiro] crate.
///
/// ## Seeding (construction)
///
/// This generator implements the [`SeedableRng`] trait. All methods are
/// suitable for seeding, but note that, even with a fixed seed, output is not
/// [portable]. Some suggestions:
///
/// 1.  To automatically seed with a unique seed, use [`SeedableRng::from_rng`]:
///     ```
///     use rand::SeedableRng;
///     use rand::rngs::SmallRng;
///     let rng = SmallRng::from_rng(&mut rand::rng());
///     # let _: SmallRng = rng;
///     ```
///     or [`SeedableRng::from_os_rng`]:
///     ```
///     # use rand::SeedableRng;
///     # use rand::rngs::SmallRng;
///     let rng = SmallRng::from_os_rng();
///     # let _: SmallRng = rng;
///     ```
/// 2.  To use a deterministic integral seed, use `seed_from_u64`. This uses a
///     hash function internally to yield a (typically) good seed from any
///     input.
///     ```
///     # use rand::{SeedableRng, rngs::SmallRng};
///     let rng = SmallRng::seed_from_u64(1);
///     # let _: SmallRng = rng;
///     ```
/// 3.  To seed deterministically from text or other input, use [`rand_seeder`].
///
/// See also [Seeding RNGs] in the book.
///
/// ## Generation
///
/// The generators implements [`RngCore`] and thus also [`Rng`][crate::Rng].
/// See also the [Random Values] chapter in the book.
///
/// [portable]: https://rust-random.github.io/book/crate-reprod.html
/// [Seeding RNGs]: https://rust-random.github.io/book/guide-seeding.html
/// [Random Values]: https://rust-random.github.io/book/guide-values.html
/// [Quality]: https://rust-random.github.io/book/guide-rngs.html#quality
/// [`StdRng`]: crate::rngs::StdRng
/// [rand_pcg]: https://crates.io/crates/rand_pcg
/// [rand_xoshiro]: https://crates.io/crates/rand_xoshiro
/// [`rand_chacha::ChaCha8Rng`]: https://docs.rs/rand_chacha/latest/rand_chacha/struct.ChaCha8Rng.html
/// [`rand_seeder`]: https://docs.rs/rand_seeder/latest/rand_seeder/
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmallRng(Rng);

impl SeedableRng for SmallRng {
    // Fix to 256 bits. Changing this is a breaking change!
    type Seed = [u8; 32];

    #[inline(always)]
    fn from_seed(seed: Self::Seed) -> Self {
        // This is for compatibility with 32-bit platforms where Rng::Seed has a different seed size
        // With MSRV >= 1.77: let seed = *seed.first_chunk().unwrap()
        const LEN: usize = core::mem::size_of::<<Rng as SeedableRng>::Seed>();
        let seed = (&seed[..LEN]).try_into().unwrap();
        SmallRng(Rng::from_seed(seed))
    }

    #[inline(always)]
    fn seed_from_u64(state: u64) -> Self {
        SmallRng(Rng::seed_from_u64(state))
    }
}

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
        self.0.fill_bytes(dest)
    }
}
