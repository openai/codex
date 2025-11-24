// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(feature="serde1")] use serde::{Serialize, Deserialize};
use rand_core::impls::{next_u64_via_u32, fill_bytes_via_next};
use rand_core::le::read_u32_into;
use rand_core::{SeedableRng, RngCore, Error};

/// A xoshiro128++ random number generator.
///
/// The xoshiro128++ algorithm is not suitable for cryptographic purposes, but
/// is very fast and has excellent statistical properties.
///
/// The algorithm used here is translated from [the `xoshiro128plusplus.c`
/// reference source code](http://xoshiro.di.unimi.it/xoshiro128plusplus.c) by
/// David Blackman and Sebastiano Vigna.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature="serde1", derive(Serialize, Deserialize))]
pub struct Xoshiro128PlusPlus {
    s: [u32; 4],
}

impl SeedableRng for Xoshiro128PlusPlus {
    type Seed = [u8; 16];

    /// Create a new `Xoshiro128PlusPlus`.  If `seed` is entirely 0, it will be
    /// mapped to a different seed.
    #[inline]
    fn from_seed(seed: [u8; 16]) -> Xoshiro128PlusPlus {
        if seed.iter().all(|&x| x == 0) {
            return Self::seed_from_u64(0);
        }
        let mut state = [0; 4];
        read_u32_into(&seed, &mut state);
        Xoshiro128PlusPlus { s: state }
    }

    /// Create a new `Xoshiro128PlusPlus` from a `u64` seed.
    ///
    /// This uses the SplitMix64 generator internally.
    fn seed_from_u64(mut state: u64) -> Self {
        const PHI: u64 = 0x9e3779b97f4a7c15;
        let mut seed = Self::Seed::default();
        for chunk in seed.as_mut().chunks_mut(8) {
            state = state.wrapping_add(PHI);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z = z ^ (z >> 31);
            chunk.copy_from_slice(&z.to_le_bytes());
        }
        Self::from_seed(seed)
    }
}

impl RngCore for Xoshiro128PlusPlus {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        let result_starstar = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(7)
            .wrapping_add(self.s[0]);

        let t = self.s[1] << 9;

        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];

        self.s[2] ^= t;

        self.s[3] = self.s[3].rotate_left(11);

        result_starstar
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        next_u64_via_u32(self)
    }

    #[inline]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        fill_bytes_via_next(self, dest);
    }

    #[inline]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference() {
        let mut rng = Xoshiro128PlusPlus::from_seed(
            [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]);
        // These values were produced with the reference implementation:
        // http://xoshiro.di.unimi.it/xoshiro128plusplus.c
        let expected = [
            641, 1573767, 3222811527, 3517856514, 836907274, 4247214768,
            3867114732, 1355841295, 495546011, 621204420,
        ];
        for &e in &expected {
            assert_eq!(rng.next_u32(), e);
        }
    }
}
