use super::size_hint;
use std::cmp::Ordering::{Equal, Greater, Less};
use std::iter::{Fuse, FusedIterator};

use crate::either_or_both::EitherOrBoth;

// ZipLongest originally written by SimonSapin,
// and dedicated to itertools https://github.com/rust-lang/rust/pull/19283

/// An iterator which iterates two other iterators simultaneously
/// and wraps the elements in [`EitherOrBoth`].
///
/// This iterator is *fused*.
///
/// See [`.zip_longest()`](crate::Itertools::zip_longest) for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct ZipLongest<T, U> {
    a: Fuse<T>,
    b: Fuse<U>,
}

/// Create a new `ZipLongest` iterator.
pub fn zip_longest<T, U>(a: T, b: U) -> ZipLongest<T, U>
where
    T: Iterator,
    U: Iterator,
{
    ZipLongest {
        a: a.fuse(),
        b: b.fuse(),
    }
}

impl<T, U> Iterator for ZipLongest<T, U>
where
    T: Iterator,
    U: Iterator,
{
    type Item = EitherOrBoth<T::Item, U::Item>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match (self.a.next(), self.b.next()) {
            (None, None) => None,
            (Some(a), None) => Some(EitherOrBoth::Left(a)),
            (None, Some(b)) => Some(EitherOrBoth::Right(b)),
            (Some(a), Some(b)) => Some(EitherOrBoth::Both(a, b)),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        size_hint::max(self.a.size_hint(), self.b.size_hint())
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let Self { mut a, mut b } = self;
        let res = a.try_fold(init, |init, a| match b.next() {
            Some(b) => Ok(f(init, EitherOrBoth::Both(a, b))),
            None => Err(f(init, EitherOrBoth::Left(a))),
        });
        match res {
            Ok(acc) => b.map(EitherOrBoth::Right).fold(acc, f),
            Err(acc) => a.map(EitherOrBoth::Left).fold(acc, f),
        }
    }
}

impl<T, U> DoubleEndedIterator for ZipLongest<T, U>
where
    T: DoubleEndedIterator + ExactSizeIterator,
    U: DoubleEndedIterator + ExactSizeIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.a.len().cmp(&self.b.len()) {
            Equal => match (self.a.next_back(), self.b.next_back()) {
                (None, None) => None,
                (Some(a), Some(b)) => Some(EitherOrBoth::Both(a, b)),
                // These can only happen if .len() is inconsistent with .next_back()
                (Some(a), None) => Some(EitherOrBoth::Left(a)),
                (None, Some(b)) => Some(EitherOrBoth::Right(b)),
            },
            Greater => self.a.next_back().map(EitherOrBoth::Left),
            Less => self.b.next_back().map(EitherOrBoth::Right),
        }
    }

    fn rfold<B, F>(self, mut init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let Self { mut a, mut b } = self;
        let a_len = a.len();
        let b_len = b.len();
        match a_len.cmp(&b_len) {
            Equal => {}
            Greater => {
                init = a
                    .by_ref()
                    .rev()
                    .take(a_len - b_len)
                    .map(EitherOrBoth::Left)
                    .fold(init, &mut f)
            }
            Less => {
                init = b
                    .by_ref()
                    .rev()
                    .take(b_len - a_len)
                    .map(EitherOrBoth::Right)
                    .fold(init, &mut f)
            }
        }
        a.rfold(init, |acc, item_a| {
            f(acc, EitherOrBoth::Both(item_a, b.next_back().unwrap()))
        })
    }
}

impl<T, U> ExactSizeIterator for ZipLongest<T, U>
where
    T: ExactSizeIterator,
    U: ExactSizeIterator,
{
}

impl<T, U> FusedIterator for ZipLongest<T, U>
where
    T: Iterator,
    U: Iterator,
{
}
