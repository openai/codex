// Copyright 2018 Developers of the Rand project.
// Copyright 2013-2017 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Distribution trait and associates

use crate::Rng;
#[cfg(feature = "alloc")]
use alloc::string::String;
use core::iter;

/// Types (distributions) that can be used to create a random instance of `T`.
///
/// It is possible to sample from a distribution through both the
/// `Distribution` and [`Rng`] traits, via `distr.sample(&mut rng)` and
/// `rng.sample(distr)`. They also both offer the [`sample_iter`] method, which
/// produces an iterator that samples from the distribution.
///
/// All implementations are expected to be immutable; this has the significant
/// advantage of not needing to consider thread safety, and for most
/// distributions efficient state-less sampling algorithms are available.
///
/// Implementations are typically expected to be portable with reproducible
/// results when used with a PRNG with fixed seed; see the
/// [portability chapter](https://rust-random.github.io/book/portability.html)
/// of The Rust Rand Book. In some cases this does not apply, e.g. the `usize`
/// type requires different sampling on 32-bit and 64-bit machines.
///
/// [`sample_iter`]: Distribution::sample_iter
pub trait Distribution<T> {
    /// Generate a random value of `T`, using `rng` as the source of randomness.
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> T;

    /// Create an iterator that generates random values of `T`, using `rng` as
    /// the source of randomness.
    ///
    /// Note that this function takes `self` by value. This works since
    /// `Distribution<T>` is impl'd for `&D` where `D: Distribution<T>`,
    /// however borrowing is not automatic hence `distr.sample_iter(...)` may
    /// need to be replaced with `(&distr).sample_iter(...)` to borrow or
    /// `(&*distr).sample_iter(...)` to reborrow an existing reference.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::distr::{Distribution, Alphanumeric, Uniform, StandardUniform};
    ///
    /// let mut rng = rand::rng();
    ///
    /// // Vec of 16 x f32:
    /// let v: Vec<f32> = StandardUniform.sample_iter(&mut rng).take(16).collect();
    ///
    /// // String:
    /// let s: String = Alphanumeric
    ///     .sample_iter(&mut rng)
    ///     .take(7)
    ///     .map(char::from)
    ///     .collect();
    ///
    /// // Dice-rolling:
    /// let die_range = Uniform::new_inclusive(1, 6).unwrap();
    /// let mut roll_die = die_range.sample_iter(&mut rng);
    /// while roll_die.next().unwrap() != 6 {
    ///     println!("Not a 6; rolling again!");
    /// }
    /// ```
    fn sample_iter<R>(self, rng: R) -> Iter<Self, R, T>
    where
        R: Rng,
        Self: Sized,
    {
        Iter {
            distr: self,
            rng,
            phantom: core::marker::PhantomData,
        }
    }

    /// Map sampled values to type `S`
    ///
    /// # Example
    ///
    /// ```
    /// use rand::distr::{Distribution, Uniform};
    ///
    /// let die = Uniform::new_inclusive(1, 6).unwrap();
    /// let even_number = die.map(|num| num % 2 == 0);
    /// while !even_number.sample(&mut rand::rng()) {
    ///     println!("Still odd; rolling again!");
    /// }
    /// ```
    fn map<F, S>(self, func: F) -> Map<Self, F, T, S>
    where
        F: Fn(T) -> S,
        Self: Sized,
    {
        Map {
            distr: self,
            func,
            phantom: core::marker::PhantomData,
        }
    }
}

impl<T, D: Distribution<T> + ?Sized> Distribution<T> for &D {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> T {
        (*self).sample(rng)
    }
}

/// An iterator over a [`Distribution`]
///
/// This iterator yields random values of type `T` with distribution `D`
/// from a random generator of type `R`.
///
/// Construct this `struct` using [`Distribution::sample_iter`] or
/// [`Rng::sample_iter`]. It is also used by [`Rng::random_iter`] and
/// [`crate::random_iter`].
#[derive(Debug)]
pub struct Iter<D, R, T> {
    distr: D,
    rng: R,
    phantom: core::marker::PhantomData<T>,
}

