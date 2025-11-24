// Copyright 2018-2023 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! `IndexedRandom`, `IndexedMutRandom`, `SliceRandom`

use super::increasing_uniform::IncreasingUniform;
use super::index;
#[cfg(feature = "alloc")]
use crate::distr::uniform::{SampleBorrow, SampleUniform};
#[cfg(feature = "alloc")]
use crate::distr::weighted::{Error as WeightError, Weight};
use crate::Rng;
use core::ops::{Index, IndexMut};

/// Extension trait on indexable lists, providing random sampling methods.
///
/// This trait is implemented on `[T]` slice types. Other types supporting
/// [`std::ops::Index<usize>`] may implement this (only [`Self::len`] must be
/// specified).
pub trait IndexedRandom: Index<usize> {
    /// The length
    fn len(&self) -> usize;

    /// True when the length is zero
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Uniformly sample one element
    ///
    /// Returns a reference to one uniformly-sampled random element of
    /// the slice, or `None` if the slice is empty.
    ///
    /// For slices, complexity is `O(1)`.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::seq::IndexedRandom;
    ///
    /// let choices = [1, 2, 4, 8, 16, 32];
    /// let mut rng = rand::rng();
    /// println!("{:?}", choices.choose(&mut rng));
    /// assert_eq!(choices[..0].choose(&mut rng), None);
    /// ```
    fn choose<R>(&self, rng: &mut R) -> Option<&Self::Output>
    where
        R: Rng + ?Sized,
    {
        if self.is_empty() {
            None
        } else {
            Some(&self[rng.random_range(..self.len())])
        }
    }

    /// Uniformly sample `amount` distinct elements from self
    ///
    /// Chooses `amount` elements from the slice at random, without repetition,
    /// and in random order. The returned iterator is appropriate both for
    /// collection into a `Vec` and filling an existing buffer (see example).
    ///
    /// In case this API is not sufficiently flexible, use [`index::sample`].
    ///
    /// For slices, complexity is the same as [`index::sample`].
    ///
    /// # Example
    /// ```
    /// use rand::seq::IndexedRandom;
    ///
    /// let mut rng = &mut rand::rng();
    /// let sample = "Hello, audience!".as_bytes();
    ///
    /// // collect the results into a vector:
    /// let v: Vec<u8> = sample.choose_multiple(&mut rng, 3).cloned().collect();
    ///
    /// // store in a buffer:
    /// let mut buf = [0u8; 5];
    /// for (b, slot) in sample.choose_multiple(&mut rng, buf.len()).zip(buf.iter_mut()) {
    ///     *slot = *b;
    /// }
    /// ```
    #[cfg(feature = "alloc")]
    fn choose_multiple<R>(
        &self,
        rng: &mut R,
        amount: usize,
    ) -> SliceChooseIter<'_, Self, Self::Output>
    where
        Self::Output: Sized,
        R: Rng + ?Sized,
    {
        let amount = core::cmp::min(amount, self.len());
        SliceChooseIter {
            slice: self,
            _phantom: Default::default(),
            indices: index::sample(rng, self.len(), amount).into_iter(),
        }
    }

    /// Uniformly sample a fixed-size array of distinct elements from self
    ///
    /// Chooses `N` elements from the slice at random, without repetition,
    /// and in random order.
    ///
    /// For slices, complexity is the same as [`index::sample_array`].
    ///
    /// # Example
    /// ```
    /// use rand::seq::IndexedRandom;
    ///
    /// let mut rng = &mut rand::rng();
    /// let sample = "Hello, audience!".as_bytes();
    ///
    /// let a: [u8; 3] = sample.choose_multiple_array(&mut rng).unwrap();
    /// ```
    fn choose_multiple_array<R, const N: usize>(&self, rng: &mut R) -> Option<[Self::Output; N]>
    where
        Self::Output: Clone + Sized,
        R: Rng + ?Sized,
    {
        let indices = index::sample_array(rng, self.len())?;
        Some(indices.map(|index| self[index].clone()))
    }

