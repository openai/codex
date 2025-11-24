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
use core::iter;
#[cfg(feature = "alloc")]
use alloc::string::String;

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
    /// use rand::thread_rng;
    /// use rand::distributions::{Distribution, Alphanumeric, Uniform, Standard};
    ///
    /// let mut rng = thread_rng();
    ///
    /// // Vec of 16 x f32:
    /// let v: Vec<f32> = Standard.sample_iter(&mut rng).take(16).collect();
    ///
    /// // String:
    /// let s: String = Alphanumeric
    ///     .sample_iter(&mut rng)
    ///     .take(7)
    ///     .map(char::from)
    ///     .collect();
    ///
    /// // Dice-rolling:
    /// let die_range = Uniform::new_inclusive(1, 6);
    /// let mut roll_die = die_range.sample_iter(&mut rng);
    /// while roll_die.next().unwrap() != 6 {
    ///     println!("Not a 6; rolling again!");
    /// }
    /// ```
    fn sample_iter<R>(self, rng: R) -> DistIter<Self, R, T>
    where
        R: Rng,
        Self: Sized,
    {
        DistIter {
            distr: self,
            rng,
            phantom: ::core::marker::PhantomData,
        }
    }

    /// Create a distribution of values of 'S' by mapping the output of `Self`
    /// through the closure `F`
    ///
    /// # Example
    ///
    /// ```
    /// use rand::thread_rng;
    /// use rand::distributions::{Distribution, Uniform};
    ///
    /// let mut rng = thread_rng();
    ///
    /// let die = Uniform::new_inclusive(1, 6);
    /// let even_number = die.map(|num| num % 2 == 0);
    /// while !even_number.sample(&mut rng) {
    ///     println!("Still odd; rolling again!");
    /// }
    /// ```
    fn map<F, S>(self, func: F) -> DistMap<Self, F, T, S>
    where
        F: Fn(T) -> S,
        Self: Sized,
    {
        DistMap {
            distr: self,
            func,
            phantom: ::core::marker::PhantomData,
        }
    }
}

impl<'a, T, D: Distribution<T>> Distribution<T> for &'a D {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> T {
        (*self).sample(rng)
    }
}

/// An iterator that generates random values of `T` with distribution `D`,
/// using `R` as the source of randomness.
///
/// This `struct` is created by the [`sample_iter`] method on [`Distribution`].
/// See its documentation for more.
///
/// [`sample_iter`]: Distribution::sample_iter
#[derive(Debug)]
pub struct DistIter<D, R, T> {
    distr: D,
    rng: R,
    phantom: ::core::marker::PhantomData<T>,
}

impl<D, R, T> Iterator for DistIter<D, R, T>
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
        (usize::max_value(), None)
    }
}

impl<D, R, T> iter::FusedIterator for DistIter<D, R, T>
where
    D: Distribution<T>,
    R: Rng,
{
}

#[cfg(features = "nightly")]
impl<D, R, T> iter::TrustedLen for DistIter<D, R, T>
where
    D: Distribution<T>,
    R: Rng,
{
}

/// A distribution of values of type `S` derived from the distribution `D`
/// by mapping its output of type `T` through the closure `F`.
///
/// This `struct` is created by the [`Distribution::map`] method.
/// See its documentation for more.
#[derive(Debug)]
pub struct DistMap<D, F, T, S> {
    distr: D,
    func: F,
    phantom: ::core::marker::PhantomData<fn(T) -> S>,
}

impl<D, F, T, S> Distribution<S> for DistMap<D, F, T, S>
where
    D: Distribution<T>,
    F: Fn(T) -> S,
{
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> S {
        (self.func)(self.distr.sample(rng))
    }
}

/// `String` sampler
///
/// Sampling a `String` of random characters is not quite the same as collecting
/// a sequence of chars. This trait contains some helpers.
#[cfg(feature = "alloc")]
pub trait DistString {
    /// Append `len` random chars to `string`
    fn append_string<R: Rng + ?Sized>(&self, rng: &mut R, string: &mut String, len: usize);

    /// Generate a `String` of `len` random chars
    #[inline]
    fn sample_string<R: Rng + ?Sized>(&self, rng: &mut R, len: usize) -> String {
        let mut s = String::new();
        self.append_string(rng, &mut s, len);
        s
    }
}

#[cfg(test)]
mod tests {
    use crate::distributions::{Distribution, Uniform};
    use crate::Rng;

    #[test]
    fn test_distributions_iter() {
        use crate::distributions::Open01;
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
        let dist = Uniform::new_inclusive(0, 5).map(|val| val + 15);

        let mut rng = crate::test::rng(212);
        let val = dist.sample(&mut rng);
        assert!((15..=20).contains(&val));
    }

    #[test]
    fn test_make_an_iter() {
        fn ten_dice_rolls_other_than_five<R: Rng>(
            rng: &mut R,
        ) -> impl Iterator<Item = i32> + '_ {
            Uniform::new_inclusive(1, 6)
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
        use core::str;
        use crate::distributions::{Alphanumeric, DistString, Standard};
        let mut rng = crate::test::rng(213);

        let s1 = Alphanumeric.sample_string(&mut rng, 20);
        assert_eq!(s1.len(), 20);
        assert_eq!(str::from_utf8(s1.as_bytes()), Ok(s1.as_str()));

        let s2 = Standard.sample_string(&mut rng, 20);
        assert_eq!(s2.chars().count(), 20);
        assert_eq!(str::from_utf8(s2.as_bytes()), Ok(s2.as_str()));
    }
}
