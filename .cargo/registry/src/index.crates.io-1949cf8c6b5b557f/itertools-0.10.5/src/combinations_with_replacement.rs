use alloc::vec::Vec;
use std::fmt;
use std::iter::FusedIterator;

use super::lazy_buffer::LazyBuffer;

/// An iterator to iterate through all the `n`-length combinations in an iterator, with replacement.
///
/// See [`.combinations_with_replacement()`](crate::Itertools::combinations_with_replacement)
/// for more information.
#[derive(Clone)]
pub struct CombinationsWithReplacement<I>
where
    I: Iterator,
    I::Item: Clone,
{
    indices: Vec<usize>,
    pool: LazyBuffer<I>,
    first: bool,
}

impl<I> fmt::Debug for CombinationsWithReplacement<I>
where
    I: Iterator + fmt::Debug,
    I::Item: fmt::Debug + Clone,
{
    debug_fmt_fields!(Combinations, indices, pool, first);
}

impl<I> CombinationsWithReplacement<I>
where
    I: Iterator,
    I::Item: Clone,
{
    /// Map the current mask over the pool to get an output combination
    fn current(&self) -> Vec<I::Item> {
        self.indices.iter().map(|i| self.pool[*i].clone()).collect()
    }
}

/// Create a new `CombinationsWithReplacement` from a clonable iterator.
pub fn combinations_with_replacement<I>(iter: I, k: usize) -> CombinationsWithReplacement<I>
where
    I: Iterator,
    I::Item: Clone,
{
    let indices: Vec<usize> = alloc::vec![0; k];
    let pool: LazyBuffer<I> = LazyBuffer::new(iter);

    CombinationsWithReplacement {
        indices,
        pool,
        first: true,
    }
}

impl<I> Iterator for CombinationsWithReplacement<I>
where
    I: Iterator,
    I::Item: Clone,
{
    type Item = Vec<I::Item>;
    fn next(&mut self) -> Option<Self::Item> {
        // If this is the first iteration, return early
        if self.first {
            // In empty edge cases, stop iterating immediately
            return if !(self.indices.is_empty() || self.pool.get_next()) {
                None
            // Otherwise, yield the initial state
            } else {
                self.first = false;
                Some(self.current())
            };
        }

        // Check if we need to consume more from the iterator
        // This will run while we increment our first index digit
        self.pool.get_next();

        // Work out where we need to update our indices
        let mut increment: Option<(usize, usize)> = None;
        for (i, indices_int) in self.indices.iter().enumerate().rev() {
            if *indices_int < self.pool.len()-1 {
                increment = Some((i, indices_int + 1));
                break;
            }
        }

        match increment {
            // If we can update the indices further
            Some((increment_from, increment_value)) => {
                // We need to update the rightmost non-max value
                // and all those to the right
                for indices_index in increment_from..self.indices.len() {
                    self.indices[indices_index] = increment_value;
                }
                Some(self.current())
            }
            // Otherwise, we're done
            None => None,
        }
    }
}

impl<I> FusedIterator for CombinationsWithReplacement<I>
where
    I: Iterator,
    I::Item: Clone,
{}