    /// Biased sampling for one element
    ///
    /// Returns a reference to one element of the slice, sampled according
    /// to the provided weights. Returns `None` only if the slice is empty.
    ///
    /// The specified function `weight` maps each item `x` to a relative
    /// likelihood `weight(x)`. The probability of each item being selected is
    /// therefore `weight(x) / s`, where `s` is the sum of all `weight(x)`.
    ///
    /// For slices of length `n`, complexity is `O(n)`.
    /// For more information about the underlying algorithm,
    /// see the [`WeightedIndex`] distribution.
    ///
    /// See also [`choose_weighted_mut`].
    ///
    /// # Example
    ///
    /// ```
    /// use rand::prelude::*;
    ///
    /// let choices = [('a', 2), ('b', 1), ('c', 1), ('d', 0)];
    /// let mut rng = rand::rng();
    /// // 50% chance to print 'a', 25% chance to print 'b', 25% chance to print 'c',
    /// // and 'd' will never be printed
    /// println!("{:?}", choices.choose_weighted(&mut rng, |item| item.1).unwrap().0);
    /// ```
    /// [`choose`]: IndexedRandom::choose
    /// [`choose_weighted_mut`]: IndexedMutRandom::choose_weighted_mut
    /// [`WeightedIndex`]: crate::distr::weighted::WeightedIndex
    #[cfg(feature = "alloc")]
    fn choose_weighted<R, F, B, X>(
        &self,
        rng: &mut R,
        weight: F,
    ) -> Result<&Self::Output, WeightError>
    where
        R: Rng + ?Sized,
        F: Fn(&Self::Output) -> B,
        B: SampleBorrow<X>,
        X: SampleUniform + Weight + PartialOrd<X>,
    {
        use crate::distr::{weighted::WeightedIndex, Distribution};
        let distr = WeightedIndex::new((0..self.len()).map(|idx| weight(&self[idx])))?;
        Ok(&self[distr.sample(rng)])
    }

    /// Biased sampling of `amount` distinct elements
    ///
    /// Similar to [`choose_multiple`], but where the likelihood of each
    /// element's inclusion in the output may be specified. Zero-weighted
    /// elements are never returned; the result may therefore contain fewer
    /// elements than `amount` even when `self.len() >= amount`. The elements
    /// are returned in an arbitrary, unspecified order.
    ///
    /// The specified function `weight` maps each item `x` to a relative
    /// likelihood `weight(x)`. The probability of each item being selected is
    /// therefore `weight(x) / s`, where `s` is the sum of all `weight(x)`.
    ///
    /// This implementation uses `O(length + amount)` space and `O(length)` time.
    /// See [`index::sample_weighted`] for details.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::prelude::*;
    ///
    /// let choices = [('a', 2), ('b', 1), ('c', 1)];
    /// let mut rng = rand::rng();
    /// // First Draw * Second Draw = total odds
    /// // -----------------------
    /// // (50% * 50%) + (25% * 67%) = 41.7% chance that the output is `['a', 'b']` in some order.
    /// // (50% * 50%) + (25% * 67%) = 41.7% chance that the output is `['a', 'c']` in some order.
    /// // (25% * 33%) + (25% * 33%) = 16.6% chance that the output is `['b', 'c']` in some order.
    /// println!("{:?}", choices.choose_multiple_weighted(&mut rng, 2, |item| item.1).unwrap().collect::<Vec<_>>());
    /// ```
    /// [`choose_multiple`]: IndexedRandom::choose_multiple
    // Note: this is feature-gated on std due to usage of f64::powf.
    // If necessary, we may use alloc+libm as an alternative (see PR #1089).
    #[cfg(feature = "std")]
    fn choose_multiple_weighted<R, F, X>(
        &self,
        rng: &mut R,
        amount: usize,
        weight: F,
    ) -> Result<SliceChooseIter<'_, Self, Self::Output>, WeightError>
    where
        Self::Output: Sized,
        R: Rng + ?Sized,
        F: Fn(&Self::Output) -> X,
        X: Into<f64>,
    {
        let amount = core::cmp::min(amount, self.len());
        Ok(SliceChooseIter {
            slice: self,
            _phantom: Default::default(),
            indices: index::sample_weighted(
                rng,
                self.len(),
                |idx| weight(&self[idx]).into(),
                amount,
            )?
            .into_iter(),
        })
    }
}

/// Extension trait on indexable lists, providing random sampling methods.
///
/// This trait is implemented automatically for every type implementing
/// [`IndexedRandom`] and [`std::ops::IndexMut<usize>`].
pub trait IndexedMutRandom: IndexedRandom + IndexMut<usize> {
    /// Uniformly sample one element (mut)
    ///
    /// Returns a mutable reference to one uniformly-sampled random element of
    /// the slice, or `None` if the slice is empty.
    ///
    /// For slices, complexity is `O(1)`.
    fn choose_mut<R>(&mut self, rng: &mut R) -> Option<&mut Self::Output>
    where
        R: Rng + ?Sized,
    {
        if self.is_empty() {
            None
        } else {
            let len = self.len();
            Some(&mut self[rng.random_range(..len)])
        }
    }

