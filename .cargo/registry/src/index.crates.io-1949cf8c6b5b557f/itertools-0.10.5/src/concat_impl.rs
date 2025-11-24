use crate::Itertools;

/// Combine all an iterator's elements into one element by using [`Extend`].
///
/// [`IntoIterator`]-enabled version of [`Itertools::concat`].
///
/// This combinator will extend the first item with each of the rest of the
/// items of the iterator. If the iterator is empty, the default value of
/// `I::Item` is returned.
///
/// ```rust
/// use itertools::concat;
/// 
/// let input = vec![vec![1], vec![2, 3], vec![4, 5, 6]];
/// assert_eq!(concat(input), vec![1, 2, 3, 4, 5, 6]);
/// ```
pub fn concat<I>(iterable: I) -> I::Item
    where I: IntoIterator,
          I::Item: Extend<<<I as IntoIterator>::Item as IntoIterator>::Item> + IntoIterator + Default
{
    #[allow(deprecated)] //TODO: once msrv hits 1.51. replace `fold1` with `reduce`
    iterable.into_iter().fold1(|mut a, b| { a.extend(b); a }).unwrap_or_default()
}
