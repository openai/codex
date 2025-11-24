#![cfg(feature = "use_alloc")]

use crate::size_hint;
use crate::Itertools;

use alloc::vec::Vec;

#[derive(Clone)]
/// An iterator adaptor that iterates over the cartesian product of
/// multiple iterators of type `I`.
///
/// An iterator element type is `Vec<I>`.
///
/// See [`.multi_cartesian_product()`](crate::Itertools::multi_cartesian_product)
/// for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct MultiProduct<I>(Vec<MultiProductIter<I>>)
    where I: Iterator + Clone,
          I::Item: Clone;

impl<I> std::fmt::Debug for MultiProduct<I>
where
    I: Iterator + Clone + std::fmt::Debug,
    I::Item: Clone + std::fmt::Debug,
{
    debug_fmt_fields!(CoalesceBy, 0);
}

/// Create a new cartesian product iterator over an arbitrary number
/// of iterators of the same type.
///
/// Iterator element is of type `Vec<H::Item::Item>`.
pub fn multi_cartesian_product<H>(iters: H) -> MultiProduct<<H::Item as IntoIterator>::IntoIter>
    where H: Iterator,
          H::Item: IntoIterator,
          <H::Item as IntoIterator>::IntoIter: Clone,
          <H::Item as IntoIterator>::Item: Clone
{
    MultiProduct(iters.map(|i| MultiProductIter::new(i.into_iter())).collect())
}

#[derive(Clone, Debug)]
/// Holds the state of a single iterator within a `MultiProduct`.
struct MultiProductIter<I>
    where I: Iterator + Clone,
          I::Item: Clone
{
    cur: Option<I::Item>,
    iter: I,
    iter_orig: I,
}

/// Holds the current state during an iteration of a `MultiProduct`.
#[derive(Debug)]
enum MultiProductIterState {
    StartOfIter,
    MidIter { on_first_iter: bool },
}

impl<I> MultiProduct<I>
    where I: Iterator + Clone,
          I::Item: Clone
{
    /// Iterates the rightmost iterator, then recursively iterates iterators
    /// to the left if necessary.
    ///
    /// Returns true if the iteration succeeded, else false.
    fn iterate_last(
        multi_iters: &mut [MultiProductIter<I>],
        mut state: MultiProductIterState
    ) -> bool {
        use self::MultiProductIterState::*;

        if let Some((last, rest)) = multi_iters.split_last_mut() {
            let on_first_iter = match state {
                StartOfIter => {
                    let on_first_iter = !last.in_progress();
                    state = MidIter { on_first_iter };
                    on_first_iter
                },
                MidIter { on_first_iter } => on_first_iter
            };

            if !on_first_iter {
                last.iterate();
            }

            if last.in_progress() {
                true
            } else if MultiProduct::iterate_last(rest, state) {
                last.reset();
                last.iterate();
                // If iterator is None twice consecutively, then iterator is
                // empty; whole product is empty.
                last.in_progress()
            } else {
                false
            }
        } else {
            // Reached end of iterator list. On initialisation, return true.
            // At end of iteration (final iterator finishes), finish.
            match state {
                StartOfIter => false,
                MidIter { on_first_iter } => on_first_iter
            }
        }
    }

    /// Returns the unwrapped value of the next iteration.
    fn curr_iterator(&self) -> Vec<I::Item> {
        self.0.iter().map(|multi_iter| {
            multi_iter.cur.clone().unwrap()
        }).collect()
    }

    /// Returns true if iteration has started and has not yet finished; false
    /// otherwise.
    fn in_progress(&self) -> bool {
        if let Some(last) = self.0.last() {
            last.in_progress()
        } else {
            false
        }
    }
}

impl<I> MultiProductIter<I>
    where I: Iterator + Clone,
          I::Item: Clone
{
    fn new(iter: I) -> Self {
        MultiProductIter {
            cur: None,
            iter: iter.clone(),
            iter_orig: iter
        }
    }

    /// Iterate the managed iterator.
    fn iterate(&mut self) {
        self.cur = self.iter.next();
    }

    /// Reset the managed iterator.
    fn reset(&mut self) {
        self.iter = self.iter_orig.clone();
    }

    /// Returns true if the current iterator has been started and has not yet
    /// finished; false otherwise.
    fn in_progress(&self) -> bool {
        self.cur.is_some()
    }
}

impl<I> Iterator for MultiProduct<I>
    where I: Iterator + Clone,
          I::Item: Clone
{
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        if MultiProduct::iterate_last(
            &mut self.0,
            MultiProductIterState::StartOfIter
        ) {
            Some(self.curr_iterator())
        } else {
            None
        }
    }

    fn count(self) -> usize {
        if self.0.is_empty() {
            return 0;
        }

        if !self.in_progress() {
            return self.0.into_iter().fold(1, |acc, multi_iter| {
                acc * multi_iter.iter.count()
            });
        }

        self.0.into_iter().fold(
            0,
            |acc, MultiProductIter { iter, iter_orig, cur: _ }| {
                let total_count = iter_orig.count();
                let cur_count = iter.count();
                acc * total_count + cur_count
            }
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Not ExactSizeIterator because size may be larger than usize
        if self.0.is_empty() {
            return (0, Some(0));
        }

        if !self.in_progress() {
            return self.0.iter().fold((1, Some(1)), |acc, multi_iter| {
                size_hint::mul(acc, multi_iter.iter.size_hint())
            });
        }

        self.0.iter().fold(
            (0, Some(0)),
            |acc, &MultiProductIter { ref iter, ref iter_orig, cur: _ }| {
                let cur_size = iter.size_hint();
                let total_size = iter_orig.size_hint();
                size_hint::add(size_hint::mul(acc, total_size), cur_size)
            }
        )
    }

    fn last(self) -> Option<Self::Item> {
        let iter_count = self.0.len();

        let lasts: Self::Item = self.0.into_iter()
            .map(|multi_iter| multi_iter.iter.last())
            .while_some()
            .collect();

        if lasts.len() == iter_count {
            Some(lasts)
        } else {
            None
        }
    }
}
