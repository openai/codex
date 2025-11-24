// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Low-level API for sampling indices

#[cfg(feature = "alloc")] use core::slice;

#[cfg(feature = "alloc")] use alloc::vec::{self, Vec};
// BTreeMap is not as fast in tests, but better than nothing.
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::collections::BTreeSet;
#[cfg(feature = "std")] use std::collections::HashSet;

#[cfg(feature = "std")]
use crate::distributions::WeightedError;

#[cfg(feature = "alloc")]
use crate::{Rng, distributions::{uniform::SampleUniform, Distribution, Uniform}};

#[cfg(feature = "serde1")]
use serde::{Serialize, Deserialize};

/// A vector of indices.
///
/// Multiple internal representations are possible.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
pub enum IndexVec {
    #[doc(hidden)]
    U32(Vec<u32>),
    #[doc(hidden)]
    USize(Vec<usize>),
}

impl IndexVec {
    /// Returns the number of indices
    #[inline]
    pub fn len(&self) -> usize {
        match *self {
            IndexVec::U32(ref v) => v.len(),
            IndexVec::USize(ref v) => v.len(),
        }
    }

    /// Returns `true` if the length is 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        match *self {
            IndexVec::U32(ref v) => v.is_empty(),
            IndexVec::USize(ref v) => v.is_empty(),
        }
    }

    /// Return the value at the given `index`.
    ///
    /// (Note: we cannot implement [`std::ops::Index`] because of lifetime
    /// restrictions.)
    #[inline]
    pub fn index(&self, index: usize) -> usize {
        match *self {
            IndexVec::U32(ref v) => v[index] as usize,
            IndexVec::USize(ref v) => v[index],
        }
    }

    /// Return result as a `Vec<usize>`. Conversion may or may not be trivial.
    #[inline]
    pub fn into_vec(self) -> Vec<usize> {
        match self {
            IndexVec::U32(v) => v.into_iter().map(|i| i as usize).collect(),
            IndexVec::USize(v) => v,
        }
    }

    /// Iterate over the indices as a sequence of `usize` values
    #[inline]
    pub fn iter(&self) -> IndexVecIter<'_> {
        match *self {
            IndexVec::U32(ref v) => IndexVecIter::U32(v.iter()),
            IndexVec::USize(ref v) => IndexVecIter::USize(v.iter()),
        }
    }
}

impl IntoIterator for IndexVec {
    type Item = usize;
    type IntoIter = IndexVecIntoIter;

    /// Convert into an iterator over the indices as a sequence of `usize` values
    #[inline]
    fn into_iter(self) -> IndexVecIntoIter {
        match self {
            IndexVec::U32(v) => IndexVecIntoIter::U32(v.into_iter()),
            IndexVec::USize(v) => IndexVecIntoIter::USize(v.into_iter()),
        }
    }
}

