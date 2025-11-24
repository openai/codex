use super::size_hint;

/// See [`multizip`] for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Zip<T> {
    t: T,
}

/// An iterator that generalizes *.zip()* and allows running multiple iterators in lockstep.
///
/// The iterator `Zip<(I, J, ..., M)>` is formed from a tuple of iterators (or values that
/// implement [`IntoIterator`]) and yields elements
/// until any of the subiterators yields `None`.
///
/// The iterator element type is a tuple like like `(A, B, ..., E)` where `A` to `E` are the
/// element types of the subiterator.
///
/// **Note:** The result of this macro is a value of a named type (`Zip<(I, J,
/// ..)>` of each component iterator `I, J, ...`) if each component iterator is
/// nameable.
///
/// Prefer [`izip!()`] over `multizip` for the performance benefits of using the
/// standard library `.zip()`. Prefer `multizip` if a nameable type is needed.
///
/// ```
/// use itertools::multizip;
///
/// // iterate over three sequences side-by-side
/// let mut results = [0, 0, 0, 0];
/// let inputs = [3, 7, 9, 6];
///
/// for (r, index, input) in multizip((&mut results, 0..10, &inputs)) {
///     *r = index * 10 + input;
/// }
///
/// assert_eq!(results, [0 + 3, 10 + 7, 29, 36]);
/// ```
/// [`izip!()`]: crate::izip
pub fn multizip<T, U>(t: U) -> Zip<T>
    where Zip<T>: From<U>,
          Zip<T>: Iterator,
{
    Zip::from(t)
}

macro_rules! impl_zip_iter {
    ($($B:ident),*) => (
        #[allow(non_snake_case)]
        impl<$($B: IntoIterator),*> From<($($B,)*)> for Zip<($($B::IntoIter,)*)> {
            fn from(t: ($($B,)*)) -> Self {
                let ($($B,)*) = t;
                Zip { t: ($($B.into_iter(),)*) }
            }
        }

        #[allow(non_snake_case)]
        #[allow(unused_assignments)]
        impl<$($B),*> Iterator for Zip<($($B,)*)>
            where
            $(
                $B: Iterator,
            )*
        {
            type Item = ($($B::Item,)*);

            fn next(&mut self) -> Option<Self::Item>
            {
                let ($(ref mut $B,)*) = self.t;

                // NOTE: Just like iter::Zip, we check the iterators
                // for None in order. We may finish unevenly (some
                // iterators gave n + 1 elements, some only n).
                $(
                    let $B = match $B.next() {
                        None => return None,
                        Some(elt) => elt
                    };
                )*
                Some(($($B,)*))
            }

            fn size_hint(&self) -> (usize, Option<usize>)
            {
                let sh = (::std::usize::MAX, None);
                let ($(ref $B,)*) = self.t;
                $(
                    let sh = size_hint::min($B.size_hint(), sh);
                )*
                sh
            }
        }

        #[allow(non_snake_case)]
        impl<$($B),*> ExactSizeIterator for Zip<($($B,)*)> where
            $(
                $B: ExactSizeIterator,
            )*
        { }

        #[allow(non_snake_case)]
        impl<$($B),*> DoubleEndedIterator for Zip<($($B,)*)> where
            $(
                $B: DoubleEndedIterator + ExactSizeIterator,
            )*
        {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                let ($(ref mut $B,)*) = self.t;
                let size = *[$( $B.len(), )*].iter().min().unwrap();

                $(
                    if $B.len() != size {
                        for _ in 0..$B.len() - size { $B.next_back(); }
                    }
                )*

                match ($($B.next_back(),)*) {
                    ($(Some($B),)*) => Some(($($B,)*)),
                    _ => None,
                }
            }
        }
    );
}

impl_zip_iter!(A);
impl_zip_iter!(A, B);
impl_zip_iter!(A, B, C);
impl_zip_iter!(A, B, C, D);
impl_zip_iter!(A, B, C, D, E);
impl_zip_iter!(A, B, C, D, E, F);
impl_zip_iter!(A, B, C, D, E, F, G);
impl_zip_iter!(A, B, C, D, E, F, G, H);
impl_zip_iter!(A, B, C, D, E, F, G, H, I);
impl_zip_iter!(A, B, C, D, E, F, G, H, I, J);
impl_zip_iter!(A, B, C, D, E, F, G, H, I, J, K);
impl_zip_iter!(A, B, C, D, E, F, G, H, I, J, K, L);
