use crate::adaptors::map::{MapSpecialCase, MapSpecialCaseFn};

macro_rules! impl_cons_iter(
    ($_A:ident, $_B:ident, ) => (); // stop

    ($A:ident, $($B:ident,)*) => (
        impl_cons_iter!($($B,)*);
        #[allow(non_snake_case)]
        impl<$($B),*, X> MapSpecialCaseFn<(($($B,)*), X)> for ConsTuplesFn {
            type Out = ($($B,)* X, );
            fn call(&mut self, (($($B,)*), X): (($($B,)*), X)) -> Self::Out {
                ($($B,)* X, )
            }
        }
    );
);

impl_cons_iter!(A, B, C, D, E, F, G, H, I, J, K, L,);

#[derive(Debug, Clone)]
pub struct ConsTuplesFn;

/// An iterator that maps an iterator of tuples like
/// `((A, B), C)` to an iterator of `(A, B, C)`.
///
/// Used by the `iproduct!()` macro.
pub type ConsTuples<I> = MapSpecialCase<I, ConsTuplesFn>;

/// Create an iterator that maps for example iterators of
/// `((A, B), C)` to `(A, B, C)`.
pub fn cons_tuples<I>(iterable: I) -> ConsTuples<I::IntoIter>
where
    I: IntoIterator,
{
    ConsTuples {
        iter: iterable.into_iter(),
        f: ConsTuplesFn,
    }
}