    /// Biased sampling for one element (mut)
    ///
    /// Returns a mutable reference to one element of the slice, sampled according
    /// to the provided weights. Returns `None` only if the slice is empty.
    ///
    /// The specified function `weight` maps each item `x` to a relative
    /// likelihood `weight(x)`. The probability of each item being selected is
    /// therefore `weight(x) / s`, where `s` is the sum of all `weight(x)`.
    ///
    /// For slices of length `n`, complexity is `O(n)`.
    /// For more information about the underlying algorithm,
    /// see the [`WeightedIndex`] distribution.
    ///
    /// See also [`choose_weighted`].
    ///
    /// [`choose_mut`]: IndexedMutRandom::choose_mut
    /// [`choose_weighted`]: IndexedRandom::choose_weighted
    /// [`WeightedIndex`]: crate::distr::weighted::WeightedIndex
    #[cfg(feature = "alloc")]
    fn choose_weighted_mut<R, F, B, X>(
        &mut self,
        rng: &mut R,
        weight: F,
    ) -> Result<&mut Self::Output, WeightError>
    where
        R: Rng + ?Sized,
        F: Fn(&Self::Output) -> B,
        B: SampleBorrow<X>,
        X: SampleUniform + Weight + PartialOrd<X>,
    {
        use crate::distr::{weighted::WeightedIndex, Distribution};
        let distr = WeightedIndex::new((0..self.len()).map(|idx| weight(&self[idx])))?;
        let index = distr.sample(rng);
        Ok(&mut self[index])
    }
}

/// Extension trait on slices, providing shuffling methods.
///
/// This trait is implemented on all `[T]` slice types, providing several
/// methods for choosing and shuffling elements. You must `use` this trait:
///
/// ```
/// use rand::seq::SliceRandom;
///
/// let mut rng = rand::rng();
/// let mut bytes = "Hello, random!".to_string().into_bytes();
/// bytes.shuffle(&mut rng);
/// let str = String::from_utf8(bytes).unwrap();
/// println!("{}", str);
/// ```
/// Example output (non-deterministic):
/// ```none
/// l,nmroHado !le
/// ```
pub trait SliceRandom: IndexedMutRandom {
    /// Shuffle a mutable slice in place.
    ///
    /// For slices of length `n`, complexity is `O(n)`.
    /// The resulting permutation is picked uniformly from the set of all possible permutations.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::seq::SliceRandom;
    ///
    /// let mut rng = rand::rng();
    /// let mut y = [1, 2, 3, 4, 5];
    /// println!("Unshuffled: {:?}", y);
    /// y.shuffle(&mut rng);
    /// println!("Shuffled:   {:?}", y);
    /// ```
    fn shuffle<R>(&mut self, rng: &mut R)
    where
        R: Rng + ?Sized;

    /// Shuffle a slice in place, but exit early.
    ///
    /// Returns two mutable slices from the source slice. The first contains
    /// `amount` elements randomly permuted. The second has the remaining
    /// elements that are not fully shuffled.
    ///
    /// This is an efficient method to select `amount` elements at random from
    /// the slice, provided the slice may be mutated.
    ///
    /// If you only need to choose elements randomly and `amount > self.len()/2`
    /// then you may improve performance by taking
    /// `amount = self.len() - amount` and using only the second slice.
    ///
    /// If `amount` is greater than the number of elements in the slice, this
    /// will perform a full shuffle.
    ///
    /// For slices, complexity is `O(m)` where `m = amount`.
    fn partial_shuffle<R>(
        &mut self,
        rng: &mut R,
        amount: usize,
    ) -> (&mut [Self::Output], &mut [Self::Output])
    where
        Self::Output: Sized,
        R: Rng + ?Sized;
}

impl<T> IndexedRandom for [T] {
    fn len(&self) -> usize {
        self.len()
    }
}

impl<IR: IndexedRandom + IndexMut<usize> + ?Sized> IndexedMutRandom for IR {}

impl<T> SliceRandom for [T] {
    fn shuffle<R>(&mut self, rng: &mut R)
    where
        R: Rng + ?Sized,
    {
        if self.len() <= 1 {
            // There is no need to shuffle an empty or single element slice
            return;
        }
        self.partial_shuffle(rng, self.len());
    }

