// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The implementations of the `StandardUniform` distribution for other built-in types.

#[cfg(feature = "alloc")]
use alloc::string::String;
use core::array;
use core::char;
use core::num::Wrapping;

#[cfg(feature = "alloc")]
use crate::distr::SampleString;
use crate::distr::{Distribution, StandardUniform, Uniform};
use crate::Rng;

#[cfg(feature = "simd_support")]
use core::simd::prelude::*;
#[cfg(feature = "simd_support")]
use core::simd::{LaneCount, MaskElement, SupportedLaneCount};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ----- Sampling distributions -----

/// Sample a `u8`, uniformly distributed over ASCII letters and numbers:
/// a-z, A-Z and 0-9.
///
/// # Example
///
/// ```
/// use rand::Rng;
/// use rand::distr::Alphanumeric;
///
/// let mut rng = rand::rng();
/// let chars: String = (0..7).map(|_| rng.sample(Alphanumeric) as char).collect();
/// println!("Random chars: {}", chars);
/// ```
///
/// The [`SampleString`] trait provides an easier method of generating
/// a random [`String`], and offers more efficient allocation:
/// ```
/// use rand::distr::{Alphanumeric, SampleString};
/// let string = Alphanumeric.sample_string(&mut rand::rng(), 16);
/// println!("Random string: {}", string);
/// ```
///
/// # Passwords
///
/// Users sometimes ask whether it is safe to use a string of random characters
/// as a password. In principle, all RNGs in Rand implementing `CryptoRng` are
/// suitable as a source of randomness for generating passwords (if they are
/// properly seeded), but it is more conservative to only use randomness
/// directly from the operating system via the `getrandom` crate, or the
/// corresponding bindings of a crypto library.
///
/// When generating passwords or keys, it is important to consider the threat
/// model and in some cases the memorability of the password. This is out of
/// scope of the Rand project, and therefore we defer to the following
/// references:
///
/// - [Wikipedia article on Password Strength](https://en.wikipedia.org/wiki/Password_strength)
/// - [Diceware for generating memorable passwords](https://en.wikipedia.org/wiki/Diceware)
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Alphanumeric;

/// Sample a [`u8`], uniformly distributed over letters:
/// a-z and A-Z.
///
/// # Example
///
/// You're able to generate random Alphabetic characters via mapping or via the
/// [`SampleString::sample_string`] method like so:
///
/// ```
/// use rand::Rng;
/// use rand::distr::{Alphabetic, SampleString};
///
/// // Manual mapping
/// let mut rng = rand::rng();
/// let chars: String = (0..7).map(|_| rng.sample(Alphabetic) as char).collect();
/// println!("Random chars: {}", chars);
///
/// // Using [`SampleString::sample_string`]
/// let string = Alphabetic.sample_string(&mut rand::rng(), 16);
/// println!("Random string: {}", string);
/// ```
///
/// # Passwords
///
/// Refer to [`Alphanumeric#Passwords`].
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Alphabetic;

// ----- Implementations of distributions -----

impl Distribution<char> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> char {
        // A valid `char` is either in the interval `[0, 0xD800)` or
        // `(0xDFFF, 0x11_0000)`. All `char`s must therefore be in
        // `[0, 0x11_0000)` but not in the "gap" `[0xD800, 0xDFFF]` which is
        // reserved for surrogates. This is the size of that gap.
        const GAP_SIZE: u32 = 0xDFFF - 0xD800 + 1;

        // Uniform::new(0, 0x11_0000 - GAP_SIZE) can also be used, but it
        // seemed slower.
        let range = Uniform::new(GAP_SIZE, 0x11_0000).unwrap();

        let mut n = range.sample(rng);
        if n <= 0xDFFF {
            n -= GAP_SIZE;
        }
        // SAFETY: We ensure above that `n` represents a `char`.
        unsafe { char::from_u32_unchecked(n) }
    }
}

#[cfg(feature = "alloc")]
impl SampleString for StandardUniform {
    fn append_string<R: Rng + ?Sized>(&self, rng: &mut R, s: &mut String, len: usize) {
        // A char is encoded with at most four bytes, thus this reservation is
        // guaranteed to be sufficient. We do not shrink_to_fit afterwards so
        // that repeated usage on the same `String` buffer does not reallocate.
        s.reserve(4 * len);
        s.extend(Distribution::<char>::sample_iter(self, rng).take(len));
    }
}

