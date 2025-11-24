/// Converts an iterator of tuples into a tuple of containers.
///
/// `multiunzip()` consumes an entire iterator of n-ary tuples, producing `n` collections, one for each
/// column.
///
/// This function is, in some sense, the opposite of [`multizip`].
///
/// ```
/// use itertools::multiunzip;
///
/// let inputs = vec![(1, 2, 3), (4, 5, 6), (7, 8, 9)];
///
/// let (a, b, c): (Vec<_>, Vec<_>, Vec<_>) = multiunzip(inputs);
///
/// assert_eq!(a, vec![1, 4, 7]);
/// assert_eq!(b, vec![2, 5, 8]);
/// assert_eq!(c, vec![3, 6, 9]);
/// ```
///
/// [`multizip`]: crate::multizip
pub fn multiunzip<FromI, I>(i: I) -> FromI
where
    I: IntoIterator,
    I::IntoIter: MultiUnzip<FromI>,
{
    i.into_iter().multiunzip()
}

/// An iterator that can be unzipped into multiple collections.
///
/// See [`.multiunzip()`](crate::Itertools::multiunzip) for more information.
pub trait MultiUnzip<FromI>: Iterator {
    /// Unzip this iterator into multiple collections.
    fn multiunzip(self) -> FromI;
}

macro_rules! impl_unzip_iter {
    ($($T:ident => $FromT:ident),*) => (
        #[allow(non_snake_case)]
        impl<IT: Iterator<Item = ($($T,)*)>, $($T, $FromT: Default + Extend<$T>),* > MultiUnzip<($($FromT,)*)> for IT {
            fn multiunzip(self) -> ($($FromT,)*) {
                // This implementation mirrors the logic of Iterator::unzip resp. Extend for (A, B) as close as possible.
                // Unfortunately a lot of the used api there is still unstable (https://github.com/rust-lang/rust/issues/72631).
                //
                // Iterator::unzip: https://doc.rust-lang.org/src/core/iter/traits/iterator.rs.html#2825-2865
                // Extend for (A, B): https://doc.rust-lang.org/src/core/iter/traits/collect.rs.html#370-411

                let mut res = ($($FromT::default(),)*);
                let ($($FromT,)*) = &mut res;

                // Still unstable #72631
                // let (lower_bound, _) = self.size_hint();
                // if lower_bound > 0 {
                //     $($FromT.extend_reserve(lower_bound);)*
                // }

                self.fold((), |(), ($($T,)*)| {
                    // Still unstable #72631
                    // $( $FromT.extend_one($T); )*
                    $( $FromT.extend(std::iter::once($T)); )*
                });
                res
            }
        }
    );
}

impl_unzip_iter!();
impl_unzip_iter!(A => FromA);
impl_unzip_iter!(A => FromA, B => FromB);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD, E => FromE);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD, E => FromE, F => FromF);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD, E => FromE, F => FromF, G => FromG);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD, E => FromE, F => FromF, G => FromG, H => FromH);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD, E => FromE, F => FromF, G => FromG, H => FromH, I => FromI);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD, E => FromE, F => FromF, G => FromG, H => FromH, I => FromI, J => FromJ);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD, E => FromE, F => FromF, G => FromG, H => FromH, I => FromI, J => FromJ, K => FromK);
impl_unzip_iter!(A => FromA, B => FromB, C => FromC, D => FromD, E => FromE, F => FromF, G => FromG, H => FromH, I => FromI, J => FromJ, K => FromK, L => FromL);
