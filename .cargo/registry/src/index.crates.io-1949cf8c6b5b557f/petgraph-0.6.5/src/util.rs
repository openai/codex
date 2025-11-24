use std::iter;

pub fn enumerate<I>(iterable: I) -> iter::Enumerate<I::IntoIter>
where
    I: IntoIterator,
{
    iterable.into_iter().enumerate()
}

pub fn zip<I, J>(i: I, j: J) -> iter::Zip<I::IntoIter, J::IntoIter>
where
    I: IntoIterator,
    J: IntoIterator,
{
    i.into_iter().zip(j)
}
