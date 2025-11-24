use core::array;
use core::borrow::BorrowMut;
use std::fmt;
use std::iter::FusedIterator;

use super::lazy_buffer::LazyBuffer;
use alloc::vec::Vec;

use crate::adaptors::checked_binomial;

/// Iterator for `Vec` valued combinations returned by [`.combinations()`](crate::Itertools::combinations)
pub type Combinations<I> = CombinationsGeneric<I, Vec<usize>>;
/// Iterator for const generic combinations returned by [`.array_combinations()`](crate::Itertools::array_combinations)
pub type ArrayCombinations<I, const K: usize> = CombinationsGeneric<I, [usize; K]>;

/// Create a new `Combinations` from a clonable iterator.
pub fn combinations<I: Iterator>(iter: I, k: usize) -> Combinations<I>
where
    I::Item: Clone,
{
    Combinations::new(iter, (0..k).collect())
}

/// Create a new `ArrayCombinations` from a clonable iterator.
pub fn array_combinations<I: Iterator, const K: usize>(iter: I) -> ArrayCombinations<I, K>
where
    I::Item: Clone,
{
    ArrayCombinations::new(iter, array::from_fn(|i| i))
}

/// An iterator to iterate through all the `k`-length combinations in an iterator.
///
/// See [`.combinations()`](crate::Itertools::combinations) and [`.array_combinations()`](crate::Itertools::array_combinations) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct CombinationsGeneric<I: Iterator, Idx> {
    indices: Idx,
    pool: LazyBuffer<I>,
    first: bool,
}

/// A type holding indices of elements in a pool or buffer of items from an inner iterator
/// and used to pick out different combinations in a generic way.
pub trait PoolIndex<T>: BorrowMut<[usize]> {
    type Item;

    fn extract_item<I: Iterator<Item = T>>(&self, pool: &LazyBuffer<I>) -> Self::Item
    where
        T: Clone;

    fn len(&self) -> usize {
        self.borrow().len()
    }
}

impl<T> PoolIndex<T> for Vec<usize> {
    type Item = Vec<T>;

    fn extract_item<I: Iterator<Item = T>>(&self, pool: &LazyBuffer<I>) -> Vec<T>
    where
        T: Clone,
    {
        pool.get_at(self)
    }
}

impl<T, const K: usize> PoolIndex<T> for [usize; K] {
    type Item = [T; K];

    fn extract_item<I: Iterator<Item = T>>(&self, pool: &LazyBuffer<I>) -> [T; K]
    where
        T: Clone,
    {
        pool.get_array(*self)
    }
}

impl<I, Idx> Clone for CombinationsGeneric<I, Idx>
where
    I: Iterator + Clone,
    I::Item: Clone,
    Idx: Clone,
{
    clone_fields!(indices, pool, first);
}

impl<I, Idx> fmt::Debug for CombinationsGeneric<I, Idx>
where
    I: Iterator + fmt::Debug,
    I::Item: fmt::Debug,
    Idx: fmt::Debug,
{
    debug_fmt_fields!(Combinations, indices, pool, first);
}

impl<I: Iterator, Idx: PoolIndex<I::Item>> CombinationsGeneric<I, Idx> {
    /// Constructor with arguments the inner iterator and the initial state for the indices.
    fn new(iter: I, indices: Idx) -> Self {
        Self {
            indices,
            pool: LazyBuffer::new(iter),
            first: true,
        }
    }

    /// Returns the length of a combination produced by this iterator.
    #[inline]
    pub fn k(&self) -> usize {
        self.indices.len()
    }

    /// Returns the (current) length of the pool from which combination elements are
    /// selected. This value can change between invocations of [`next`](Combinations::next).
    #[inline]
    pub fn n(&self) -> usize {
        self.pool.len()
    }

    /// Returns a reference to the source pool.
    #[inline]
    pub(crate) fn src(&self) -> &LazyBuffer<I> {
        &self.pool
    }

    /// Return the length of the inner iterator and the count of remaining combinations.
    pub(crate) fn n_and_count(self) -> (usize, usize) {
        let Self {
            indices,
            pool,
            first,
        } = self;
        let n = pool.count();
        (n, remaining_for(n, first, indices.borrow()).unwrap())
    }

    /// Initialises the iterator by filling a buffer with elements from the
    /// iterator. Returns true if there are no combinations, false otherwise.
    fn init(&mut self) -> bool {
        self.pool.prefill(self.k());
        let done = self.k() > self.n();
        if !done {
            self.first = false;
        }

        done
    }

