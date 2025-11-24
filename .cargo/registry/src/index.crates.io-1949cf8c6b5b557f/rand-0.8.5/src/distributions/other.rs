// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The implementations of the `Standard` distribution for other built-in types.

use core::char;
use core::num::Wrapping;
#[cfg(feature = "alloc")]
use alloc::string::String;

use crate::distributions::{Distribution, Standard, Uniform};
#[cfg(feature = "alloc")]
use crate::distributions::DistString;
use crate::Rng;

#[cfg(feature = "serde1")]
use serde::{Serialize, Deserialize};
#[cfg(feature = "min_const_gen")]
use core::mem::{self, MaybeUninit};


// ----- Sampling distributions -----

/// Sample a `u8`, uniformly distributed over ASCII letters and numbers:
/// a-z, A-Z and 0-9.
///
/// # Example
///
/// ```
/// use rand::{Rng, thread_rng};
/// use rand::distributions::Alphanumeric;
///
/// let mut rng = thread_rng();
/// let chars: String = (0..7).map(|_| rng.sample(Alphanumeric) as char).collect();
/// println!("Random chars: {}", chars);
/// ```
///
/// The [`DistString`] trait provides an easier method of generating
/// a random `String`, and offers more efficient allocation:
/// ```
/// use rand::distributions::{Alphanumeric, DistString};
/// let string = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
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
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
pub struct Alphanumeric;


// ----- Implementations of distributions -----

impl Distribution<char> for Standard {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> char {
        // A valid `char` is either in the interval `[0, 0xD800)` or
        // `(0xDFFF, 0x11_0000)`. All `char`s must therefore be in
        // `[0, 0x11_0000)` but not in the "gap" `[0xD800, 0xDFFF]` which is
        // reserved for surrogates. This is the size of that gap.
        const GAP_SIZE: u32 = 0xDFFF - 0xD800 + 1;

        // Uniform::new(0, 0x11_0000 - GAP_SIZE) can also be used but it
        // seemed slower.
        let range = Uniform::new(GAP_SIZE, 0x11_0000);

        let mut n = range.sample(rng);
        if n <= 0xDFFF {
            n -= GAP_SIZE;
        }
        unsafe { char::from_u32_unchecked(n) }
    }
}

/// Note: the `String` is potentially left with excess capacity; optionally the
/// user may call `string.shrink_to_fit()` afterwards.
#[cfg(feature = "alloc")]
impl DistString for Standard {
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

#[cfg(feature = "alloc")]
impl DistString for Alphanumeric {
    fn append_string<R: Rng + ?Sized>(&self, rng: &mut R, string: &mut String, len: usize) {
        unsafe {
            let v = string.as_mut_vec();
            v.extend(self.sample_iter(rng).take(len));
        }
    }
}

impl Distribution<bool> for Standard {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> bool {
        // We can compare against an arbitrary bit of an u32 to get a bool.
        // Because the least significant bits of a lower quality RNG can have
        // simple patterns, we compare against the most significant bit. This is
        // easiest done using a sign test.
        (rng.next_u32() as i32) < 0
    }
}

macro_rules! tuple_impl {
    // use variables to indicate the arity of the tuple
    ($($tyvar:ident),* ) => {
        // the trailing commas are for the 1 tuple
        impl< $( $tyvar ),* >
            Distribution<( $( $tyvar ),* , )>
            for Standard
            where $( Standard: Distribution<$tyvar> ),*
        {
            #[inline]
            fn sample<R: Rng + ?Sized>(&self, _rng: &mut R) -> ( $( $tyvar ),* , ) {
                (
                    // use the $tyvar's to get the appropriate number of
                    // repeats (they're not actually needed)
                    $(
                        _rng.gen::<$tyvar>()
                    ),*
                    ,
                )
            }
        }
    }
}

impl Distribution<()> for Standard {
    #[allow(clippy::unused_unit)]
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, _: &mut R) -> () {
        ()
    }
}
tuple_impl! {A}
tuple_impl! {A, B}
tuple_impl! {A, B, C}
tuple_impl! {A, B, C, D}
tuple_impl! {A, B, C, D, E}
tuple_impl! {A, B, C, D, E, F}
tuple_impl! {A, B, C, D, E, F, G}
tuple_impl! {A, B, C, D, E, F, G, H}
tuple_impl! {A, B, C, D, E, F, G, H, I}
tuple_impl! {A, B, C, D, E, F, G, H, I, J}
tuple_impl! {A, B, C, D, E, F, G, H, I, J, K}
tuple_impl! {A, B, C, D, E, F, G, H, I, J, K, L}

