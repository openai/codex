use std::fmt;
use std::iter::FusedIterator;
use std::usize;
use alloc::vec::Vec;

use super::combinations::{Combinations, combinations};
use super::size_hint;

/// An iterator to iterate through the powerset of the elements from an iterator.
///
/// See [`.powerset()`](crate::Itertools::powerset) for more
/// information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Powerset<I: Iterator> {
    combs: Combinations<I>,
    // Iterator `position` (equal to count of yielded elements).
    pos: usize,
}

impl<I> Clone for Powerset<I>
    where I: Clone + Iterator,
          I::Item: Clone,
{
    clone_fields!(combs, pos);
}

impl<I> fmt::Debug for Powerset<I>
    where I: Iterator + fmt::Debug,
          I::Item: fmt::Debug,
{
    debug_fmt_fields!(Powerset, combs, pos);
}

/// Create a new `Powerset` from a clonable iterator.
pub fn powerset<I>(src: I) -> Powerset<I>
    where I: Iterator,
          I::Item: Clone,
{
    Powerset {
        combs: combinations(src, 0),
        pos: 0,
    }
}

impl<I> Iterator for Powerset<I>
    where
        I: Iterator,
        I::Item: Clone,
{
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(elt) = self.combs.next() {
            self.pos = self.pos.saturating_add(1);
            Some(elt)
        } else if self.combs.k() < self.combs.n()
            || self.combs.k() == 0
        {
            self.combs.reset(self.combs.k() + 1);
            self.combs.next().map(|elt| {
                self.pos = self.pos.saturating_add(1);
                elt
            })
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Total bounds for source iterator.
        let src_total = size_hint::add_scalar(self.combs.src().size_hint(), self.combs.n());

        // Total bounds for self ( length(powerset(set) == 2 ^ length(set) )
        let self_total = size_hint::pow_scalar_base(2, src_total);

        if self.pos < usize::MAX {
            // Subtract count of elements already yielded from total.
            size_hint::sub_scalar(self_total, self.pos)
        } else {
            // Fallback: self.pos is saturated and no longer reliable.
            (0, self_total.1)
        }
    }
}

impl<I> FusedIterator for Powerset<I>
    where
        I: Iterator,
        I::Item: Clone,
{}
