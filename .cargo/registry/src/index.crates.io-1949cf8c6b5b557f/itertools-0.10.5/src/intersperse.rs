use std::iter::{Fuse, FusedIterator};
use super::size_hint;

pub trait IntersperseElement<Item> {
    fn generate(&mut self) -> Item;
}

#[derive(Debug, Clone)]
pub struct IntersperseElementSimple<Item>(Item);

impl<Item: Clone> IntersperseElement<Item> for IntersperseElementSimple<Item> {
    fn generate(&mut self) -> Item {
        self.0.clone()
    }
}

/// An iterator adaptor to insert a particular value
/// between each element of the adapted iterator.
///
/// Iterator element type is `I::Item`
///
/// This iterator is *fused*.
///
/// See [`.intersperse()`](crate::Itertools::intersperse) for more information.
pub type Intersperse<I> = IntersperseWith<I, IntersperseElementSimple<<I as Iterator>::Item>>;

/// Create a new Intersperse iterator
pub fn intersperse<I>(iter: I, elt: I::Item) -> Intersperse<I>
    where I: Iterator,
{
    intersperse_with(iter, IntersperseElementSimple(elt))
}

impl<Item, F: FnMut()->Item> IntersperseElement<Item> for F {
    fn generate(&mut self) -> Item {
        self()
    }
}

/// An iterator adaptor to insert a particular value created by a function
/// between each element of the adapted iterator.
///
/// Iterator element type is `I::Item`
///
/// This iterator is *fused*.
///
/// See [`.intersperse_with()`](crate::Itertools::intersperse_with) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone, Debug)]
pub struct IntersperseWith<I, ElemF>
    where I: Iterator,
{
    element: ElemF,
    iter: Fuse<I>,
    peek: Option<I::Item>,
}

/// Create a new `IntersperseWith` iterator
pub fn intersperse_with<I, ElemF>(iter: I, elt: ElemF) -> IntersperseWith<I, ElemF>
    where I: Iterator,
{
    let mut iter = iter.fuse();
    IntersperseWith {
        peek: iter.next(),
        iter,
        element: elt,
    }
}

impl<I, ElemF> Iterator for IntersperseWith<I, ElemF>
    where I: Iterator,
          ElemF: IntersperseElement<I::Item>
{
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.peek.is_some() {
            self.peek.take()
        } else {
            self.peek = self.iter.next();
            if self.peek.is_some() {
                Some(self.element.generate())
            } else {
                None
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // 2 * SH + { 1 or 0 }
        let has_peek = self.peek.is_some() as usize;
        let sh = self.iter.size_hint();
        size_hint::add_scalar(size_hint::add(sh, sh), has_peek)
    }

    fn fold<B, F>(mut self, init: B, mut f: F) -> B where
        Self: Sized, F: FnMut(B, Self::Item) -> B,
    {
        let mut accum = init;

        if let Some(x) = self.peek.take() {
            accum = f(accum, x);
        }

        let element = &mut self.element;

        self.iter.fold(accum,
            |accum, x| {
                let accum = f(accum, element.generate());
                f(accum, x)
        })
    }
}

impl<I, ElemF> FusedIterator for IntersperseWith<I, ElemF>
    where I: Iterator,
          ElemF: IntersperseElement<I::Item>
{}
