use super::size_hint;
use std::iter::{Fuse, FusedIterator};

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
where
    I: Iterator,
{
    intersperse_with(iter, IntersperseElementSimple(elt))
}

impl<Item, F: FnMut() -> Item> IntersperseElement<Item> for F {
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
where
    I: Iterator,
{
    element: ElemF,
    iter: Fuse<I>,
    /// `peek` is None while no item have been taken out of `iter` (at definition).
    /// Then `peek` will alternatively be `Some(None)` and `Some(Some(item))`,
    /// where `None` indicates it's time to generate from `element` (unless `iter` is empty).
    peek: Option<Option<I::Item>>,
}

/// Create a new `IntersperseWith` iterator
pub fn intersperse_with<I, ElemF>(iter: I, elt: ElemF) -> IntersperseWith<I, ElemF>
where
    I: Iterator,
{
    IntersperseWith {
        peek: None,
        iter: iter.fuse(),
        element: elt,
    }
}

impl<I, ElemF> Iterator for IntersperseWith<I, ElemF>
where
    I: Iterator,
    ElemF: IntersperseElement<I::Item>,
{
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            element,
            iter,
            peek,
        } = self;
        match peek {
            Some(item @ Some(_)) => item.take(),
            Some(None) => match iter.next() {
                new @ Some(_) => {
                    *peek = Some(new);
                    Some(element.generate())
                }
                None => None,
            },
            None => {
                *peek = Some(None);
                iter.next()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let mut sh = self.iter.size_hint();
        sh = size_hint::add(sh, sh);
        match self.peek {
            Some(Some(_)) => size_hint::add_scalar(sh, 1),
            Some(None) => sh,
            None => size_hint::sub_scalar(sh, 1),
        }
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let Self {
            mut element,
            mut iter,
            peek,
        } = self;
        let mut accum = init;

        if let Some(x) = peek.unwrap_or_else(|| iter.next()) {
            accum = f(accum, x);
        }

        iter.fold(accum, |accum, x| {
            let accum = f(accum, element.generate());
            f(accum, x)
        })
    }
}

impl<I, ElemF> FusedIterator for IntersperseWith<I, ElemF>
where
    I: Iterator,
    ElemF: IntersperseElement<I::Item>,
{
}