#[cfg(feature = "min_const_gen")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "min_const_gen")))]
impl<T, const N: usize> Distribution<[T; N]> for Standard
where Standard: Distribution<T>
{
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, _rng: &mut R) -> [T; N] {
        let mut buff: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };

        for elem in &mut buff {
            *elem = MaybeUninit::new(_rng.gen());
        }

        unsafe { mem::transmute_copy::<_, _>(&buff) }
    }
}

#[cfg(not(feature = "min_const_gen"))]
macro_rules! array_impl {
    // recursive, given at least one type parameter:
    {$n:expr, $t:ident, $($ts:ident,)*} => {
        array_impl!{($n - 1), $($ts,)*}

        impl<T> Distribution<[T; $n]> for Standard where Standard: Distribution<T> {
            #[inline]
            fn sample<R: Rng + ?Sized>(&self, _rng: &mut R) -> [T; $n] {
                [_rng.gen::<$t>(), $(_rng.gen::<$ts>()),*]
            }
        }
    };
    // empty case:
    {$n:expr,} => {
        impl<T> Distribution<[T; $n]> for Standard {
            fn sample<R: Rng + ?Sized>(&self, _rng: &mut R) -> [T; $n] { [] }
        }
    };
}

#[cfg(not(feature = "min_const_gen"))]
array_impl! {32, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T,}

impl<T> Distribution<Option<T>> for Standard
where Standard: Distribution<T>
{
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<T> {
        // UFCS is needed here: https://github.com/rust-lang/rust/issues/24066
        if rng.gen::<bool>() {
            Some(rng.gen())
        } else {
            None
        }
    }
}

impl<T> Distribution<Wrapping<T>> for Standard
where Standard: Distribution<T>
{
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Wrapping<T> {
        Wrapping(rng.gen())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::RngCore;
    #[cfg(feature = "alloc")] use alloc::string::String;

    #[test]
    fn test_misc() {
        let rng: &mut dyn RngCore = &mut crate::test::rng(820);

        rng.sample::<char, _>(Standard);
        rng.sample::<bool, _>(Standard);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_chars() {
        use core::iter;
        let mut rng = crate::test::rng(805);

        // Test by generating a relatively large number of chars, so we also
        // take the rejection sampling path.
        let word: String = iter::repeat(())
            .map(|()| rng.gen::<char>())
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
            incorrect |= !(('0'..='9').contains(&c) ||
                           ('A'..='Z').contains(&c) ||
                           ('a'..='z').contains(&c) );
        }
        assert!(!incorrect);
    }

    #[test]
    fn value_stability() {
        fn test_samples<T: Copy + core::fmt::Debug + PartialEq, D: Distribution<T>>(
            distr: &D, zero: T, expected: &[T],
        ) {
            let mut rng = crate::test::rng(807);
            let mut buf = [zero; 5];
            for x in &mut buf {
                *x = rng.sample(&distr);
            }
            assert_eq!(&buf, expected);
        }

        test_samples(&Standard, 'a', &[
            '\u{8cdac}',
            '\u{a346a}',
            '\u{80120}',
            '\u{ed692}',
            '\u{35888}',
        ]);
        test_samples(&Alphanumeric, 0, &[104, 109, 101, 51, 77]);
        test_samples(&Standard, false, &[true, true, false, true, false]);
        test_samples(&Standard, None as Option<bool>, &[
            Some(true),
            None,
            Some(false),
            None,
            Some(false),
        ]);
        test_samples(&Standard, Wrapping(0i32), &[
            Wrapping(-2074640887),
            Wrapping(-1719949321),
            Wrapping(2018088303),
            Wrapping(-547181756),
            Wrapping(838957336),
        ]);

        // We test only sub-sets of tuple and array impls
        test_samples(&Standard, (), &[(), (), (), (), ()]);
        test_samples(&Standard, (false,), &[
            (true,),
            (true,),
            (false,),
            (true,),
            (false,),
        ]);
        test_samples(&Standard, (false, false), &[
            (true, true),
            (false, true),
            (false, false),
            (true, false),
            (false, false),
        ]);

        test_samples(&Standard, [0u8; 0], &[[], [], [], [], []]);
        test_samples(&Standard, [0u8; 3], &[
            [9, 247, 111],
            [68, 24, 13],
            [174, 19, 194],
            [172, 69, 213],
            [149, 207, 29],
        ]);
    }
}
