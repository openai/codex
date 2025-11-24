// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Low-level API for sampling indices
use alloc::vec::{self, Vec};
use core::slice;
use core::{hash::Hash, ops::AddAssign};
// BTreeMap is not as fast in tests, but better than nothing.
#[cfg(feature = "std")]
use super::WeightError;
use crate::distr::uniform::SampleUniform;
use crate::distr::{Distribution, Uniform};
use crate::Rng;
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeSet;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::collections::HashSet;

#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!("unsupported pointer width");

/// A vector of indices.
///
/// Multiple internal representations are possible.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IndexVec {
    #[doc(hidden)]
    U32(Vec<u32>),
    #[cfg(target_pointer_width = "64")]
    #[doc(hidden)]
    U64(Vec<u64>),
}

impl IndexVec {
    /// Returns the number of indices
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            IndexVec::U32(v) => v.len(),
            #[cfg(target_pointer_width = "64")]
            IndexVec::U64(v) => v.len(),
        }
    }

    /// Returns `true` if the length is 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            IndexVec::U32(v) => v.is_empty(),
            #[cfg(target_pointer_width = "64")]
            IndexVec::U64(v) => v.is_empty(),
        }
    }

    /// Return the value at the given `index`.
    ///
    /// (Note: we cannot implement [`std::ops::Index`] because of lifetime
    /// restrictions.)
    #[inline]
    pub fn index(&self, index: usize) -> usize {
        match self {
            IndexVec::U32(v) => v[index] as usize,
            #[cfg(target_pointer_width = "64")]
            IndexVec::U64(v) => v[index] as usize,
        }
    }

    /// Return result as a `Vec<usize>`. Conversion may or may not be trivial.
    #[inline]
    pub fn into_vec(self) -> Vec<usize> {
        match self {
            IndexVec::U32(v) => v.into_iter().map(|i| i as usize).collect(),
            #[cfg(target_pointer_width = "64")]
            IndexVec::U64(v) => v.into_iter().map(|i| i as usize).collect(),
        }
    }

    /// Iterate over the indices as a sequence of `usize` values
    #[inline]
    pub fn iter(&self) -> IndexVecIter<'_> {
        match self {
            IndexVec::U32(v) => IndexVecIter::U32(v.iter()),
            #[cfg(target_pointer_width = "64")]
            IndexVec::U64(v) => IndexVecIter::U64(v.iter()),
        }
    }
}

impl IntoIterator for IndexVec {
    type IntoIter = IndexVecIntoIter;
    type Item = usize;

    /// Convert into an iterator over the indices as a sequence of `usize` values
    #[inline]
    fn into_iter(self) -> IndexVecIntoIter {
        match self {
            IndexVec::U32(v) => IndexVecIntoIter::U32(v.into_iter()),
            #[cfg(target_pointer_width = "64")]
            IndexVec::U64(v) => IndexVecIntoIter::U64(v.into_iter()),
        }
    }
}

impl PartialEq for IndexVec {
    fn eq(&self, other: &IndexVec) -> bool {
        use self::IndexVec::*;
        match (self, other) {
            (U32(v1), U32(v2)) => v1 == v2,
            #[cfg(target_pointer_width = "64")]
            (U64(v1), U64(v2)) => v1 == v2,
            #[cfg(target_pointer_width = "64")]
            (U32(v1), U64(v2)) => {
                (v1.len() == v2.len()) && (v1.iter().zip(v2.iter()).all(|(x, y)| *x as u64 == *y))
            }
            #[cfg(target_pointer_width = "64")]
            (U64(v1), U32(v2)) => {
                (v1.len() == v2.len()) && (v1.iter().zip(v2.iter()).all(|(x, y)| *x == *y as u64))
            }
        }
    }
}

impl From<Vec<u32>> for IndexVec {
    #[inline]
    fn from(v: Vec<u32>) -> Self {
        IndexVec::U32(v)
    }
}

#[cfg(target_pointer_width = "64")]
impl From<Vec<u64>> for IndexVec {
    #[inline]
    fn from(v: Vec<u64>) -> Self {
        IndexVec::U64(v)
    }
}

