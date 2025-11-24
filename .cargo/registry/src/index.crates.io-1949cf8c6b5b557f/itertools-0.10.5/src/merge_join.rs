use std::cmp::Ordering;
use std::iter::Fuse;
use std::fmt;

use super::adaptors::{PutBack, put_back};
use crate::either_or_both::EitherOrBoth;
#[cfg(doc)]
use crate::Itertools;

/// Return an iterator adaptor that merge-joins items from the two base iterators in ascending order.
///
/// [`IntoIterator`] enabled version of [`Itertools::merge_join_by`].
pub fn merge_join_by<I, J, F>(left: I, right: J, cmp_fn: F)
    -> MergeJoinBy<I::IntoIter, J::IntoIter, F>
    where I: IntoIterator,
          J: IntoIterator,
          F: FnMut(&I::Item, &J::Item) -> Ordering
{
    MergeJoinBy {
        left: put_back(left.into_iter().fuse()),
        right: put_back(right.into_iter().fuse()),
        cmp_fn,
    }
}

/// An iterator adaptor that merge-joins items from the two base iterators in ascending order.
///
/// See [`.merge_join_by()`](crate::Itertools::merge_join_by) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct MergeJoinBy<I: Iterator, J: Iterator, F> {
    left: PutBack<Fuse<I>>,
    right: PutBack<Fuse<J>>,
    cmp_fn: F
}

impl<I, J, F> Clone for MergeJoinBy<I, J, F>
    where I: Iterator,
          J: Iterator,
          PutBack<Fuse<I>>: Clone,
          PutBack<Fuse<J>>: Clone,
          F: Clone,
{
    clone_fields!(left, right, cmp_fn);
}

impl<I, J, F> fmt::Debug for MergeJoinBy<I, J, F>
    where I: Iterator + fmt::Debug,
          I::Item: fmt::Debug,
          J: Iterator + fmt::Debug,
          J::Item: fmt::Debug,
{
    debug_fmt_fields!(MergeJoinBy, left, right);
}

impl<I, J, F> Iterator for MergeJoinBy<I, J, F>
    where I: Iterator,
          J: Iterator,
          F: FnMut(&I::Item, &J::Item) -> Ordering
{
    type Item = EitherOrBoth<I::Item, J::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.left.next(), self.right.next()) {
            (None, None) => None,
            (Some(left), None) =>
                Some(EitherOrBoth::Left(left)),
            (None, Some(right)) =>
                Some(EitherOrBoth::Right(right)),
            (Some(left), Some(right)) => {
                match (self.cmp_fn)(&left, &right) {
                    Ordering::Equal =>
                        Some(EitherOrBoth::Both(left, right)),
                    Ordering::Less => {
                        self.right.put_back(right);
                        Some(EitherOrBoth::Left(left))
                    },
                    Ordering::Greater => {
                        self.left.put_back(left);
                        Some(EitherOrBoth::Right(right))
                    }
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (a_lower, a_upper) = self.left.size_hint();
        let (b_lower, b_upper) = self.right.size_hint();

        let lower = ::std::cmp::max(a_lower, b_lower);

        let upper = match (a_upper, b_upper) {
            (Some(x), Some(y)) => x.checked_add(y),
            _ => None,
        };

        (lower, upper)
    }

    fn count(mut self) -> usize {
        let mut count = 0;
        loop {
            match (self.left.next(), self.right.next()) {
                (None, None) => break count,
                (Some(_left), None) => break count + 1 + self.left.into_parts().1.count(),
                (None, Some(_right)) => break count + 1 + self.right.into_parts().1.count(),
                (Some(left), Some(right)) => {
                    count += 1;
                    match (self.cmp_fn)(&left, &right) {
                        Ordering::Equal => {}
                        Ordering::Less => self.right.put_back(right),
                        Ordering::Greater => self.left.put_back(left),
                    }
                }
            }
        }
    }

    fn last(mut self) -> Option<Self::Item> {
        let mut previous_element = None;
        loop {
            match (self.left.next(), self.right.next()) {
                (None, None) => break previous_element,
                (Some(left), None) => {
                    break Some(EitherOrBoth::Left(
                        self.left.into_parts().1.last().unwrap_or(left),
                    ))
                }
                (None, Some(right)) => {
                    break Some(EitherOrBoth::Right(
                        self.right.into_parts().1.last().unwrap_or(right),
                    ))
                }
                (Some(left), Some(right)) => {
                    previous_element = match (self.cmp_fn)(&left, &right) {
                        Ordering::Equal => Some(EitherOrBoth::Both(left, right)),
                        Ordering::Less => {
                            self.right.put_back(right);
                            Some(EitherOrBoth::Left(left))
                        }
                        Ordering::Greater => {
                            self.left.put_back(left);
                            Some(EitherOrBoth::Right(right))
                        }
                    }
                }
            }
        }
    }

    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        loop {
            if n == 0 {
                break self.next();
            }
            n -= 1;
            match (self.left.next(), self.right.next()) {
                (None, None) => break None,
                (Some(_left), None) => break self.left.nth(n).map(EitherOrBoth::Left),
                (None, Some(_right)) => break self.right.nth(n).map(EitherOrBoth::Right),
                (Some(left), Some(right)) => match (self.cmp_fn)(&left, &right) {
                    Ordering::Equal => {}
                    Ordering::Less => self.right.put_back(right),
                    Ordering::Greater => self.left.put_back(left),
                },
            }
        }
    }
}
