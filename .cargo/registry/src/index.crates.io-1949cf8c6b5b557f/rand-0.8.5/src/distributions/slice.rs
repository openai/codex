// Copyright 2021 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::distributions::{Distribution, Uniform};

/// A distribution to sample items uniformly from a slice.
///
/// [`Slice::new`] constructs a distribution referencing a slice and uniformly
/// samples references from the items in the slice. It may do extra work up
/// front to make sampling of multiple values faster; if only one sample from
/// the slice is required, [`SliceRandom::choose`] can be more efficient.
///
/// Steps are taken to avoid bias which might be present in naive
/// implementations; for example `slice[rng.gen() % slice.len()]` samples from
/// the slice, but may be more likely to select numbers in the low range than
/// other values.
///
/// This distribution samples with replacement; each sample is independent.
/// Sampling without replacement requires state to be retained, and therefore
/// cannot be handled by a distribution; you should instead consider methods
/// on [`SliceRandom`], such as [`SliceRandom::choose_multiple`].
///
/// # Example
///
/// ```
/// use rand::Rng;
/// use rand::distributions::Slice;
///
/// let vowels = ['a', 'e', 'i', 'o', 'u'];
/// let vowels_dist = Slice::new(&vowels).unwrap();
/// let rng = rand::thread_rng();
///
/// // build a string of 10 vowels
/// let vowel_string: String = rng
///     .sample_iter(&vowels_dist)
///     .take(10)
///     .collect();
///
/// println!("{}", vowel_string);
/// assert_eq!(vowel_string.len(), 10);
/// assert!(vowel_string.chars().all(|c| vowels.contains(&c)));
/// ```
///
/// For a single sample, [`SliceRandom::choose`][crate::seq::SliceRandom::choose]
/// may be preferred:
///
/// ```
/// use rand::seq::SliceRandom;
///
/// let vowels = ['a', 'e', 'i', 'o', 'u'];
/// let mut rng = rand::thread_rng();
///
/// println!("{}", vowels.choose(&mut rng).unwrap())
/// ```
///
/// [`SliceRandom`]: crate::seq::SliceRandom
/// [`SliceRandom::choose`]: crate::seq::SliceRandom::choose
/// [`SliceRandom::choose_multiple`]: crate::seq::SliceRandom::choose_multiple
#[derive(Debug, Clone, Copy)]
pub struct Slice<'a, T> {
    slice: &'a [T],
    range: Uniform<usize>,
}

impl<'a, T> Slice<'a, T> {
    /// Create a new `Slice` instance which samples uniformly from the slice.
    /// Returns `Err` if the slice is empty.
    pub fn new(slice: &'a [T]) -> Result<Self, EmptySlice> {
        match slice.len() {
            0 => Err(EmptySlice),
            len => Ok(Self {
                slice,
                range: Uniform::new(0, len),
            }),
        }
    }
}

impl<'a, T> Distribution<&'a T> for Slice<'a, T> {
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

/// Error type indicating that a [`Slice`] distribution was improperly
/// constructed with an empty slice.
#[derive(Debug, Clone, Copy)]
pub struct EmptySlice;

impl core::fmt::Display for EmptySlice {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Tried to create a `distributions::Slice` with an empty slice"
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EmptySlice {}