/// Return type of `IndexVec::iter`.
#[derive(Debug)]
pub enum IndexVecIter<'a> {
    #[doc(hidden)]
    U32(slice::Iter<'a, u32>),
    #[cfg(target_pointer_width = "64")]
    #[doc(hidden)]
    U64(slice::Iter<'a, u64>),
}

impl Iterator for IndexVecIter<'_> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        use self::IndexVecIter::*;
        match self {
            U32(iter) => iter.next().map(|i| *i as usize),
            #[cfg(target_pointer_width = "64")]
            U64(iter) => iter.next().map(|i| *i as usize),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            IndexVecIter::U32(v) => v.size_hint(),
            #[cfg(target_pointer_width = "64")]
            IndexVecIter::U64(v) => v.size_hint(),
        }
    }
}

impl ExactSizeIterator for IndexVecIter<'_> {}

/// Return type of `IndexVec::into_iter`.
#[derive(Clone, Debug)]
pub enum IndexVecIntoIter {
    #[doc(hidden)]
    U32(vec::IntoIter<u32>),
    #[cfg(target_pointer_width = "64")]
    #[doc(hidden)]
    U64(vec::IntoIter<u64>),
}

impl Iterator for IndexVecIntoIter {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        use self::IndexVecIntoIter::*;
        match self {
            U32(v) => v.next().map(|i| i as usize),
            #[cfg(target_pointer_width = "64")]
            U64(v) => v.next().map(|i| i as usize),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        use self::IndexVecIntoIter::*;
        match self {
            U32(v) => v.size_hint(),
            #[cfg(target_pointer_width = "64")]
            U64(v) => v.size_hint(),
        }
    }
}

impl ExactSizeIterator for IndexVecIntoIter {}

/// Randomly sample exactly `amount` distinct indices from `0..length`, and
/// return them in random order (fully shuffled).
///
/// This method is used internally by the slice sampling methods, but it can
/// sometimes be useful to have the indices themselves so this is provided as
/// an alternative.
///
/// The implementation used is not specified; we automatically select the
/// fastest available algorithm for the `length` and `amount` parameters
/// (based on detailed profiling on an Intel Haswell CPU). Roughly speaking,
/// complexity is `O(amount)`, except that when `amount` is small, performance
/// is closer to `O(amount^2)`, and when `length` is close to `amount` then
/// `O(length)`.
///
/// Note that performance is significantly better over `u32` indices than over
/// `u64` indices. Because of this we hide the underlying type behind an
/// abstraction, `IndexVec`.
///
/// If an allocation-free `no_std` function is required, it is suggested
/// to adapt the internal `sample_floyd` implementation.
///
/// Panics if `amount > length`.
#[track_caller]
pub fn sample<R>(rng: &mut R, length: usize, amount: usize) -> IndexVec
where
    R: Rng + ?Sized,
{
    if amount > length {
        panic!("`amount` of samples must be less than or equal to `length`");
    }
    if length > (u32::MAX as usize) {
        #[cfg(target_pointer_width = "32")]
        unreachable!();

        // We never want to use inplace here, but could use floyd's alg
        // Lazy version: always use the cache alg.
        #[cfg(target_pointer_width = "64")]
        return sample_rejection(rng, length as u64, amount as u64);
    }
    let amount = amount as u32;
    let length = length as u32;

    // Choice of algorithm here depends on both length and amount. See:
    // https://github.com/rust-random/rand/pull/479
    // We do some calculations with f32. Accuracy is not very important.

    if amount < 163 {
        const C: [[f32; 2]; 2] = [[1.6, 8.0 / 45.0], [10.0, 70.0 / 9.0]];
        let j = usize::from(length >= 500_000);
        let amount_fp = amount as f32;
        let m4 = C[0][j] * amount_fp;
        // Short-cut: when amount < 12, floyd's is always faster
        if amount > 11 && (length as f32) < (C[1][j] + m4) * amount_fp {
            sample_inplace(rng, length, amount)
        } else {
            sample_floyd(rng, length, amount)
        }
    } else {
        const C: [f32; 2] = [270.0, 330.0 / 9.0];
        let j = usize::from(length >= 500_000);
        if (length as f32) < C[j] * (amount as f32) {
            sample_inplace(rng, length, amount)
        } else {
            sample_rejection(rng, length, amount)
        }
    }
}

