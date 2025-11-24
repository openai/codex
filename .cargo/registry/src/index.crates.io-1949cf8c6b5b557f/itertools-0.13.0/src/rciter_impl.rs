use alloc::rc::Rc;
use std::cell::RefCell;
use std::iter::{FusedIterator, IntoIterator};

/// A wrapper for `Rc<RefCell<I>>`, that implements the `Iterator` trait.
#[derive(Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct RcIter<I> {
    /// The boxed iterator.
    pub rciter: Rc<RefCell<I>>,
}

/// Return an iterator inside a `Rc<RefCell<_>>` wrapper.
///
/// The returned `RcIter` can be cloned, and each clone will refer back to the
/// same original iterator.
///
/// `RcIter` allows doing interesting things like using `.zip()` on an iterator with
/// itself, at the cost of runtime borrow checking which may have a performance
/// penalty.
///
/// Iterator element type is `Self::Item`.
///
/// ```
/// use itertools::rciter;
/// use itertools::zip;
///
/// // In this example a range iterator is created and we iterate it using
/// // three separate handles (two of them given to zip).
/// // We also use the IntoIterator implementation for `&RcIter`.
///
/// let mut iter = rciter(0..9);
/// let mut z = zip(&iter, &iter);
///
/// assert_eq!(z.next(), Some((0, 1)));
/// assert_eq!(z.next(), Some((2, 3)));
/// assert_eq!(z.next(), Some((4, 5)));
/// assert_eq!(iter.next(), Some(6));
/// assert_eq!(z.next(), Some((7, 8)));
/// assert_eq!(z.next(), None);
/// ```
///
/// **Panics** in iterator methods if a borrow error is encountered in the
/// iterator methods. It can only happen if the `RcIter` is reentered in
/// `.next()`, i.e. if it somehow participates in an “iterator knot”
/// where it is an adaptor of itself.
pub fn rciter<I>(iterable: I) -> RcIter<I::IntoIter>
where
    I: IntoIterator,
{
    RcIter {
        rciter: Rc::new(RefCell::new(iterable.into_iter())),
    }
}

impl<I> Clone for RcIter<I> {
    clone_fields!(rciter);
}

impl<A, I> Iterator for RcIter<I>
where
    I: Iterator<Item = A>,
{
    type Item = A;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.rciter.borrow_mut().next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // To work sanely with other API that assume they own an iterator,
        // so it can't change in other places, we can't guarantee as much
        // in our size_hint. Other clones may drain values under our feet.
        (0, self.rciter.borrow().size_hint().1)
    }
}

impl<I> DoubleEndedIterator for RcIter<I>
where
    I: DoubleEndedIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.rciter.borrow_mut().next_back()
    }
}

/// Return an iterator from `&RcIter<I>` (by simply cloning it).
impl<'a, I> IntoIterator for &'a RcIter<I>
where
    I: Iterator,
{
    type Item = I::Item;
    type IntoIter = RcIter<I>;

    fn into_iter(self) -> RcIter<I> {
        self.clone()
    }
}

impl<A, I> FusedIterator for RcIter<I> where I: FusedIterator<Item = A> {}
