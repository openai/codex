use super::{for_both, Either, Left, Right};
use core::iter;

macro_rules! wrap_either {
    ($value:expr => $( $tail:tt )*) => {
        match $value {
            Left(inner) => inner.map(Left) $($tail)*,
            Right(inner) => inner.map(Right) $($tail)*,
        }
    };
}

/// Iterator that maps left or right iterators to corresponding `Either`-wrapped items.
///
/// This struct is created by the [`Either::factor_into_iter`],
/// [`factor_iter`][Either::factor_iter],
/// and [`factor_iter_mut`][Either::factor_iter_mut] methods.
#[derive(Clone, Debug)]
pub struct IterEither<L, R> {
    inner: Either<L, R>,
}

impl<L, R> IterEither<L, R> {
    pub(crate) fn new(inner: Either<L, R>) -> Self {
        IterEither { inner }
    }
}

impl<L, R, A> Extend<A> for Either<L, R>
where
    L: Extend<A>,
    R: Extend<A>,
{
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = A>,
    {
        for_both!(self, inner => inner.extend(iter))
    }
}

/// `Either<L, R>` is an iterator if both `L` and `R` are iterators.
impl<L, R> Iterator for Either<L, R>
where
    L: Iterator,
    R: Iterator<Item = L::Item>,
{
    type Item = L::Item;

    fn next(&mut self) -> Option<Self::Item> {
        for_both!(self, inner => inner.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        for_both!(self, inner => inner.size_hint())
    }

    fn fold<Acc, G>(self, init: Acc, f: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        for_both!(self, inner => inner.fold(init, f))
    }

    fn for_each<F>(self, f: F)
    where
        F: FnMut(Self::Item),
    {
        for_both!(self, inner => inner.for_each(f))
    }

    fn count(self) -> usize {
        for_both!(self, inner => inner.count())
    }

    fn last(self) -> Option<Self::Item> {
        for_both!(self, inner => inner.last())
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        for_both!(self, inner => inner.nth(n))
    }

    fn collect<B>(self) -> B
    where
        B: iter::FromIterator<Self::Item>,
    {
        for_both!(self, inner => inner.collect())
    }

    fn partition<B, F>(self, f: F) -> (B, B)
    where
        B: Default + Extend<Self::Item>,
        F: FnMut(&Self::Item) -> bool,
    {
        for_both!(self, inner => inner.partition(f))
    }

    fn all<F>(&mut self, f: F) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        for_both!(self, inner => inner.all(f))
    }

    fn any<F>(&mut self, f: F) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        for_both!(self, inner => inner.any(f))
    }

    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        for_both!(self, inner => inner.find(predicate))
    }

    fn find_map<B, F>(&mut self, f: F) -> Option<B>
    where
        F: FnMut(Self::Item) -> Option<B>,
    {
        for_both!(self, inner => inner.find_map(f))
    }

    fn position<P>(&mut self, predicate: P) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
    {
        for_both!(self, inner => inner.position(predicate))
    }
}

impl<L, R> DoubleEndedIterator for Either<L, R>
where
    L: DoubleEndedIterator,
    R: DoubleEndedIterator<Item = L::Item>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        for_both!(self, inner => inner.next_back())
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        for_both!(self, inner => inner.nth_back(n))
    }

    fn rfold<Acc, G>(self, init: Acc, f: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        for_both!(self, inner => inner.rfold(init, f))
    }

    fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        for_both!(self, inner => inner.rfind(predicate))
    }
}

impl<L, R> ExactSizeIterator for Either<L, R>
where
    L: ExactSizeIterator,
    R: ExactSizeIterator<Item = L::Item>,
{
    fn len(&self) -> usize {
        for_both!(self, inner => inner.len())
    }
}

impl<L, R> iter::FusedIterator for Either<L, R>
where
    L: iter::FusedIterator,
    R: iter::FusedIterator<Item = L::Item>,
{
}

impl<L, R> Iterator for IterEither<L, R>
where
    L: Iterator,
    R: Iterator,
{
    type Item = Either<L::Item, R::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(map_either!(self.inner, ref mut inner => inner.next()?))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        for_both!(self.inner, ref inner => inner.size_hint())
    }

    fn fold<Acc, G>(self, init: Acc, f: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        wrap_either!(self.inner => .fold(init, f))
    }

    fn for_each<F>(self, f: F)
    where
        F: FnMut(Self::Item),
    {
        wrap_either!(self.inner => .for_each(f))
    }

    fn count(self) -> usize {
        for_both!(self.inner, inner => inner.count())
    }

    fn last(self) -> Option<Self::Item> {
        Some(map_either!(self.inner, inner => inner.last()?))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        Some(map_either!(self.inner, ref mut inner => inner.nth(n)?))
    }

    fn collect<B>(self) -> B
    where
        B: iter::FromIterator<Self::Item>,
    {
        wrap_either!(self.inner => .collect())
    }

    fn partition<B, F>(self, f: F) -> (B, B)
    where
        B: Default + Extend<Self::Item>,
        F: FnMut(&Self::Item) -> bool,
    {
        wrap_either!(self.inner => .partition(f))
    }

    fn all<F>(&mut self, f: F) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        wrap_either!(&mut self.inner => .all(f))
    }

    fn any<F>(&mut self, f: F) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        wrap_either!(&mut self.inner => .any(f))
    }

    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        wrap_either!(&mut self.inner => .find(predicate))
    }

    fn find_map<B, F>(&mut self, f: F) -> Option<B>
    where
        F: FnMut(Self::Item) -> Option<B>,
    {
        wrap_either!(&mut self.inner => .find_map(f))
    }

    fn position<P>(&mut self, predicate: P) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
    {
        wrap_either!(&mut self.inner => .position(predicate))
    }
}

impl<L, R> DoubleEndedIterator for IterEither<L, R>
where
    L: DoubleEndedIterator,
    R: DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        Some(map_either!(self.inner, ref mut inner => inner.next_back()?))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        Some(map_either!(self.inner, ref mut inner => inner.nth_back(n)?))
    }

    fn rfold<Acc, G>(self, init: Acc, f: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        wrap_either!(self.inner => .rfold(init, f))
    }

    fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        wrap_either!(&mut self.inner => .rfind(predicate))
    }
}

impl<L, R> ExactSizeIterator for IterEither<L, R>
where
    L: ExactSizeIterator,
    R: ExactSizeIterator,
{
    fn len(&self) -> usize {
        for_both!(self.inner, ref inner => inner.len())
    }
}

impl<L, R> iter::FusedIterator for IterEither<L, R>
where
    L: iter::FusedIterator,
    R: iter::FusedIterator,
{
}
