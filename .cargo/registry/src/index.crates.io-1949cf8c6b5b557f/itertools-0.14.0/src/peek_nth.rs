use crate::size_hint;
use crate::PeekingNext;
use alloc::collections::VecDeque;
use std::iter::Fuse;

/// See [`peek_nth()`] for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
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
    /// Works exactly like the `peek` method in [`std::iter::Peekable`].
    pub fn peek(&mut self) -> Option<&I::Item> {
        self.peek_nth(0)
    }

    /// Works exactly like the `peek_mut` method in [`std::iter::Peekable`].
    pub fn peek_mut(&mut self) -> Option<&mut I::Item> {
        self.peek_nth_mut(0)
    }

    /// Returns a reference to the `nth` value without advancing the iterator.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use itertools::peek_nth;
    ///
    /// let xs = vec![1, 2, 3];
    /// let mut iter = peek_nth(xs.into_iter());
    ///
    /// assert_eq!(iter.peek_nth(0), Some(&1));
    /// assert_eq!(iter.next(), Some(1));
    ///
    /// // The iterator does not advance even if we call `peek_nth` multiple times
    /// assert_eq!(iter.peek_nth(0), Some(&2));
    /// assert_eq!(iter.peek_nth(1), Some(&3));
    /// assert_eq!(iter.next(), Some(2));
    ///
    /// // Calling `peek_nth` past the end of the iterator will return `None`
    /// assert_eq!(iter.peek_nth(1), None);
    /// ```
    pub fn peek_nth(&mut self, n: usize) -> Option<&I::Item> {
        let unbuffered_items = (n + 1).saturating_sub(self.buf.len());

        self.buf.extend(self.iter.by_ref().take(unbuffered_items));

        self.buf.get(n)
    }

    /// Returns a mutable reference to the `nth` value without advancing the iterator.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use itertools::peek_nth;
    ///
    /// let xs = vec![1, 2, 3, 4, 5];
    /// let mut iter = peek_nth(xs.into_iter());
    ///
    /// assert_eq!(iter.peek_nth_mut(0), Some(&mut 1));
    /// assert_eq!(iter.next(), Some(1));
    ///
    /// // The iterator does not advance even if we call `peek_nth_mut` multiple times
    /// assert_eq!(iter.peek_nth_mut(0), Some(&mut 2));
    /// assert_eq!(iter.peek_nth_mut(1), Some(&mut 3));
    /// assert_eq!(iter.next(), Some(2));
    ///
    /// // Peek into the iterator and set the value behind the mutable reference.
    /// if let Some(p) = iter.peek_nth_mut(1) {
    ///     assert_eq!(*p, 4);
    ///     *p = 9;
    /// }
    ///
    /// // The value we put in reappears as the iterator continues.
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(9));
    ///
    /// // Calling `peek_nth_mut` past the end of the iterator will return `None`
    /// assert_eq!(iter.peek_nth_mut(1), None);
    /// ```
    pub fn peek_nth_mut(&mut self, n: usize) -> Option<&mut I::Item> {
        let unbuffered_items = (n + 1).saturating_sub(self.buf.len());

        self.buf.extend(self.iter.by_ref().take(unbuffered_items));

        self.buf.get_mut(n)
    }

    /// Works exactly like the `next_if` method in [`std::iter::Peekable`].
    pub fn next_if(&mut self, func: impl FnOnce(&I::Item) -> bool) -> Option<I::Item> {
        match self.next() {
            Some(item) if func(&item) => Some(item),
            Some(item) => {
                self.buf.push_front(item);
                None
            }
            _ => None,
        }
    }

    /// Works exactly like the `next_if_eq` method in [`std::iter::Peekable`].
    pub fn next_if_eq<T>(&mut self, expected: &T) -> Option<I::Item>
    where
        T: ?Sized,
        I::Item: PartialEq<T>,
    {
        self.next_if(|next| next == expected)
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

    fn fold<B, F>(self, mut init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        init = self.buf.into_iter().fold(init, &mut f);
        self.iter.fold(init, f)
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