impl Distribution<u8> for Alphanumeric {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> u8 {
        const RANGE: u32 = 26 + 26 + 10;
        const GEN_ASCII_STR_CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                abcdefghijklmnopqrstuvwxyz\
                0123456789";
        // We can pick from 62 characters. This is so close to a power of 2, 64,
        // that we can do better than `Uniform`. Use a simple bitshift and
        // rejection sampling. We do not use a bitmask, because for small RNGs
        // the most significant bits are usually of higher quality.
        loop {
            let var = rng.next_u32() >> (32 - 6);
            if var < RANGE {
                return GEN_ASCII_STR_CHARSET[var as usize];
            }
        }
    }
}

impl Distribution<u8> for Alphabetic {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> u8 {
        const RANGE: u8 = 26 + 26;

        let offset = rng.random_range(0..RANGE) + b'A';

        // Account for upper-cases
        offset + (offset > b'Z') as u8 * (b'a' - b'Z' - 1)
    }
}

#[cfg(feature = "alloc")]
impl SampleString for Alphanumeric {
    fn append_string<R: Rng + ?Sized>(&self, rng: &mut R, string: &mut String, len: usize) {
        // SAFETY: `self` only samples alphanumeric characters, which are valid UTF-8.
        unsafe {
            let v = string.as_mut_vec();
            v.extend(
                self.sample_iter(rng)
                    .take(len)
                    .inspect(|b| debug_assert!(b.is_ascii_alphanumeric())),
            );
        }
    }
}

#[cfg(feature = "alloc")]
impl SampleString for Alphabetic {
    fn append_string<R: Rng + ?Sized>(&self, rng: &mut R, string: &mut String, len: usize) {
        // SAFETY: With this distribution we guarantee that we're working with valid ASCII
        // characters.
        // See [#1590](https://github.com/rust-random/rand/issues/1590).
        unsafe {
            let v = string.as_mut_vec();
            v.reserve_exact(len);
            v.extend(self.sample_iter(rng).take(len));
        }
    }
}

impl Distribution<bool> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> bool {
        // We can compare against an arbitrary bit of an u32 to get a bool.
        // Because the least significant bits of a lower quality RNG can have
        // simple patterns, we compare against the most significant bit. This is
        // easiest done using a sign test.
        (rng.next_u32() as i32) < 0
    }
}

/// Note that on some hardware like x86/64 mask operations like [`_mm_blendv_epi8`]
/// only care about a single bit. This means that you could use uniform random bits
/// directly:
///
/// ```ignore
/// // this may be faster...
/// let x = unsafe { _mm_blendv_epi8(a.into(), b.into(), rng.random::<__m128i>()) };
///
/// // ...than this
/// let x = rng.random::<mask8x16>().select(b, a);
/// ```
///
/// Since most bits are unused you could also generate only as many bits as you need, i.e.:
/// ```
/// #![feature(portable_simd)]
/// use std::simd::prelude::*;
/// use rand::prelude::*;
/// let mut rng = rand::rng();
///
/// let x = u16x8::splat(rng.random::<u8>() as u16);
/// let mask = u16x8::splat(1) << u16x8::from([0, 1, 2, 3, 4, 5, 6, 7]);
/// let rand_mask = (x & mask).simd_eq(mask);
/// ```
///
/// [`_mm_blendv_epi8`]: https://www.intel.com/content/www/us/en/docs/intrinsics-guide/index.html#text=_mm_blendv_epi8&ig_expand=514/
/// [`simd_support`]: https://github.com/rust-random/rand#crate-features
#[cfg(feature = "simd_support")]
impl<T, const LANES: usize> Distribution<Mask<T, LANES>> for StandardUniform
where
    T: MaskElement + Default,
    LaneCount<LANES>: SupportedLaneCount,
    StandardUniform: Distribution<Simd<T, LANES>>,
    Simd<T, LANES>: SimdPartialOrd<Mask = Mask<T, LANES>>,
{
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Mask<T, LANES> {
        // `MaskElement` must be a signed integer, so this is equivalent
        // to the scalar `i32 < 0` method
        let var = rng.random::<Simd<T, LANES>>();
        var.simd_lt(Simd::default())
    }
}

/// Implement `Distribution<(A, B, C, ...)> for StandardUniform`, using the list of
/// identifiers
macro_rules! tuple_impl {
    ($($tyvar:ident)*) => {
        impl< $($tyvar,)* > Distribution<($($tyvar,)*)> for StandardUniform
        where $(
            StandardUniform: Distribution< $tyvar >,
        )*
        {
            #[inline]
            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> ( $($tyvar,)* ) {
                let out = ($(
                    // use the $tyvar's to get the appropriate number of
                    // repeats (they're not actually needed)
                    rng.random::<$tyvar>()
                ,)*);

                // Suppress the unused variable warning for empty tuple
                let _rng = rng;

                out
            }
        }
    }
}

