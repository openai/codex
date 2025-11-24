// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rand_core::impls::fill_bytes_via_next;
use rand_core::le::read_u64_into;
use rand_core::{RngCore, SeedableRng};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A xoshiro256++ random number generator.
///
/// The xoshiro256++ algorithm is not suitable for cryptographic purposes, but
/// is very fast and has excellent statistical properties.
///
/// The algorithm used here is translated from [the `xoshiro256plusplus.c`
/// reference source code](http://xoshiro.di.unimi.it/xoshiro256plusplus.c) by
/// David Blackman and Sebastiano Vigna.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Xoshiro256PlusPlus {
    s: [u64; 4],
}

impl SeedableRng for Xoshiro256PlusPlus {
    type Seed = [u8; 32];

    /// Create a new `Xoshiro256PlusPlus`.  If `seed` is entirely 0, it will be
    /// mapped to a different seed.
    #[inline]
    fn from_seed(seed: [u8; 32]) -> Xoshiro256PlusPlus {
        let mut state = [0; 4];
        read_u64_into(&seed, &mut state);
        // Check for zero on aligned integers for better code generation.
        // Furtermore, seed_from_u64(0) will expand to a constant when optimized.
        if state.iter().all(|&x| x == 0) {
            return Self::seed_from_u64(0);
        }
        Xoshiro256PlusPlus { s: state }
    }

    /// Create a new `Xoshiro256PlusPlus` from a `u64` seed.
    ///
    /// This uses the SplitMix64 generator internally.
    #[inline]
    fn seed_from_u64(mut state: u64) -> Self {
        const PHI: u64 = 0x9e3779b97f4a7c15;
        let mut s = [0; 4];
        for i in s.iter_mut() {
            state = state.wrapping_add(PHI);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z = z ^ (z >> 31);
            *i = z;
        }
        // By using a non-zero PHI we are guaranteed to generate a non-zero state
        // Thus preventing a recursion between from_seed and seed_from_u64.
        debug_assert_ne!(s, [0; 4]);
        Xoshiro256PlusPlus { s }
    }
}

impl RngCore for Xoshiro256PlusPlus {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        // The lowest bits have some linear dependencies, so we use the
        // upper bits instead.
        let val = self.next_u64();
        (val >> 32) as u32
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let res = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);

        let t = self.s[1] << 17;

        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];

        self.s[2] ^= t;

        self.s[3] = self.s[3].rotate_left(45);

        res
    }

    #[inline]
    fn fill_bytes(&mut self, dst: &mut [u8]) {
        fill_bytes_via_next(self, dst)
    }
}

#[cfg(test)]
mod tests {
    use super::Xoshiro256PlusPlus;
    use rand_core::{RngCore, SeedableRng};

    #[test]
    fn reference() {
        let mut rng = Xoshiro256PlusPlus::from_seed([
            1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0,
            0, 0, 0,
        ]);
        // These values were produced with the reference implementation:
        // http://xoshiro.di.unimi.it/xoshiro256plusplus.c
        let expected = [
            41943041,
            58720359,
            3588806011781223,
            3591011842654386,
            9228616714210784205,
            9973669472204895162,
            14011001112246962877,
            12406186145184390807,
            15849039046786891736,
            10450023813501588000,
        ];
        for &e in &expected {
            assert_eq!(rng.next_u64(), e);
        }
    }

    #[test]
    fn stable_seed_from_u64_and_from_seed() {
        // We don't guarantee value-stability for SmallRng but this
        // could influence keeping stability whenever possible (e.g. after optimizations).
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(0);
        // from_seed([0; 32]) should produce the same state as seed_from_u64(0).
        let mut rng_from_seed_0 = Xoshiro256PlusPlus::from_seed([0; 32]);
        let expected = [
            5987356902031041503,
            7051070477665621255,
            6633766593972829180,
            211316841551650330,
            9136120204379184874,
            379361710973160858,
            15813423377499357806,
            15596884590815070553,
            5439680534584881407,
            1369371744833522710,
        ];
        for &e in &expected {
            assert_eq!(rng.next_u64(), e);
            assert_eq!(rng_from_seed_0.next_u64(), e);
        }
    }
}
