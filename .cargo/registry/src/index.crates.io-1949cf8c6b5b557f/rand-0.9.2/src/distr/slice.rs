// Copyright 2021 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Distributions over slices

use core::num::NonZeroUsize;

use crate::distr::uniform::{UniformSampler, UniformUsize};
use crate::distr::Distribution;
#[cfg(feature = "alloc")]
use alloc::string::String;

/// A distribution to uniformly sample elements of a slice
///
/// Like [`IndexedRandom::choose`], this uniformly samples elements of a slice
/// without modification of the slice (so called "sampling with replacement").
/// This distribution object may be a little faster for repeated sampling (but
/// slower for small numbers of samples).
///
/// ## Examples
///
/// Since this is a distribution, [`Rng::sample_iter`] and
/// [`Distribution::sample_iter`] may be used, for example:
/// ```
/// use rand::distr::{Distribution, slice::Choose};
///
/// let vowels = ['a', 'e', 'i', 'o', 'u'];
/// let vowels_dist = Choose::new(&vowels).unwrap();
///
/// // build a string of 10 vowels
/// let vowel_string: String = vowels_dist
///     .sample_iter(&mut rand::rng())
///     .take(10)
///     .collect();
///
/// println!("{}", vowel_string);
/// assert_eq!(vowel_string.len(), 10);
/// assert!(vowel_string.chars().all(|c| vowels.contains(&c)));
/// ```
///
/// For a single sample, [`IndexedRandom::choose`] may be preferred:
/// ```
/// use rand::seq::IndexedRandom;
///
/// let vowels = ['a', 'e', 'i', 'o', 'u'];
/// let mut rng = rand::rng();
///
/// println!("{}", vowels.choose(&mut rng).unwrap());
/// ```
///
/// [`IndexedRandom::choose`]: crate::seq::IndexedRandom::choose
/// [`Rng::sample_iter`]: crate::Rng::sample_iter
#[derive(Debug, Clone, Copy)]
pub struct Choose<'a, T> {
    slice: &'a [T],
    range: UniformUsize,
    num_choices: NonZeroUsize,
}

impl<'a, T> Choose<'a, T> {
    /// Create a new `Choose` instance which samples uniformly from the slice.
    ///
    /// Returns error [`Empty`] if the slice is empty.
    pub fn new(slice: &'a [T]) -> Result<Self, Empty> {
        let num_choices = NonZeroUsize::new(slice.len()).ok_or(Empty)?;

        Ok(Self {
            slice,
            range: UniformUsize::new(0, num_choices.get()).unwrap(),
            num_choices,
        })
    }

    /// Returns the count of choices in this distribution
    pub fn num_choices(&self) -> NonZeroUsize {
        self.num_choices
    }
}

impl<'a, T> Distribution<&'a T> for Choose<'a, T> {
    fn sample<R: crate::Rng + ?Sized>(&self, rng: &mut R) -> &'a T {
        let idx = self.range.sample(rng);

        debug_assert!(
            idx < self.slice.len(),
            "Uniform::new(0, {}) somehow returned {}",
            self.slice.len(),
            idx
        );

        // Safety: at construction time, it was ensured that the slice was
        // non-empty, and that the `Uniform` range produces values in range
        // for the slice
        unsafe { self.slice.get_unchecked(idx) }
    }
}

/// Error: empty slice
///
/// This error is returned when [`Choose::new`] is given an empty slice.
#[derive(Debug, Clone, Copy)]
pub struct Empty;

impl core::fmt::Display for Empty {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Tried to create a `rand::distr::slice::Choose` with an empty slice"
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Empty {}

#[cfg(feature = "alloc")]
impl super::SampleString for Choose<'_, char> {
    fn append_string<R: crate::Rng + ?Sized>(&self, rng: &mut R, string: &mut String, len: usize) {
        // Get the max char length to minimize extra space.
        // Limit this check to avoid searching for long slice.
        let max_char_len = if self.slice.len() < 200 {
            self.slice
                .iter()
                .try_fold(1, |max_len, char| {
                    // When the current max_len is 4, the result max_char_len will be 4.
                    Some(max_len.max(char.len_utf8())).filter(|len| *len < 4)
                })
                .unwrap_or(4)
        } else {
            4
        };

        // Split the extension of string to reuse the unused capacities.
        // Skip the split for small length or only ascii slice.
        let mut extend_len = if max_char_len == 1 || len < 100 {
            len
        } else {
            len / 4
        };
        let mut remain_len = len;
        while extend_len > 0 {
            string.reserve(max_char_len * extend_len);
            string.extend(self.sample_iter(&mut *rng).take(extend_len));
            remain_len -= extend_len;
            extend_len = extend_len.min(remain_len);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::iter;

    #[test]
    fn value_stability() {
        let rng = crate::test::rng(651);
        let slice = Choose::new(b"escaped emus explore extensively").unwrap();
        let expected = b"eaxee";
        assert!(iter::zip(slice.sample_iter(rng), expected).all(|(a, b)| a == b));
    }
}
