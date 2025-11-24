// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Weighted index sampling

use crate::distributions::uniform::{SampleBorrow, SampleUniform, UniformSampler};
use crate::distributions::Distribution;
use crate::Rng;
use core::cmp::PartialOrd;
use core::fmt;

// Note that this whole module is only imported if feature="alloc" is enabled.
use alloc::vec::Vec;

#[cfg(feature = "serde1")]
use serde::{Serialize, Deserialize};

/// A distribution using weighted sampling of discrete items
///
/// Sampling a `WeightedIndex` distribution returns the index of a randomly
/// selected element from the iterator used when the `WeightedIndex` was
/// created. The chance of a given element being picked is proportional to the
/// value of the element. The weights can use any type `X` for which an
/// implementation of [`Uniform<X>`] exists.
///
/// # Performance
///
/// Time complexity of sampling from `WeightedIndex` is `O(log N)` where
/// `N` is the number of weights. As an alternative,
/// [`rand_distr::weighted_alias`](https://docs.rs/rand_distr/*/rand_distr/weighted_alias/index.html)
/// supports `O(1)` sampling, but with much higher initialisation cost.
///
/// A `WeightedIndex<X>` contains a `Vec<X>` and a [`Uniform<X>`] and so its
/// size is the sum of the size of those objects, possibly plus some alignment.
///
/// Creating a `WeightedIndex<X>` will allocate enough space to hold `N - 1`
/// weights of type `X`, where `N` is the number of weights. However, since
/// `Vec` doesn't guarantee a particular growth strategy, additional memory
/// might be allocated but not used. Since the `WeightedIndex` object also
/// contains, this might cause additional allocations, though for primitive
/// types, [`Uniform<X>`] doesn't allocate any memory.
///
/// Sampling from `WeightedIndex` will result in a single call to
/// `Uniform<X>::sample` (method of the [`Distribution`] trait), which typically
/// will request a single value from the underlying [`RngCore`], though the
/// exact number depends on the implementation of `Uniform<X>::sample`.
///
/// # Example
///
/// ```
/// use rand::prelude::*;
/// use rand::distributions::WeightedIndex;
///
/// let choices = ['a', 'b', 'c'];
/// let weights = [2,   1,   1];
/// let dist = WeightedIndex::new(&weights).unwrap();
/// let mut rng = thread_rng();
/// for _ in 0..100 {
///     // 50% chance to print 'a', 25% chance to print 'b', 25% chance to print 'c'
///     println!("{}", choices[dist.sample(&mut rng)]);
/// }
///
/// let items = [('a', 0), ('b', 3), ('c', 7)];
/// let dist2 = WeightedIndex::new(items.iter().map(|item| item.1)).unwrap();
/// for _ in 0..100 {
///     // 0% chance to print 'a', 30% chance to print 'b', 70% chance to print 'c'
///     println!("{}", items[dist2.sample(&mut rng)].0);
/// }
/// ```
///
/// [`Uniform<X>`]: crate::distributions::Uniform
/// [`RngCore`]: crate::RngCore
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(doc_cfg, doc(cfg(feature = "alloc")))]
pub struct WeightedIndex<X: SampleUniform + PartialOrd> {
    cumulative_weights: Vec<X>,
    total_weight: X,
    weight_distribution: X::Sampler,
}

impl<X: SampleUniform + PartialOrd> WeightedIndex<X> {
    /// Creates a new a `WeightedIndex` [`Distribution`] using the values
    /// in `weights`. The weights can use any type `X` for which an
    /// implementation of [`Uniform<X>`] exists.
    ///
    /// Returns an error if the iterator is empty, if any weight is `< 0`, or
    /// if its total value is 0.
    ///
    /// [`Uniform<X>`]: crate::distributions::uniform::Uniform
    pub fn new<I>(weights: I) -> Result<WeightedIndex<X>, WeightedError>
    where
        I: IntoIterator,
        I::Item: SampleBorrow<X>,
        X: for<'a> ::core::ops::AddAssign<&'a X> + Clone + Default,
    {
        let mut iter = weights.into_iter();
        let mut total_weight: X = iter.next().ok_or(WeightedError::NoItem)?.borrow().clone();

        let zero = <X as Default>::default();
        if !(total_weight >= zero) {
            return Err(WeightedError::InvalidWeight);
        }

        let mut weights = Vec::<X>::with_capacity(iter.size_hint().0);
        for w in iter {
            // Note that `!(w >= x)` is not equivalent to `w < x` for partially
            // ordered types due to NaNs which are equal to nothing.
            if !(w.borrow() >= &zero) {
                return Err(WeightedError::InvalidWeight);
            }
            weights.push(total_weight.clone());
            total_weight += w.borrow();
        }

        if total_weight == zero {
            return Err(WeightedError::AllWeightsZero);
        }
        let distr = X::Sampler::new(zero, total_weight.clone());

        Ok(WeightedIndex {
            cumulative_weights: weights,
            total_weight,
            weight_distribution: distr,
        })
    }