    fn partial_shuffle<R>(&mut self, rng: &mut R, amount: usize) -> (&mut [T], &mut [T])
    where
        R: Rng + ?Sized,
    {
        let m = self.len().saturating_sub(amount);

        // The algorithm below is based on Durstenfeld's algorithm for the
        // [Fisher–Yates shuffle](https://en.wikipedia.org/wiki/Fisher%E2%80%93Yates_shuffle#The_modern_algorithm)
        // for an unbiased permutation.
        // It ensures that the last `amount` elements of the slice
        // are randomly selected from the whole slice.

        // `IncreasingUniform::next_index()` is faster than `Rng::random_range`
        // but only works for 32 bit integers
        // So we must use the slow method if the slice is longer than that.
        if self.len() < (u32::MAX as usize) {
            let mut chooser = IncreasingUniform::new(rng, m as u32);
            for i in m..self.len() {
                let index = chooser.next_index();
                self.swap(i, index);
            }
        } else {
            for i in m..self.len() {
                let index = rng.random_range(..i + 1);
                self.swap(i, index);
            }
        }
        let r = self.split_at_mut(m);
        (r.1, r.0)
    }
}

/// An iterator over multiple slice elements.
///
/// This struct is created by
/// [`IndexedRandom::choose_multiple`](trait.IndexedRandom.html#tymethod.choose_multiple).
#[cfg(feature = "alloc")]
#[derive(Debug)]
pub struct SliceChooseIter<'a, S: ?Sized + 'a, T: 'a> {
    slice: &'a S,
    _phantom: core::marker::PhantomData<T>,
    indices: index::IndexVecIntoIter,
}

#[cfg(feature = "alloc")]
impl<'a, S: Index<usize, Output = T> + ?Sized + 'a, T: 'a> Iterator for SliceChooseIter<'a, S, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO: investigate using SliceIndex::get_unchecked when stable
        self.indices.next().map(|i| &self.slice[i])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.indices.len(), Some(self.indices.len()))
    }
}

