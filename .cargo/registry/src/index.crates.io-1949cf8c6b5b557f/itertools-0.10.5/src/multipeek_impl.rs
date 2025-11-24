use std::iter::Fuse;
use alloc::collections::VecDeque;
use crate::size_hint;
use crate::PeekingNext;
#[cfg(doc)]
use crate::Itertools;

/// See [`multipeek()`] for more information.
#[derive(Clone, Debug)]
pub struct MultiPeek<I>
    where I: Iterator
{
    iter: Fuse<I>,
    buf: VecDeque<I::Item>,
    index: usize,
}

/// An iterator adaptor that allows the user to peek at multiple `.next()`
/// values without advancing the base iterator.
///
/// [`IntoIterator`] enabled version of [`Itertools::multipeek`].
pub fn multipeek<I>(iterable: I) -> MultiPeek<I::IntoIter>
    where I: IntoIterator
{
    MultiPeek {
        iter: iterable.into_iter().fuse(),
        buf: VecDeque::new(),
        index: 0,
    }
}

impl<I> MultiPeek<I>
    where I: Iterator
{
    /// Reset the peeking “cursor”
    pub fn reset_peek(&mut self) {
        self.index = 0;
    }
}

impl<I: Iterator> MultiPeek<I> {
    /// Works exactly like `.next()` with the only difference that it doesn't
    /// advance itself. `.peek()` can be called multiple times, to peek
    /// further ahead.
    /// When `.next()` is called, reset the peeking “cursor”.
    pub fn peek(&mut self) -> Option<&I::Item> {
        let ret = if self.index < self.buf.len() {
            Some(&self.buf[self.index])
        } else {
            match self.iter.next() {
                Some(x) => {
                    self.buf.push_back(x);
                    Some(&self.buf[self.index])
                }
                None => return None,
            }
        };

        self.index += 1;
        ret
    }
}

impl<I> PeekingNext for MultiPeek<I>
    where I: Iterator,
{
    fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
        where F: FnOnce(&Self::Item) -> bool
    {
        if self.buf.is_empty() {
            if let Some(r) = self.peek() {
                if !accept(r) { return None }
            }
        } else if let Some(r) = self.buf.get(0) {
            if !accept(r) { return None }
        }
        self.next()
    }
}

impl<I> Iterator for MultiPeek<I>
    where I: Iterator
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.index = 0;
        self.buf.pop_front().or_else(|| self.iter.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        size_hint::add_scalar(self.iter.size_hint(), self.buf.len())
    }
}

// Same size
impl<I> ExactSizeIterator for MultiPeek<I>
    where I: ExactSizeIterator
{}