/// Looping wrapper for `tuple_impl`. Given (A, B, C), it also generates
/// implementations for (A, B) and (A,)
macro_rules! tuple_impls {
    ($($tyvar:ident)*) => {tuple_impls!{[] $($tyvar)*}};

    ([$($prefix:ident)*] $head:ident $($tail:ident)*) => {
        tuple_impl!{$($prefix)*}
        tuple_impls!{[$($prefix)* $head] $($tail)*}
    };


    ([$($prefix:ident)*]) => {
        tuple_impl!{$($prefix)*}
    };

}

tuple_impls! {A B C D E F G H I J K L}

impl<T, const N: usize> Distribution<[T; N]> for StandardUniform
where
    StandardUniform: Distribution<T>,
{
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> [T; N] {
        array::from_fn(|_| rng.random())
    }
}

impl<T> Distribution<Wrapping<T>> for StandardUniform
where
    StandardUniform: Distribution<T>,
{
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Wrapping<T> {
        Wrapping(rng.random())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RngCore;

    #[test]
    fn test_misc() {
        let rng: &mut dyn RngCore = &mut crate::test::rng(820);

        rng.sample::<char, _>(StandardUniform);
        rng.sample::<bool, _>(StandardUniform);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_chars() {
        use core::iter;
        let mut rng = crate::test::rng(805);

        // Test by generating a relatively large number of chars, so we also
        // take the rejection sampling path.
        let word: String = iter::repeat(())
            .map(|()| rng.random::<char>())
            .take(1000)
            .collect();
        assert!(!word.is_empty());
    }

    #[test]
    fn test_alphanumeric() {
        let mut rng = crate::test::rng(806);

        // Test by generating a relatively large number of chars, so we also
        // take the rejection sampling path.
        let mut incorrect = false;
        for _ in 0..100 {
            let c: char = rng.sample(Alphanumeric).into();
            incorrect |= !c.is_ascii_alphanumeric();
        }
        assert!(!incorrect);
    }

    #[test]
    fn test_alphabetic() {
        let mut rng = crate::test::rng(806);

        // Test by generating a relatively large number of chars, so we also
        // take the rejection sampling path.
        let mut incorrect = false;
        for _ in 0..100 {
            let c: char = rng.sample(Alphabetic).into();
            incorrect |= !c.is_ascii_alphabetic();
        }
        assert!(!incorrect);
    }

    #[test]
    fn value_stability() {
        fn test_samples<T: Copy + core::fmt::Debug + PartialEq, D: Distribution<T>>(
            distr: &D,
            zero: T,
            expected: &[T],
        ) {
            let mut rng = crate::test::rng(807);
            let mut buf = [zero; 5];
            for x in &mut buf {
                *x = rng.sample(distr);
            }
            assert_eq!(&buf, expected);
        }

        test_samples(
            &StandardUniform,
            'a',
            &[
                '\u{8cdac}',
                '\u{a346a}',
                '\u{80120}',
                '\u{ed692}',
                '\u{35888}',
            ],
        );
        test_samples(&Alphanumeric, 0, &[104, 109, 101, 51, 77]);
        test_samples(&Alphabetic, 0, &[97, 102, 89, 116, 75]);
        test_samples(&StandardUniform, false, &[true, true, false, true, false]);
        test_samples(
            &StandardUniform,
            Wrapping(0i32),
            &[
                Wrapping(-2074640887),
                Wrapping(-1719949321),
                Wrapping(2018088303),
                Wrapping(-547181756),
                Wrapping(838957336),
            ],
        );

        // We test only sub-sets of tuple and array impls
        test_samples(&StandardUniform, (), &[(), (), (), (), ()]);
        test_samples(
            &StandardUniform,
            (false,),
            &[(true,), (true,), (false,), (true,), (false,)],
        );
        test_samples(
            &StandardUniform,
            (false, false),
            &[
                (true, true),
                (false, true),
                (false, false),
                (true, false),
                (false, false),
            ],
        );

        test_samples(&StandardUniform, [0u8; 0], &[[], [], [], [], []]);
        test_samples(
            &StandardUniform,
            [0u8; 3],
            &[
                [9, 247, 111],
                [68, 24, 13],
                [174, 19, 194],
                [172, 69, 213],
                [149, 207, 29],
            ],
        );
    }
}
