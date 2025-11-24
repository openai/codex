use alloc::vec::Vec;

use crate::size_hint;

/// An iterator adaptor that allows putting multiple
/// items in front of the iterator.
///
/// Iterator element type is `I::Item`.
#[derive(Debug, Clone)]
pub struct PutBackN<I: Iterator> {
    top: Vec<I::Item>,
    iter: I,
}

/// Create an iterator where you can put back multiple values to the front
/// of the iteration.
///
/// Iterator element type is `I::Item`.
pub fn put_back_n<I>(iterable: I) -> PutBackN<I::IntoIter>
    where I: IntoIterator
{
    PutBackN {
        top: Vec::new(),
        iter: iterable.into_iter(),
    }
}

impl<I: Iterator> PutBackN<I> {
    /// Puts x in front of the iterator.
    /// The values are yielded in order of the most recently put back
    /// values first.
    ///
    /// ```rust
    /// use itertools::put_back_n;
    ///
    /// let mut it = put_back_n(1..5);
    /// it.next();
    /// it.put_back(1);
    /// it.put_back(0);
    ///
    /// assert!(itertools::equal(it, 0..5));
    /// ```
    #[inline]
    pub fn put_back(&mut self, x: I::Item) {
        self.top.push(x);
    }
}

impl<I: Iterator> Iterator for PutBackN<I> {
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.top.pop().or_else(|| self.iter.next())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        size_hint::add_scalar(self.iter.size_hint(), self.top.len())
    }
}