impl<D, R, T> Iterator for Iter<D, R, T>
where
    D: Distribution<T>,
    R: Rng,
{
    type Item = T;

    #[inline(always)]
    fn next(&mut self) -> Option<T> {
        // Here, self.rng may be a reference, but we must take &mut anyway.
        // Even if sample could take an R: Rng by value, we would need to do this
        // since Rng is not copyable and we cannot enforce that this is "reborrowable".
        Some(self.distr.sample(&mut self.rng))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

impl<D, R, T> iter::FusedIterator for Iter<D, R, T>
where
    D: Distribution<T>,
    R: Rng,
{
}

/// A [`Distribution`] which maps sampled values to type `S`
///
/// This `struct` is created by the [`Distribution::map`] method.
/// See its documentation for more.
#[derive(Debug)]
pub struct Map<D, F, T, S> {
    distr: D,
    func: F,
    phantom: core::marker::PhantomData<fn(T) -> S>,
}

impl<D, F, T, S> Distribution<S> for Map<D, F, T, S>
where
    D: Distribution<T>,
    F: Fn(T) -> S,
{
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> S {
        (self.func)(self.distr.sample(rng))
    }
}

/// Sample or extend a [`String`]
///
/// Helper methods to extend a [`String`] or sample a new [`String`].
#[cfg(feature = "alloc")]
pub trait SampleString {
    /// Append `len` random chars to `string`
    ///
    /// Note: implementations may leave `string` with excess capacity. If this
    /// is undesirable, consider calling [`String::shrink_to_fit`] after this
    /// method.
    fn append_string<R: Rng + ?Sized>(&self, rng: &mut R, string: &mut String, len: usize);

    /// Generate a [`String`] of `len` random chars
    ///
    /// Note: implementations may leave the string with excess capacity. If this
    /// is undesirable, consider calling [`String::shrink_to_fit`] after this
    /// method.
    #[inline]
    fn sample_string<R: Rng + ?Sized>(&self, rng: &mut R, len: usize) -> String {
        let mut s = String::new();
        self.append_string(rng, &mut s, len);
        s
    }
}

#[cfg(test)]
mod tests {
    use crate::distr::{Distribution, Uniform};
    use crate::Rng;

    #[test]
    fn test_distributions_iter() {
        use crate::distr::Open01;
        let mut rng = crate::test::rng(210);
        let distr = Open01;
        let mut iter = Distribution::<f32>::sample_iter(distr, &mut rng);
        let mut sum: f32 = 0.;
        for _ in 0..100 {
            sum += iter.next().unwrap();
        }
        assert!(0. < sum && sum < 100.);
    }

    #[test]
    fn test_distributions_map() {
        let dist = Uniform::new_inclusive(0, 5).unwrap().map(|val| val + 15);

        let mut rng = crate::test::rng(212);
        let val = dist.sample(&mut rng);
        assert!((15..=20).contains(&val));
    }

    #[test]
    fn test_make_an_iter() {
        fn ten_dice_rolls_other_than_five<R: Rng>(rng: &mut R) -> impl Iterator<Item = i32> + '_ {
            Uniform::new_inclusive(1, 6)
                .unwrap()
                .sample_iter(rng)
                .filter(|x| *x != 5)
                .take(10)
        }

        let mut rng = crate::test::rng(211);
        let mut count = 0;
        for val in ten_dice_rolls_other_than_five(&mut rng) {
            assert!((1..=6).contains(&val) && val != 5);
            count += 1;
        }
        assert_eq!(count, 10);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_dist_string() {
        use crate::distr::{Alphabetic, Alphanumeric, SampleString, StandardUniform};
        use core::str;
        let mut rng = crate::test::rng(213);

        let s1 = Alphanumeric.sample_string(&mut rng, 20);
        assert_eq!(s1.len(), 20);
        assert_eq!(str::from_utf8(s1.as_bytes()), Ok(s1.as_str()));

        let s2 = StandardUniform.sample_string(&mut rng, 20);
        assert_eq!(s2.chars().count(), 20);
        assert_eq!(str::from_utf8(s2.as_bytes()), Ok(s2.as_str()));

        let s3 = Alphabetic.sample_string(&mut rng, 20);
        assert_eq!(s3.len(), 20);
        assert_eq!(str::from_utf8(s3.as_bytes()), Ok(s3.as_str()));
    }
}