    /// Increments indices representing the combination to advance to the next
    /// (in lexicographic order by increasing sequence) combination. For example
    /// if we have n=4 & k=2 then `[0, 1] -> [0, 2] -> [0, 3] -> [1, 2] -> ...`
    ///
    /// Returns true if we've run out of combinations, false otherwise.
    fn increment_indices(&mut self) -> bool {
        // Borrow once instead of noise each time it's indexed
        let indices = self.indices.borrow_mut();

        if indices.is_empty() {
            return true; // Done
        }
        // Scan from the end, looking for an index to increment
        let mut i: usize = indices.len() - 1;

        // Check if we need to consume more from the iterator
        if indices[i] == self.pool.len() - 1 {
            self.pool.get_next(); // may change pool size
        }

        while indices[i] == i + self.pool.len() - indices.len() {
            if i > 0 {
                i -= 1;
            } else {
                // Reached the last combination
                return true;
            }
        }

        // Increment index, and reset the ones to its right
        indices[i] += 1;
        for j in i + 1..indices.len() {
            indices[j] = indices[j - 1] + 1;
        }
        // If we've made it this far, we haven't run out of combos
        false
    }

    /// Returns the n-th item or the number of successful steps.
    pub(crate) fn try_nth(&mut self, n: usize) -> Result<<Self as Iterator>::Item, usize>
    where
        I: Iterator,
        I::Item: Clone,
    {
        let done = if self.first {
            self.init()
        } else {
            self.increment_indices()
        };
        if done {
            return Err(0);
        }
        for i in 0..n {
            if self.increment_indices() {
                return Err(i + 1);
            }
        }
        Ok(self.indices.extract_item(&self.pool))
    }
}

impl<I, Idx> Iterator for CombinationsGeneric<I, Idx>
where
    I: Iterator,
    I::Item: Clone,
    Idx: PoolIndex<I::Item>,
{
    type Item = Idx::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let done = if self.first {
            self.init()
        } else {
            self.increment_indices()
        };

        if done {
            return None;
        }

        Some(self.indices.extract_item(&self.pool))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.try_nth(n).ok()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (mut low, mut upp) = self.pool.size_hint();
        low = remaining_for(low, self.first, self.indices.borrow()).unwrap_or(usize::MAX);
        upp = upp.and_then(|upp| remaining_for(upp, self.first, self.indices.borrow()));
        (low, upp)
    }

    #[inline]
    fn count(self) -> usize {
        self.n_and_count().1
    }
}

impl<I, Idx> FusedIterator for CombinationsGeneric<I, Idx>
where
    I: Iterator,
    I::Item: Clone,
    Idx: PoolIndex<I::Item>,
{
}

impl<I: Iterator> Combinations<I> {
    /// Resets this `Combinations` back to an initial state for combinations of length
    /// `k` over the same pool data source. If `k` is larger than the current length
    /// of the data pool an attempt is made to prefill the pool so that it holds `k`
    /// elements.
    pub(crate) fn reset(&mut self, k: usize) {
        self.first = true;

        if k < self.indices.len() {
            self.indices.truncate(k);
            for i in 0..k {
                self.indices[i] = i;
            }
        } else {
            for i in 0..self.indices.len() {
                self.indices[i] = i;
            }
            self.indices.extend(self.indices.len()..k);
            self.pool.prefill(k);
        }
    }
}

/// For a given size `n`, return the count of remaining combinations or None if it would overflow.
fn remaining_for(n: usize, first: bool, indices: &[usize]) -> Option<usize> {
    let k = indices.len();
    if n < k {
        Some(0)
    } else if first {
        checked_binomial(n, k)
    } else {
        // https://en.wikipedia.org/wiki/Combinatorial_number_system
        // http://www.site.uottawa.ca/~lucia/courses/5165-09/GenCombObj.pdf

        // The combinations generated after the current one can be counted by counting as follows:
        // - The subsequent combinations that differ in indices[0]:
        //   If subsequent combinations differ in indices[0], then their value for indices[0]
        //   must be at least 1 greater than the current indices[0].
        //   As indices is strictly monotonically sorted, this means we can effectively choose k values
        //   from (n - 1 - indices[0]), leading to binomial(n - 1 - indices[0], k) possibilities.
        // - The subsequent combinations with same indices[0], but differing indices[1]:
        //   Here we can choose k - 1 values from (n - 1 - indices[1]) values,
        //   leading to binomial(n - 1 - indices[1], k - 1) possibilities.
        // - (...)
        // - The subsequent combinations with same indices[0..=i], but differing indices[i]:
        //   Here we can choose k - i values from (n - 1 - indices[i]) values: binomial(n - 1 - indices[i], k - i).
        //   Since subsequent combinations can in any index, we must sum up the aforementioned binomial coefficients.

        // Below, `n0` resembles indices[i].
        indices.iter().enumerate().try_fold(0usize, |sum, (i, n0)| {
            sum.checked_add(checked_binomial(n - 1 - *n0, k - i)?)
        })
    }
}