impl PartialEq for IndexVec {
    fn eq(&self, other: &IndexVec) -> bool {
        use self::IndexVec::*;
        match (self, other) {
            (&U32(ref v1), &U32(ref v2)) => v1 == v2,
            (&USize(ref v1), &USize(ref v2)) => v1 == v2,
            (&U32(ref v1), &USize(ref v2)) => {
                (v1.len() == v2.len()) && (v1.iter().zip(v2.iter()).all(|(x, y)| *x as usize == *y))
            }
            (&USize(ref v1), &U32(ref v2)) => {
                (v1.len() == v2.len()) && (v1.iter().zip(v2.iter()).all(|(x, y)| *x == *y as usize))
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

impl From<Vec<usize>> for IndexVec {
    #[inline]
    fn from(v: Vec<usize>) -> Self {
        IndexVec::USize(v)
    }
}

/// Return type of `IndexVec::iter`.
#[derive(Debug)]
pub enum IndexVecIter<'a> {
    #[doc(hidden)]
    U32(slice::Iter<'a, u32>),
    #[doc(hidden)]
    USize(slice::Iter<'a, usize>),
}

impl<'a> Iterator for IndexVecIter<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        use self::IndexVecIter::*;
        match *self {
            U32(ref mut iter) => iter.next().map(|i| *i as usize),
            USize(ref mut iter) => iter.next().cloned(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match *self {
            IndexVecIter::U32(ref v) => v.size_hint(),
            IndexVecIter::USize(ref v) => v.size_hint(),
        }
    }
}

impl<'a> ExactSizeIterator for IndexVecIter<'a> {}

/// Return type of `IndexVec::into_iter`.
#[derive(Clone, Debug)]
pub enum IndexVecIntoIter {
    #[doc(hidden)]
    U32(vec::IntoIter<u32>),
    #[doc(hidden)]
    USize(vec::IntoIter<usize>),
}

impl Iterator for IndexVecIntoIter {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        use self::IndexVecIntoIter::*;
        match *self {
            U32(ref mut v) => v.next().map(|i| i as usize),
            USize(ref mut v) => v.next(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        use self::IndexVecIntoIter::*;
        match *self {
            U32(ref v) => v.size_hint(),
            USize(ref v) => v.size_hint(),
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
pub fn sample<R>(rng: &mut R, length: usize, amount: usize) -> IndexVec
where R: Rng + ?Sized {
    if amount > length {
        panic!("`amount` of samples must be less than or equal to `length`");
    }
    if length > (::core::u32::MAX as usize) {
        // We never want to use inplace here, but could use floyd's alg
        // Lazy version: always use the cache alg.
        return sample_rejection(rng, length, amount);
    }
    let amount = amount as u32;
    let length = length as u32;

    // Choice of algorithm here depends on both length and amount. See:
    // https://github.com/rust-random/rand/pull/479
    // We do some calculations with f32. Accuracy is not very important.

    if amount < 163 {
        const C: [[f32; 2]; 2] = [[1.6, 8.0 / 45.0], [10.0, 70.0 / 9.0]];
        let j = if length < 500_000 { 0 } else { 1 };
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
        let j = if length < 500_000 { 0 } else { 1 };
        if (length as f32) < C[j] * (amount as f32) {
            sample_inplace(rng, length, amount)
        } else {
            sample_rejection(rng, length, amount)
        }
    }
}

/// Randomly sample exactly `amount` distinct indices from `0..length`, and
/// return them in an arbitrary order (there is no guarantee of shuffling or
/// ordering). The weights are to be provided by the input function `weights`,
/// which will be called once for each index.
///
/// This method is used internally by the slice sampling methods, but it can
/// sometimes be useful to have the indices themselves so this is provided as
/// an alternative.
///
/// This implementation uses `O(length + amount)` space and `O(length)` time
/// if the "nightly" feature is enabled, or `O(length)` space and
/// `O(length + amount * log length)` time otherwise.
///
/// Panics if `amount > length`.
#[cfg(feature = "std")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "std")))]
pub fn sample_weighted<R, F, X>(
    rng: &mut R, length: usize, weight: F, amount: usize,
) -> Result<IndexVec, WeightedError>
where
    R: Rng + ?Sized,
    F: Fn(usize) -> X,
    X: Into<f64>,
{
    if length > (core::u32::MAX as usize) {
        sample_efraimidis_spirakis(rng, length, weight, amount)
    } else {
        assert!(amount <= core::u32::MAX as usize);
        let amount = amount as u32;
        let length = length as u32;
        sample_efraimidis_spirakis(rng, length, weight, amount)
    }
}


/// Randomly sample exactly `amount` distinct indices from `0..length`, and
/// return them in an arbitrary order (there is no guarantee of shuffling or
/// ordering). The weights are to be provided by the input function `weights`,
/// which will be called once for each index.
///
/// This implementation uses the algorithm described by Efraimidis and Spirakis
/// in this paper: https://doi.org/10.1016/j.ipl.2005.11.003
/// It uses `O(length + amount)` space and `O(length)` time if the
/// "nightly" feature is enabled, or `O(length)` space and `O(length
/// + amount * log length)` time otherwise.
///
/// Panics if `amount > length`.
#[cfg(feature = "std")]
fn sample_efraimidis_spirakis<R, F, X, N>(
    rng: &mut R, length: N, weight: F, amount: N,
) -> Result<IndexVec, WeightedError>
where
    R: Rng + ?Sized,
    F: Fn(usize) -> X,
    X: Into<f64>,
    N: UInt,
    IndexVec: From<Vec<N>>,
{
    if amount == N::zero() {
        return Ok(IndexVec::U32(Vec::new()));
    }

    if amount > length {
        panic!("`amount` of samples must be less than or equal to `length`");
    }

    struct Element<N> {
        index: N,
        key: f64,
    }
    impl<N> PartialOrd for Element<N> {
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            self.key.partial_cmp(&other.key)
        }
    }
    impl<N> Ord for Element<N> {
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
             // partial_cmp will always produce a value,
             // because we check that the weights are not nan
            self.partial_cmp(other).unwrap()
        }
    }
    impl<N> PartialEq for Element<N> {
        fn eq(&self, other: &Self) -> bool {
            self.key == other.key
        }
    }
    impl<N> Eq for Element<N> {}

    #[cfg(feature = "nightly")]
    {
        let mut candidates = Vec::with_capacity(length.as_usize());
        let mut index = N::zero();
        while index < length {
            let weight = weight(index.as_usize()).into();
            if !(weight >= 0.) {
                return Err(WeightedError::InvalidWeight);
            }

            let key = rng.gen::<f64>().powf(1.0 / weight);
            candidates.push(Element { index, key });

            index += N::one();
        }

        // Partially sort the array to find the `amount` elements with the greatest
        // keys. Do this by using `select_nth_unstable` to put the elements with
        // the *smallest* keys at the beginning of the list in `O(n)` time, which
        // provides equivalent information about the elements with the *greatest* keys.
        let (_, mid, greater)
            = candidates.select_nth_unstable(length.as_usize() - amount.as_usize());

        let mut result: Vec<N> = Vec::with_capacity(amount.as_usize());
        result.push(mid.index);
        for element in greater {
            result.push(element.index);
        }
        Ok(IndexVec::from(result))
    }

    #[cfg(not(feature = "nightly"))]
    {
        use alloc::collections::BinaryHeap;

        // Partially sort the array such that the `amount` elements with the largest
        // keys are first using a binary max heap.
        let mut candidates = BinaryHeap::with_capacity(length.as_usize());
        let mut index = N::zero();
        while index < length {
            let weight = weight(index.as_usize()).into();
            if !(weight >= 0.) {
                return Err(WeightedError::InvalidWeight);
            }

            let key = rng.gen::<f64>().powf(1.0 / weight);
            candidates.push(Element { index, key });

            index += N::one();
        }

        let mut result: Vec<N> = Vec::with_capacity(amount.as_usize());
        while result.len() < amount.as_usize() {
            result.push(candidates.pop().unwrap().index);
        }
        Ok(IndexVec::from(result))
    }
}

/// Randomly sample exactly `amount` indices from `0..length`, using Floyd's
/// combination algorithm.
///
/// The output values are fully shuffled. (Overhead is under 50%.)
///
/// This implementation uses `O(amount)` memory and `O(amount^2)` time.
fn sample_floyd<R>(rng: &mut R, length: u32, amount: u32) -> IndexVec
where R: Rng + ?Sized {
    // For small amount we use Floyd's fully-shuffled variant. For larger
    // amounts this is slow due to Vec::insert performance, so we shuffle
    // afterwards. Benchmarks show little overhead from extra logic.
    let floyd_shuffle = amount < 50;

    debug_assert!(amount <= length);
    let mut indices = Vec::with_capacity(amount as usize);
    for j in length - amount..length {
        let t = rng.gen_range(0..=j);
        if floyd_shuffle {
            if let Some(pos) = indices.iter().position(|&x| x == t) {
                indices.insert(pos, j);
                continue;
            }
        } else if indices.contains(&t) {
            indices.push(j);
            continue;
        }
        indices.push(t);
    }
    if !floyd_shuffle {
        // Reimplement SliceRandom::shuffle with smaller indices
        for i in (1..amount).rev() {
            // invariant: elements with index > i have been locked in place.
            indices.swap(i as usize, rng.gen_range(0..=i) as usize);
        }
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
where R: Rng + ?Sized {
    debug_assert!(amount <= length);
    let mut indices: Vec<u32> = Vec::with_capacity(length as usize);
    indices.extend(0..length);
    for i in 0..amount {
        let j: u32 = rng.gen_range(i..length);
        indices.swap(i as usize, j as usize);
    }
    indices.truncate(amount as usize);
    debug_assert_eq!(indices.len(), amount as usize);
    IndexVec::from(indices)
}

trait UInt: Copy + PartialOrd + Ord + PartialEq + Eq + SampleUniform
    + core::hash::Hash + core::ops::AddAssign {
    fn zero() -> Self;
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
impl UInt for usize {
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
        self
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
    let distr = Uniform::new(X::zero(), length);
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

    #[test]
    #[cfg(feature = "serde1")]
    fn test_serialization_index_vec() {
        let some_index_vec = IndexVec::from(vec![254_usize, 234, 2, 1]);
        let de_some_index_vec: IndexVec = bincode::deserialize(&bincode::serialize(&some_index_vec).unwrap()).unwrap();
        match (some_index_vec, de_some_index_vec) {
            (IndexVec::U32(a), IndexVec::U32(b)) => {
                assert_eq!(a, b);
            },
            (IndexVec::USize(a), IndexVec::USize(b)) => {
                assert_eq!(a, b);
            },
            _ => {panic!("failed to seralize/deserialize `IndexVec`")}
        }
    }

    #[cfg(feature = "alloc")] use alloc::vec;

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
        for &(amount, len) in &[(0, 10), (5, 10), (10, 10)] {
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
                },
                IndexVec::USize(_) => panic!("expected `IndexVec::U32`"),
            }
        }
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

        do_test(10, 6, &[8, 0, 3, 5, 9, 6]); // floyd
        do_test(25, 10, &[18, 15, 14, 9, 0, 13, 5, 24]); // floyd
        do_test(300, 8, &[30, 283, 150, 1, 73, 13, 285, 35]); // floyd
        do_test(300, 80, &[31, 289, 248, 154, 5, 78, 19, 286]); // inplace
        do_test(300, 180, &[31, 289, 248, 154, 5, 78, 19, 286]); // inplace

        do_test(1_000_000, 8, &[
            103717, 963485, 826422, 509101, 736394, 807035, 5327, 632573,
        ]); // floyd
        do_test(1_000_000, 180, &[
            103718, 963490, 826426, 509103, 736396, 807036, 5327, 632573,
        ]); // rejection
    }
}