    /// Update a subset of weights, without changing the number of weights.
    ///
    /// `new_weights` must be sorted by the index.
    ///
    /// Using this method instead of `new` might be more efficient if only a small number of
    /// weights is modified. No allocations are performed, unless the weight type `X` uses
    /// allocation internally.
    ///
    /// In case of error, `self` is not modified.
    pub fn update_weights(&mut self, new_weights: &[(usize, &X)]) -> Result<(), WeightedError>
    where X: for<'a> ::core::ops::AddAssign<&'a X>
            + for<'a> ::core::ops::SubAssign<&'a X>
            + Clone
            + Default {
        if new_weights.is_empty() {
            return Ok(());
        }

        let zero = <X as Default>::default();

        let mut total_weight = self.total_weight.clone();

        // Check for errors first, so we don't modify `self` in case something
        // goes wrong.
        let mut prev_i = None;
        for &(i, w) in new_weights {
            if let Some(old_i) = prev_i {
                if old_i >= i {
                    return Err(WeightedError::InvalidWeight);
                }
            }
            if !(*w >= zero) {
                return Err(WeightedError::InvalidWeight);
            }
            if i > self.cumulative_weights.len() {
                return Err(WeightedError::TooMany);
            }

            let mut old_w = if i < self.cumulative_weights.len() {
                self.cumulative_weights[i].clone()
            } else {
                self.total_weight.clone()
            };
            if i > 0 {
                old_w -= &self.cumulative_weights[i - 1];
            }

            total_weight -= &old_w;
            total_weight += w;
            prev_i = Some(i);
        }
        if total_weight <= zero {
            return Err(WeightedError::AllWeightsZero);
        }

        // Update the weights. Because we checked all the preconditions in the
        // previous loop, this should never panic.
        let mut iter = new_weights.iter();

        let mut prev_weight = zero.clone();
        let mut next_new_weight = iter.next();
        let &(first_new_index, _) = next_new_weight.unwrap();
        let mut cumulative_weight = if first_new_index > 0 {
            self.cumulative_weights[first_new_index - 1].clone()
        } else {
            zero.clone()
        };
        for i in first_new_index..self.cumulative_weights.len() {
            match next_new_weight {
                Some(&(j, w)) if i == j => {
                    cumulative_weight += w;
                    next_new_weight = iter.next();
                }
                _ => {
                    let mut tmp = self.cumulative_weights[i].clone();
                    tmp -= &prev_weight; // We know this is positive.
                    cumulative_weight += &tmp;
                }
            }
            prev_weight = cumulative_weight.clone();
            core::mem::swap(&mut prev_weight, &mut self.cumulative_weights[i]);
        }

        self.total_weight = total_weight;
        self.weight_distribution = X::Sampler::new(zero, self.total_weight.clone());

        Ok(())
    }
}

