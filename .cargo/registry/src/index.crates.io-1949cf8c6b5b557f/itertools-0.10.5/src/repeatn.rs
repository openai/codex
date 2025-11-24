use std::iter::FusedIterator;

/// An iterator that produces *n* repetitions of an element.
///
/// See [`repeat_n()`](crate::repeat_n) for more information.
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Clone, Debug)]
pub struct RepeatN<A> {
    elt: Option<A>,
    n: usize,
}

/// Create an iterator that produces `n` repetitions of `element`.
pub fn repeat_n<A>(element: A, n: usize) -> RepeatN<A>
    where A: Clone,
{
    if n == 0 {
        RepeatN { elt: None, n, }
    } else {
        RepeatN { elt: Some(element), n, }
    }
}

impl<A> Iterator for RepeatN<A>
    where A: Clone
{
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        if self.n > 1 {
            self.n -= 1;
            self.elt.as_ref().cloned()
        } else {
            self.n = 0;
            self.elt.take()
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.n, Some(self.n))
    }
}

impl<A> DoubleEndedIterator for RepeatN<A>
    where A: Clone
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.next()
    }
}

impl<A> ExactSizeIterator for RepeatN<A>
    where A: Clone
{}

impl<A> FusedIterator for RepeatN<A>
    where A: Clone
{}