/// Randomly sample `amount` distinct indices from `0..length`
///
/// The result may contain less than `amount` indices if insufficient non-zero
/// weights are available. Results are returned in an arbitrary order (there is
/// no guarantee of shuffling or ordering).
///
/// Function `weight` is called once for each index to provide weights.
///
/// This method is used internally by the slice sampling methods, but it can
/// sometimes be useful to have the indices themselves so this is provided as
/// an alternative.
///
/// Error cases:
/// -   [`WeightError::InvalidWeight`] when a weight is not-a-number or negative.
///
/// This implementation uses `O(length + amount)` space and `O(length)` time.
#[cfg(feature = "std")]
pub fn sample_weighted<R, F, X>(
    rng: &mut R,
    length: usize,
    weight: F,
    amount: usize,
) -> Result<IndexVec, WeightError>
where
    R: Rng + ?Sized,
    F: Fn(usize) -> X,
    X: Into<f64>,
{
    if length > (u32::MAX as usize) {
        #[cfg(target_pointer_width = "32")]
        unreachable!();

        #[cfg(target_pointer_width = "64")]
        {
            let amount = amount as u64;
            let length = length as u64;
            sample_efraimidis_spirakis(rng, length, weight, amount)
        }
    } else {
        assert!(amount <= u32::MAX as usize);
        let amount = amount as u32;
        let length = length as u32;
        sample_efraimidis_spirakis(rng, length, weight, amount)
    }
}

/// Randomly sample `amount` distinct indices from `0..length`
///
/// The result may contain less than `amount` indices if insufficient non-zero
/// weights are available. Results are returned in an arbitrary order (there is
/// no guarantee of shuffling or ordering).
///
/// Function `weight` is called once for each index to provide weights.
///
/// This implementation is based on the algorithm A-ExpJ as found in
/// [Efraimidis and Spirakis, 2005](https://doi.org/10.1016/j.ipl.2005.11.003).
/// It uses `O(length + amount)` space and `O(length)` time.
///
/// Error cases:
/// -   [`WeightError::InvalidWeight`] when a weight is not-a-number or negative.
#[cfg(feature = "std")]
fn sample_efraimidis_spirakis<R, F, X, N>(
    rng: &mut R,
    length: N,
    weight: F,
    amount: N,
) -> Result<IndexVec, WeightError>
where
    R: Rng + ?Sized,
    F: Fn(usize) -> X,
    X: Into<f64>,
    N: UInt,
    IndexVec: From<Vec<N>>,
{
    use std::{cmp::Ordering, collections::BinaryHeap};

    if amount == N::zero() {
        return Ok(IndexVec::U32(Vec::new()));
    }

    struct Element<N> {
        index: N,
        key: f64,
    }

    impl<N> PartialOrd for Element<N> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl<N> Ord for Element<N> {
        fn cmp(&self, other: &Self) -> Ordering {
            // unwrap() should not panic since weights should not be NaN
            // We reverse so that BinaryHeap::peek shows the smallest item
            self.key.partial_cmp(&other.key).unwrap().reverse()
        }
    }

    impl<N> PartialEq for Element<N> {
        fn eq(&self, other: &Self) -> bool {
            self.key == other.key
        }
    }

    impl<N> Eq for Element<N> {}

    let mut candidates = BinaryHeap::with_capacity(amount.as_usize());
    let mut index = N::zero();
    while index < length && candidates.len() < amount.as_usize() {
        let weight = weight(index.as_usize()).into();
        if weight > 0.0 {
            // We use the log of the key used in A-ExpJ to improve precision
            // for small weights:
            let key = rng.random::<f64>().ln() / weight;
            candidates.push(Element { index, key });
        } else if !(weight >= 0.0) {
            return Err(WeightError::InvalidWeight);
        }

        index += N::one();
    }

    if index < length {
        let mut x = rng.random::<f64>().ln() / candidates.peek().unwrap().key;
        while index < length {
            let weight = weight(index.as_usize()).into();
            if weight > 0.0 {
                x -= weight;
                if x <= 0.0 {
                    let min_candidate = candidates.pop().unwrap();
                    let t = (min_candidate.key * weight).exp();
                    let key = rng.random_range(t..1.0).ln() / weight;
                    candidates.push(Element { index, key });

                    x = rng.random::<f64>().ln() / candidates.peek().unwrap().key;
                }
            } else if !(weight >= 0.0) {
                return Err(WeightError::InvalidWeight);
            }

            index += N::one();
        }
    }

    Ok(IndexVec::from(
        candidates.iter().map(|elt| elt.index).collect(),
    ))
}

