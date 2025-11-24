use std::iter::FromIterator;
use std::marker::PhantomData;

#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct MapSpecialCase<I, F> {
    iter: I,
    f: F,
}

pub trait MapSpecialCaseFn<T> {
    type Out;
    fn call(&mut self, t: T) -> Self::Out;
}

impl<I, R> Iterator for MapSpecialCase<I, R>
where
    I: Iterator,
    R: MapSpecialCaseFn<I::Item>,
{
    type Item = R::Out;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|i| self.f.call(i))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn fold<Acc, Fold>(self, init: Acc, mut fold_f: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.iter.fold(init, move |acc, v| fold_f(acc, f.call(v)))
    }

    fn collect<C>(self) -> C
    where
        C: FromIterator<Self::Item>,
    {
        let mut f = self.f;
        self.iter.map(move |v| f.call(v)).collect()
    }
}

impl<I, R> DoubleEndedIterator for MapSpecialCase<I, R>
where
    I: DoubleEndedIterator,
    R: MapSpecialCaseFn<I::Item>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|i| self.f.call(i))
    }
}

impl<I, R> ExactSizeIterator for MapSpecialCase<I, R>
where
    I: ExactSizeIterator,
    R: MapSpecialCaseFn<I::Item>,
{
}

/// An iterator adapter to apply a transformation within a nested `Result::Ok`.
///
/// See [`.map_ok()`](crate::Itertools::map_ok) for more information.
pub type MapOk<I, F> = MapSpecialCase<I, MapSpecialCaseFnOk<F>>;

/// See [`MapOk`].
#[deprecated(note = "Use MapOk instead", since = "0.10.0")]
pub type MapResults<I, F> = MapOk<I, F>;

impl<F, T, U, E> MapSpecialCaseFn<Result<T, E>> for MapSpecialCaseFnOk<F>
where
    F: FnMut(T) -> U,
{
    type Out = Result<U, E>;
    fn call(&mut self, t: Result<T, E>) -> Self::Out {
        t.map(|v| self.0(v))
    }
}

#[derive(Clone)]
pub struct MapSpecialCaseFnOk<F>(F);

impl<F> std::fmt::Debug for MapSpecialCaseFnOk<F> {
    debug_fmt_fields!(MapSpecialCaseFnOk,);
}

/// Create a new `MapOk` iterator.
pub fn map_ok<I, F, T, U, E>(iter: I, f: F) -> MapOk<I, F>
where
    I: Iterator<Item = Result<T, E>>,
    F: FnMut(T) -> U,
{
    MapSpecialCase {
        iter,
        f: MapSpecialCaseFnOk(f),
    }
}

/// An iterator adapter to apply `Into` conversion to each element.
///
/// See [`.map_into()`](crate::Itertools::map_into) for more information.
pub type MapInto<I, R> = MapSpecialCase<I, MapSpecialCaseFnInto<R>>;

impl<T: Into<U>, U> MapSpecialCaseFn<T> for MapSpecialCaseFnInto<U> {
    type Out = U;
    fn call(&mut self, t: T) -> Self::Out {
        t.into()
    }
}

#[derive(Clone, Debug)]
pub struct MapSpecialCaseFnInto<U>(PhantomData<U>);

/// Create a new [`MapInto`] iterator.
pub fn map_into<I, R>(iter: I) -> MapInto<I, R> {
    MapSpecialCase {
        iter,
        f: MapSpecialCaseFnInto(PhantomData),
    }
}
