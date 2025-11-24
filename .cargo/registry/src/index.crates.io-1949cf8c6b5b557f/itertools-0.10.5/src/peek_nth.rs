use crate::size_hint;
use crate::PeekingNext;
use alloc::collections::VecDeque;
use std::iter::Fuse;

/// See [`peek_nth()`] for more information.
#[derive(Clone, Debug)]
pub struct PeekNth<I>
where
    I: Iterator,
{
    iter: Fuse<I>,
    buf: VecDeque<I::Item>,
}

/// A drop-in replacement for [`std::iter::Peekable`] which adds a `peek_nth`
/// method allowing the user to `peek` at a value several iterations forward
/// without advancing the base iterator.
///
/// This differs from `multipeek` in that subsequent calls to `peek` or
/// `peek_nth` will always return the same value until `next` is called
/// (making `reset_peek` unnecessary).
pub fn peek_nth<I>(iterable: I) -> PeekNth<I::IntoIter>
where
    I: IntoIterator,
{
    PeekNth {
        iter: iterable.into_iter().fuse(),
        buf: VecDeque::new(),
    }
}

impl<I> PeekNth<I>
where
    I: Iterator,
{
    /// Works exactly like the `peek` method in `std::iter::Peekable`
    pub fn peek(&mut self) -> Option<&I::Item> {
        self.peek_nth(0)
    }

    /// Returns a reference to the `nth` value without advancing the iterator.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust
    /// use itertools::peek_nth;
    ///
    /// let xs = vec![1,2,3];
    /// let mut iter = peek_nth(xs.iter());
    ///
    /// assert_eq!(iter.peek_nth(0), Some(&&1));
    /// assert_eq!(iter.next(), Some(&1));
    ///
    /// // The iterator does not advance even if we call `peek_nth` multiple times
    /// assert_eq!(iter.peek_nth(0), Some(&&2));
    /// assert_eq!(iter.peek_nth(1), Some(&&3));
    /// assert_eq!(iter.next(), Some(&2));
    ///
    /// // Calling `peek_nth` past the end of the iterator will return `None`
    /// assert_eq!(iter.peek_nth(1), None);
    /// ```
    pub fn peek_nth(&mut self, n: usize) -> Option<&I::Item> {
        let unbuffered_items = (n + 1).saturating_sub(self.buf.len());

        self.buf.extend(self.iter.by_ref().take(unbuffered_items));

        self.buf.get(n)
    }
}

impl<I> Iterator for PeekNth<I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.buf.pop_front().or_else(|| self.iter.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        size_hint::add_scalar(self.iter.size_hint(), self.buf.len())
    }
}

impl<I> ExactSizeIterator for PeekNth<I> where I: ExactSizeIterator {}

impl<I> PeekingNext for PeekNth<I>
where
    I: Iterator,
{
    fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
    where
        F: FnOnce(&Self::Item) -> bool,
    {
        self.peek().filter(|item| accept(item))?;
        self.next()
    }
}