/// Randomly sample exactly `amount` indices from `0..length`, using Floyd's
/// combination algorithm.
///
/// The output values are fully shuffled. (Overhead is under 50%.)
///
/// This implementation uses `O(amount)` memory and `O(amount^2)` time.
fn sample_floyd<R>(rng: &mut R, length: u32, amount: u32) -> IndexVec
where
    R: Rng + ?Sized,
{
    // Note that the values returned by `rng.random_range()` can be
    // inferred from the returned vector by working backwards from
    // the last entry. This bijection proves the algorithm fair.
    debug_assert!(amount <= length);
    let mut indices = Vec::with_capacity(amount as usize);
    for j in length - amount..length {
        let t = rng.random_range(..=j);
        if let Some(pos) = indices.iter().position(|&x| x == t) {
            indices[pos] = j;
        }
        indices.push(t);
    }
    IndexVec::from(indices)
}

/// Randomly sample exactly `amount` indices from `0..length`, using an inplace
/// partial Fisher-Yates method.
/// Sample an amount of indices using an inplace partial fisher yates method.
///
/// This allocates the entire `length` of indices and randomizes only the first `amount`.
/// It then truncates to `amount` and returns.
///
/// This method is not appropriate for large `length` and potentially uses a lot
/// of memory; because of this we only implement for `u32` index (which improves
/// performance in all cases).
///
/// Set-up is `O(length)` time and memory and shuffling is `O(amount)` time.
fn sample_inplace<R>(rng: &mut R, length: u32, amount: u32) -> IndexVec
where
    R: Rng + ?Sized,
{
    debug_assert!(amount <= length);
    let mut indices: Vec<u32> = Vec::with_capacity(length as usize);
    indices.extend(0..length);
    for i in 0..amount {
        let j: u32 = rng.random_range(i..length);
        indices.swap(i as usize, j as usize);
    }
    indices.truncate(amount as usize);
    debug_assert_eq!(indices.len(), amount as usize);
    IndexVec::from(indices)
}

trait UInt: Copy + PartialOrd + Ord + PartialEq + Eq + SampleUniform + Hash + AddAssign {
    fn zero() -> Self;
    #[cfg_attr(feature = "alloc", allow(dead_code))]
    fn one() -> Self;
    fn as_usize(self) -> usize;
}

impl UInt for u32 {
    #[inline]
    fn zero() -> Self {
        0
    }

    #[inline]
    fn one() -> Self {
        1
    }

    #[inline]
    fn as_usize(self) -> usize {
        self as usize
    }
}

#[cfg(target_pointer_width = "64")]
impl UInt for u64 {
    #[inline]
    fn zero() -> Self {
        0
    }

    #[inline]
    fn one() -> Self {
        1
    }

    #[inline]
    fn as_usize(self) -> usize {
        self as usize
    }
}

