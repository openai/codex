use super::size_hint;

/// An iterator which iterates two other iterators simultaneously
/// and panic if they have different lengths.
///
/// See [`.zip_eq()`](crate::Itertools::zip_eq) for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct ZipEq<I, J> {
    a: I,
    b: J,
}

/// Zips two iterators but **panics** if they are not of the same length.
///
/// [`IntoIterator`] enabled version of [`Itertools::zip_eq`](crate::Itertools::zip_eq).
///
/// ```
/// use itertools::zip_eq;
///
/// let data = [1, 2, 3, 4, 5];
/// for (a, b) in zip_eq(&data[..data.len() - 1], &data[1..]) {
///     /* loop body */
/// }
/// ```
pub fn zip_eq<I, J>(i: I, j: J) -> ZipEq<I::IntoIter, J::IntoIter>
where
    I: IntoIterator,
    J: IntoIterator,
{
    ZipEq {
        a: i.into_iter(),
        b: j.into_iter(),
    }
}

impl<I, J> Iterator for ZipEq<I, J>
where
    I: Iterator,
    J: Iterator,
{
    type Item = (I::Item, J::Item);

    fn next(&mut self) -> Option<Self::Item> {
        match (self.a.next(), self.b.next()) {
            (None, None) => None,
            (Some(a), Some(b)) => Some((a, b)),
            (None, Some(_)) | (Some(_), None) => {
                panic!("itertools: .zip_eq() reached end of one iterator before the other")
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        size_hint::min(self.a.size_hint(), self.b.size_hint())
    }
}

impl<I, J> ExactSizeIterator for ZipEq<I, J>
where
    I: ExactSizeIterator,
    J: ExactSizeIterator,
{
}