impl<X> Distribution<usize> for WeightedIndex<X>
where X: SampleUniform + PartialOrd
{
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> usize {
        use ::core::cmp::Ordering;
        let chosen_weight = self.weight_distribution.sample(rng);
        // Find the first item which has a weight *higher* than the chosen weight.
        self.cumulative_weights
            .binary_search_by(|w| {
                if *w <= chosen_weight {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            })
            .unwrap_err()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(feature = "serde1")]
    #[test]
    fn test_weightedindex_serde1() {
        let weighted_index = WeightedIndex::new(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).unwrap();

        let ser_weighted_index = bincode::serialize(&weighted_index).unwrap();
        let de_weighted_index: WeightedIndex<i32> =
            bincode::deserialize(&ser_weighted_index).unwrap();

        assert_eq!(
            de_weighted_index.cumulative_weights,
            weighted_index.cumulative_weights
        );
        assert_eq!(de_weighted_index.total_weight, weighted_index.total_weight);
    }

    #[test]
    fn test_accepting_nan(){
        assert_eq!(
            WeightedIndex::new(&[core::f32::NAN, 0.5]).unwrap_err(),
            WeightedError::InvalidWeight,
        );
        assert_eq!(
            WeightedIndex::new(&[core::f32::NAN]).unwrap_err(),
            WeightedError::InvalidWeight,
        );
        assert_eq!(
            WeightedIndex::new(&[0.5, core::f32::NAN]).unwrap_err(),
            WeightedError::InvalidWeight,
        );

        assert_eq!(
            WeightedIndex::new(&[0.5, 7.0])
                .unwrap()
                .update_weights(&[(0, &core::f32::NAN)])
                .unwrap_err(),
            WeightedError::InvalidWeight,
        )
    }


    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_weightedindex() {
        let mut r = crate::test::rng(700);
        const N_REPS: u32 = 5000;
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

        // WeightedIndex from vec
        let mut chosen = [0i32; 14];
        let distr = WeightedIndex::new(weights.to_vec()).unwrap();
        for _ in 0..N_REPS {
            chosen[distr.sample(&mut r)] += 1;
        }
        verify(chosen);

        // WeightedIndex from slice
        chosen = [0i32; 14];
        let distr = WeightedIndex::new(&weights[..]).unwrap();
        for _ in 0..N_REPS {
            chosen[distr.sample(&mut r)] += 1;
        }
        verify(chosen);

        // WeightedIndex from iterator
        chosen = [0i32; 14];
        let distr = WeightedIndex::new(weights.iter()).unwrap();
        for _ in 0..N_REPS {
            chosen[distr.sample(&mut r)] += 1;
        }
        verify(chosen);

        for _ in 0..5 {
            assert_eq!(WeightedIndex::new(&[0, 1]).unwrap().sample(&mut r), 1);
            assert_eq!(WeightedIndex::new(&[1, 0]).unwrap().sample(&mut r), 0);
            assert_eq!(
                WeightedIndex::new(&[0, 0, 0, 0, 10, 0])
                    .unwrap()
                    .sample(&mut r),
                4
            );
        }

        assert_eq!(
            WeightedIndex::new(&[10][0..0]).unwrap_err(),
            WeightedError::NoItem
        );
        assert_eq!(
            WeightedIndex::new(&[0]).unwrap_err(),
            WeightedError::AllWeightsZero
        );
        assert_eq!(
            WeightedIndex::new(&[10, 20, -1, 30]).unwrap_err(),
            WeightedError::InvalidWeight
        );
        assert_eq!(
            WeightedIndex::new(&[-10, 20, 1, 30]).unwrap_err(),
            WeightedError::InvalidWeight
        );
        assert_eq!(
            WeightedIndex::new(&[-10]).unwrap_err(),
            WeightedError::InvalidWeight
        );
    }

    #[test]
    fn test_update_weights() {
        let data = [
            (
                &[10u32, 2, 3, 4][..],
                &[(1, &100), (2, &4)][..], // positive change
                &[10, 100, 4, 4][..],
            ),
            (
                &[1u32, 2, 3, 0, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7][..],
                &[(2, &1), (5, &1), (13, &100)][..], // negative change and last element
                &[1u32, 2, 1, 0, 5, 1, 7, 1, 2, 3, 4, 5, 6, 100][..],
            ),
        ];

        for (weights, update, expected_weights) in data.iter() {
            let total_weight = weights.iter().sum::<u32>();
            let mut distr = WeightedIndex::new(weights.to_vec()).unwrap();
            assert_eq!(distr.total_weight, total_weight);

            distr.update_weights(update).unwrap();
            let expected_total_weight = expected_weights.iter().sum::<u32>();
            let expected_distr = WeightedIndex::new(expected_weights.to_vec()).unwrap();
            assert_eq!(distr.total_weight, expected_total_weight);
            assert_eq!(distr.total_weight, expected_distr.total_weight);
            assert_eq!(distr.cumulative_weights, expected_distr.cumulative_weights);
        }
    }

    #[test]
    fn value_stability() {
        fn test_samples<X: SampleUniform + PartialOrd, I>(
            weights: I, buf: &mut [usize], expected: &[usize],
        ) where
            I: IntoIterator,
            I::Item: SampleBorrow<X>,
            X: for<'a> ::core::ops::AddAssign<&'a X> + Clone + Default,
        {
            assert_eq!(buf.len(), expected.len());
            let distr = WeightedIndex::new(weights).unwrap();
            let mut rng = crate::test::rng(701);
            for r in buf.iter_mut() {
                *r = rng.sample(&distr);
            }
            assert_eq!(buf, expected);
        }

        let mut buf = [0; 10];
        test_samples(&[1i32, 1, 1, 1, 1, 1, 1, 1, 1], &mut buf, &[
            0, 6, 2, 6, 3, 4, 7, 8, 2, 5,
        ]);
        test_samples(&[0.7f32, 0.1, 0.1, 0.1], &mut buf, &[
            0, 0, 0, 1, 0, 0, 2, 3, 0, 0,
        ]);
        test_samples(&[1.0f64, 0.999, 0.998, 0.997], &mut buf, &[
            2, 2, 1, 3, 2, 1, 3, 3, 2, 1,
        ]);
    }

    #[test]
    fn weighted_index_distributions_can_be_compared() {
        assert_eq!(WeightedIndex::new(&[1, 2]), WeightedIndex::new(&[1, 2]));
    }
}

/// Error type returned from `WeightedIndex::new`.
#[cfg_attr(doc_cfg, doc(cfg(feature = "alloc")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightedError {
    /// The provided weight collection contains no items.
    NoItem,

    /// A weight is either less than zero, greater than the supported maximum,
    /// NaN, or otherwise invalid.
    InvalidWeight,

    /// All items in the provided weight collection are zero.
    AllWeightsZero,

    /// Too many weights are provided (length greater than `u32::MAX`)
    TooMany,
}

#[cfg(feature = "std")]
impl std::error::Error for WeightedError {}

impl fmt::Display for WeightedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            WeightedError::NoItem => "No weights provided in distribution",
            WeightedError::InvalidWeight => "A weight is invalid in distribution",
            WeightedError::AllWeightsZero => "All weights are zero in distribution",
            WeightedError::TooMany => "Too many weights (hit u32::MAX) in distribution",
        })
    }
}