/// Randomly sample exactly `amount` indices from `0..length`, using rejection
/// sampling.
///
/// Since `amount <<< length` there is a low chance of a random sample in
/// `0..length` being a duplicate. We test for duplicates and resample where
/// necessary. The algorithm is `O(amount)` time and memory.
///
/// This function  is generic over X primarily so that results are value-stable
/// over 32-bit and 64-bit platforms.
fn sample_rejection<X: UInt, R>(rng: &mut R, length: X, amount: X) -> IndexVec
where
    R: Rng + ?Sized,
    IndexVec: From<Vec<X>>,
{
    debug_assert!(amount < length);
    #[cfg(feature = "std")]
    let mut cache = HashSet::with_capacity(amount.as_usize());
    #[cfg(not(feature = "std"))]
    let mut cache = BTreeSet::new();
    let distr = Uniform::new(X::zero(), length).unwrap();
    let mut indices = Vec::with_capacity(amount.as_usize());
    for _ in 0..amount.as_usize() {
        let mut pos = distr.sample(rng);
        while !cache.insert(pos) {
            pos = distr.sample(rng);
        }
        indices.push(pos);
    }

    debug_assert_eq!(indices.len(), amount.as_usize());
    IndexVec::from(indices)
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;

    #[test]
    #[cfg(feature = "serde")]
    fn test_serialization_index_vec() {
        let some_index_vec = IndexVec::from(vec![254_u32, 234, 2, 1]);
        let de_some_index_vec: IndexVec =
            bincode::deserialize(&bincode::serialize(&some_index_vec).unwrap()).unwrap();
        assert_eq!(some_index_vec, de_some_index_vec);
    }

    #[test]
    fn test_sample_boundaries() {
        let mut r = crate::test::rng(404);

        assert_eq!(sample_inplace(&mut r, 0, 0).len(), 0);
        assert_eq!(sample_inplace(&mut r, 1, 0).len(), 0);
        assert_eq!(sample_inplace(&mut r, 1, 1).into_vec(), vec![0]);

        assert_eq!(sample_rejection(&mut r, 1u32, 0).len(), 0);

        assert_eq!(sample_floyd(&mut r, 0, 0).len(), 0);
        assert_eq!(sample_floyd(&mut r, 1, 0).len(), 0);
        assert_eq!(sample_floyd(&mut r, 1, 1).into_vec(), vec![0]);

        // These algorithms should be fast with big numbers. Test average.
        let sum: usize = sample_rejection(&mut r, 1 << 25, 10u32).into_iter().sum();
        assert!(1 << 25 < sum && sum < (1 << 25) * 25);

        let sum: usize = sample_floyd(&mut r, 1 << 25, 10).into_iter().sum();
        assert!(1 << 25 < sum && sum < (1 << 25) * 25);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_sample_alg() {
        let seed_rng = crate::test::rng;

        // We can't test which algorithm is used directly, but Floyd's alg
        // should produce different results from the others. (Also, `inplace`
        // and `cached` currently use different sizes thus produce different results.)

        // A small length and relatively large amount should use inplace
        let (length, amount): (usize, usize) = (100, 50);
        let v1 = sample(&mut seed_rng(420), length, amount);
        let v2 = sample_inplace(&mut seed_rng(420), length as u32, amount as u32);
        assert!(v1.iter().all(|e| e < length));
        assert_eq!(v1, v2);

        // Test Floyd's alg does produce different results
        let v3 = sample_floyd(&mut seed_rng(420), length as u32, amount as u32);
        assert!(v1 != v3);

        // A large length and small amount should use Floyd
        let (length, amount): (usize, usize) = (1 << 20, 50);
        let v1 = sample(&mut seed_rng(421), length, amount);
        let v2 = sample_floyd(&mut seed_rng(421), length as u32, amount as u32);
        assert!(v1.iter().all(|e| e < length));
        assert_eq!(v1, v2);

        // A large length and larger amount should use cache
        let (length, amount): (usize, usize) = (1 << 20, 600);
        let v1 = sample(&mut seed_rng(422), length, amount);
        let v2 = sample_rejection(&mut seed_rng(422), length as u32, amount as u32);
        assert!(v1.iter().all(|e| e < length));
        assert_eq!(v1, v2);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_sample_weighted() {
        let seed_rng = crate::test::rng;
        for &(amount, len) in &[(0, 10), (5, 10), (9, 10)] {
            let v = sample_weighted(&mut seed_rng(423), len, |i| i as f64, amount).unwrap();
            match v {
                IndexVec::U32(mut indices) => {
                    assert_eq!(indices.len(), amount);
                    indices.sort_unstable();
                    indices.dedup();
                    assert_eq!(indices.len(), amount);
                    for &i in &indices {
                        assert!((i as usize) < len);
                    }
                }
                #[cfg(target_pointer_width = "64")]
                _ => panic!("expected `IndexVec::U32`"),
            }
        }

        let r = sample_weighted(&mut seed_rng(423), 10, |i| i as f64, 10);
        assert_eq!(r.unwrap().len(), 9);
    }

    #[test]
    fn value_stability_sample() {
        let do_test = |length, amount, values: &[u32]| {
            let mut buf = [0u32; 8];
            let mut rng = crate::test::rng(410);

            let res = sample(&mut rng, length, amount);
            let len = res.len().min(buf.len());
            for (x, y) in res.into_iter().zip(buf.iter_mut()) {
                *y = x as u32;
            }
            assert_eq!(
                &buf[0..len],
                values,
                "failed sampling {}, {}",
                length,
                amount
            );
        };

        do_test(10, 6, &[0, 9, 5, 4, 6, 8]); // floyd
        do_test(25, 10, &[24, 20, 19, 9, 22, 16, 0, 14]); // floyd
        do_test(300, 8, &[30, 283, 243, 150, 218, 240, 1, 189]); // floyd
        do_test(300, 80, &[31, 289, 248, 154, 221, 243, 7, 192]); // inplace
        do_test(300, 180, &[31, 289, 248, 154, 221, 243, 7, 192]); // inplace

        do_test(
            1_000_000,
            8,
            &[103717, 963485, 826422, 509101, 736394, 807035, 5327, 632573],
        ); // floyd
        do_test(
            1_000_000,
            180,
            &[103718, 963490, 826426, 509103, 736396, 807036, 5327, 632573],
        ); // rejection
    }
}