#[cfg(feature = "alloc")]
impl<'a, S: Index<usize, Output = T> + ?Sized + 'a, T: 'a> ExactSizeIterator
    for SliceChooseIter<'a, S, T>
{
    fn len(&self) -> usize {
        self.indices.len()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[cfg(feature = "alloc")]
    use alloc::vec::Vec;

    #[test]
    fn test_slice_choose() {
        let mut r = crate::test::rng(107);
        let chars = [
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n',
        ];
        let mut chosen = [0i32; 14];
        // The below all use a binomial distribution with n=1000, p=1/14.
        // binocdf(40, 1000, 1/14) ~= 2e-5; 1-binocdf(106, ..) ~= 2e-5
        for _ in 0..1000 {
            let picked = *chars.choose(&mut r).unwrap();
            chosen[(picked as usize) - ('a' as usize)] += 1;
        }
        for count in chosen.iter() {
            assert!(40 < *count && *count < 106);
        }

        chosen.iter_mut().for_each(|x| *x = 0);
        for _ in 0..1000 {
            *chosen.choose_mut(&mut r).unwrap() += 1;
        }
        for count in chosen.iter() {
            assert!(40 < *count && *count < 106);
        }

        let mut v: [isize; 0] = [];
        assert_eq!(v.choose(&mut r), None);
        assert_eq!(v.choose_mut(&mut r), None);
    }

    #[test]
    fn value_stability_slice() {
        let mut r = crate::test::rng(413);
        let chars = [
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n',
        ];
        let mut nums = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

        assert_eq!(chars.choose(&mut r), Some(&'l'));
        assert_eq!(nums.choose_mut(&mut r), Some(&mut 3));

        assert_eq!(
            &chars.choose_multiple_array(&mut r),
            &Some(['f', 'i', 'd', 'b', 'c', 'm', 'j', 'k'])
        );

        #[cfg(feature = "alloc")]
        assert_eq!(
            &chars
                .choose_multiple(&mut r, 8)
                .cloned()
                .collect::<Vec<char>>(),
            &['h', 'm', 'd', 'b', 'c', 'e', 'n', 'f']
        );

        #[cfg(feature = "alloc")]
        assert_eq!(chars.choose_weighted(&mut r, |_| 1), Ok(&'i'));
        #[cfg(feature = "alloc")]
        assert_eq!(nums.choose_weighted_mut(&mut r, |_| 1), Ok(&mut 2));

        let mut r = crate::test::rng(414);
        nums.shuffle(&mut r);
        assert_eq!(nums, [5, 11, 0, 8, 7, 12, 6, 4, 9, 3, 1, 2, 10]);
        nums = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let res = nums.partial_shuffle(&mut r, 6);
        assert_eq!(res.0, &mut [7, 12, 6, 8, 1, 9]);
        assert_eq!(res.1, &mut [0, 11, 2, 3, 4, 5, 10]);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_shuffle() {
        let mut r = crate::test::rng(108);
        let empty: &mut [isize] = &mut [];
        empty.shuffle(&mut r);
        let mut one = [1];
        one.shuffle(&mut r);
        let b: &[_] = &[1];
        assert_eq!(one, b);

        let mut two = [1, 2];
        two.shuffle(&mut r);
        assert!(two == [1, 2] || two == [2, 1]);

        fn move_last(slice: &mut [usize], pos: usize) {
            // use slice[pos..].rotate_left(1); once we can use that
            let last_val = slice[pos];
            for i in pos..slice.len() - 1 {
                slice[i] = slice[i + 1];
            }
            *slice.last_mut().unwrap() = last_val;
        }
        let mut counts = [0i32; 24];
        for _ in 0..10000 {
            let mut arr: [usize; 4] = [0, 1, 2, 3];
            arr.shuffle(&mut r);
            let mut permutation = 0usize;
            let mut pos_value = counts.len();
            for i in 0..4 {
                pos_value /= 4 - i;
                let pos = arr.iter().position(|&x| x == i).unwrap();
                assert!(pos < (4 - i));
                permutation += pos * pos_value;
                move_last(&mut arr, pos);
                assert_eq!(arr[3], i);
            }
            for (i, &a) in arr.iter().enumerate() {
                assert_eq!(a, i);
            }
            counts[permutation] += 1;
        }
        for count in counts.iter() {
            // Binomial(10000, 1/24) with average 416.667
            // Octave: binocdf(n, 10000, 1/24)
            // 99.9% chance samples lie within this range:
            assert!(352 <= *count && *count <= 483, "count: {}", count);
        }
    }

    #[test]
    fn test_partial_shuffle() {
        let mut r = crate::test::rng(118);

        let mut empty: [u32; 0] = [];
        let res = empty.partial_shuffle(&mut r, 10);
        assert_eq!((res.0.len(), res.1.len()), (0, 0));

        let mut v = [1, 2, 3, 4, 5];
        let res = v.partial_shuffle(&mut r, 2);
        assert_eq!((res.0.len(), res.1.len()), (2, 3));
        assert!(res.0[0] != res.0[1]);
        // First elements are only modified if selected, so at least one isn't modified:
        assert!(res.1[0] == 1 || res.1[1] == 2 || res.1[2] == 3);
    }

    #[test]
    #[cfg(feature = "alloc")]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_weighted() {
        let mut r = crate::test::rng(406);
        const N_REPS: u32 = 3000;
        let weights = [1u32, 2, 3, 0, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7];
        let total_weight = weights.iter().sum::<u32>() as f32;

        let verify = |result: [i32; 14]| {
            for (i, count) in result.iter().enumerate() {
                let exp = (weights[i] * N_REPS) as f32 / total_weight;
                let mut err = (*count as f32 - exp).abs();
                if err != 0.0 {
                    err /= exp;
                }
                assert!(err <= 0.25);
            }
        };

        // choose_weighted
        fn get_weight<T>(item: &(u32, T)) -> u32 {
            item.0
        }
        let mut chosen = [0i32; 14];
        let mut items = [(0u32, 0usize); 14]; // (weight, index)
        for (i, item) in items.iter_mut().enumerate() {
            *item = (weights[i], i);
        }
        for _ in 0..N_REPS {
            let item = items.choose_weighted(&mut r, get_weight).unwrap();
            chosen[item.1] += 1;
        }
        verify(chosen);

        // choose_weighted_mut
        let mut items = [(0u32, 0i32); 14]; // (weight, count)
        for (i, item) in items.iter_mut().enumerate() {
            *item = (weights[i], 0);
        }
        for _ in 0..N_REPS {
            items.choose_weighted_mut(&mut r, get_weight).unwrap().1 += 1;
        }
        for (ch, item) in chosen.iter_mut().zip(items.iter()) {
            *ch = item.1;
        }
        verify(chosen);

        // Check error cases
        let empty_slice = &mut [10][0..0];
        assert_eq!(
            empty_slice.choose_weighted(&mut r, |_| 1),
            Err(WeightError::InvalidInput)
        );
        assert_eq!(
            empty_slice.choose_weighted_mut(&mut r, |_| 1),
            Err(WeightError::InvalidInput)
        );
        assert_eq!(
            ['x'].choose_weighted_mut(&mut r, |_| 0),
            Err(WeightError::InsufficientNonZero)
        );
        assert_eq!(
            [0, -1].choose_weighted_mut(&mut r, |x| *x),
            Err(WeightError::InvalidWeight)
        );
        assert_eq!(
            [-1, 0].choose_weighted_mut(&mut r, |x| *x),
            Err(WeightError::InvalidWeight)
        );
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_multiple_weighted_edge_cases() {
        use super::*;

        let mut rng = crate::test::rng(413);

        // Case 1: One of the weights is 0
        let choices = [('a', 2), ('b', 1), ('c', 0)];
        for _ in 0..100 {
            let result = choices
                .choose_multiple_weighted(&mut rng, 2, |item| item.1)
                .unwrap()
                .collect::<Vec<_>>();

            assert_eq!(result.len(), 2);
            assert!(!result.iter().any(|val| val.0 == 'c'));
        }

        // Case 2: All of the weights are 0
        let choices = [('a', 0), ('b', 0), ('c', 0)];
        let r = choices.choose_multiple_weighted(&mut rng, 2, |item| item.1);
        assert_eq!(r.unwrap().len(), 0);

        // Case 3: Negative weights
        let choices = [('a', -1), ('b', 1), ('c', 1)];
        let r = choices.choose_multiple_weighted(&mut rng, 2, |item| item.1);
        assert_eq!(r.unwrap_err(), WeightError::InvalidWeight);

        // Case 4: Empty list
        let choices = [];
        let r = choices.choose_multiple_weighted(&mut rng, 0, |_: &()| 0);
        assert_eq!(r.unwrap().count(), 0);

        // Case 5: NaN weights
        let choices = [('a', f64::NAN), ('b', 1.0), ('c', 1.0)];
        let r = choices.choose_multiple_weighted(&mut rng, 2, |item| item.1);
        assert_eq!(r.unwrap_err(), WeightError::InvalidWeight);

        // Case 6: +infinity weights
        let choices = [('a', f64::INFINITY), ('b', 1.0), ('c', 1.0)];
        for _ in 0..100 {
            let result = choices
                .choose_multiple_weighted(&mut rng, 2, |item| item.1)
                .unwrap()
                .collect::<Vec<_>>();
            assert_eq!(result.len(), 2);
            assert!(result.iter().any(|val| val.0 == 'a'));
        }

        // Case 7: -infinity weights
        let choices = [('a', f64::NEG_INFINITY), ('b', 1.0), ('c', 1.0)];
        let r = choices.choose_multiple_weighted(&mut rng, 2, |item| item.1);
        assert_eq!(r.unwrap_err(), WeightError::InvalidWeight);

        // Case 8: -0 weights
        let choices = [('a', -0.0), ('b', 1.0), ('c', 1.0)];
        let r = choices.choose_multiple_weighted(&mut rng, 2, |item| item.1);
        assert!(r.is_ok());
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_multiple_weighted_distributions() {
        use super::*;

        // The theoretical probabilities of the different outcomes are:
        // AB: 0.5   * 0.667 = 0.3333
        // AC: 0.5   * 0.333 = 0.1667
        // BA: 0.333 * 0.75  = 0.25
        // BC: 0.333 * 0.25  = 0.0833
        // CA: 0.167 * 0.6   = 0.1
        // CB: 0.167 * 0.4   = 0.0667
        let choices = [('a', 3), ('b', 2), ('c', 1)];
        let mut rng = crate::test::rng(414);

        let mut results = [0i32; 3];
        let expected_results = [5833, 2667, 1500];
        for _ in 0..10000 {
            let result = choices
                .choose_multiple_weighted(&mut rng, 2, |item| item.1)
                .unwrap()
                .collect::<Vec<_>>();

            assert_eq!(result.len(), 2);

            match (result[0].0, result[1].0) {
                ('a', 'b') | ('b', 'a') => {
                    results[0] += 1;
                }
                ('a', 'c') | ('c', 'a') => {
                    results[1] += 1;
                }
                ('b', 'c') | ('c', 'b') => {
                    results[2] += 1;
                }
                (_, _) => panic!("unexpected result"),
            }
        }

        let mut diffs = results
            .iter()
            .zip(&expected_results)
            .map(|(a, b)| (a - b).abs());
        assert!(!diffs.any(|deviation| deviation > 100));
    }
}
