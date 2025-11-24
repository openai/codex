
macro_rules! impl_cons_iter(
    ($_A:ident, $_B:ident, ) => (); // stop

    ($A:ident, $($B:ident,)*) => (
        impl_cons_iter!($($B,)*);
        #[allow(non_snake_case)]
        impl<X, Iter, $($B),*> Iterator for ConsTuples<Iter, (($($B,)*), X)>
            where Iter: Iterator<Item = (($($B,)*), X)>,
        {
            type Item = ($($B,)* X, );
            fn next(&mut self) -> Option<Self::Item> {
                self.iter.next().map(|(($($B,)*), x)| ($($B,)* x, ))
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.iter.size_hint()
            }
            fn fold<Acc, Fold>(self, accum: Acc, mut f: Fold) -> Acc
                where Fold: FnMut(Acc, Self::Item) -> Acc,
            {
                self.iter.fold(accum, move |acc, (($($B,)*), x)| f(acc, ($($B,)* x, )))
            }
        }

        #[allow(non_snake_case)]
        impl<X, Iter, $($B),*> DoubleEndedIterator for ConsTuples<Iter, (($($B,)*), X)>
            where Iter: DoubleEndedIterator<Item = (($($B,)*), X)>,
        {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.iter.next().map(|(($($B,)*), x)| ($($B,)* x, ))
            }
        }

    );
);

impl_cons_iter!(A, B, C, D, E, F, G, H, I, J, K, L,);

/// An iterator that maps an iterator of tuples like
/// `((A, B), C)` to an iterator of `(A, B, C)`.
///
/// Used by the `iproduct!()` macro.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct ConsTuples<I, J>
    where I: Iterator<Item=J>,
{
    iter: I,
}

impl<I, J> Clone for ConsTuples<I, J>
    where I: Clone + Iterator<Item=J>,
{
    clone_fields!(iter);
}

/// Create an iterator that maps for example iterators of
/// `((A, B), C)` to `(A, B, C)`.
pub fn cons_tuples<I, J>(iterable: I) -> ConsTuples<I::IntoIter, J>
    where I: IntoIterator<Item=J>
{
    ConsTuples { iter: iterable.into_iter() }
}
