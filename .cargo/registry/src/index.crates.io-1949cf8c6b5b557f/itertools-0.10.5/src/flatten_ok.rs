use crate::size_hint;
use std::{
    fmt,
    iter::{DoubleEndedIterator, FusedIterator},
};

pub fn flatten_ok<I, T, E>(iter: I) -> FlattenOk<I, T, E>
where
    I: Iterator<Item = Result<T, E>>,
    T: IntoIterator,
{
    FlattenOk {
        iter,
        inner_front: None,
        inner_back: None,
    }
}

/// An iterator adaptor that flattens `Result::Ok` values and
/// allows `Result::Err` values through unchanged.
///
/// See [`.flatten_ok()`](crate::Itertools::flatten_ok) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct FlattenOk<I, T, E>
where
    I: Iterator<Item = Result<T, E>>,
    T: IntoIterator,
{
    iter: I,
    inner_front: Option<T::IntoIter>,
    inner_back: Option<T::IntoIter>,
}

impl<I, T, E> Iterator for FlattenOk<I, T, E>
where
    I: Iterator<Item = Result<T, E>>,
    T: IntoIterator,
{
    type Item = Result<T::Item, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Handle the front inner iterator.
            if let Some(inner) = &mut self.inner_front {
                if let Some(item) = inner.next() {
                    return Some(Ok(item));
                }

                // This is necessary for the iterator to implement `FusedIterator`
                // with only the original iterator being fused.
                self.inner_front = None;
            }

            match self.iter.next() {
                Some(Ok(ok)) => self.inner_front = Some(ok.into_iter()),
                Some(Err(e)) => return Some(Err(e)),
                None => {
                    // Handle the back inner iterator.
                    if let Some(inner) = &mut self.inner_back {
                        if let Some(item) = inner.next() {
                            return Some(Ok(item));
                        }

                        // This is necessary for the iterator to implement `FusedIterator`
                        // with only the original iterator being fused.
                        self.inner_back = None;
                    } else {
                        return None;
                    }
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let inner_hint = |inner: &Option<T::IntoIter>| {
            inner
                .as_ref()
                .map(Iterator::size_hint)
                .unwrap_or((0, Some(0)))
        };
        let inner_front = inner_hint(&self.inner_front);
        let inner_back = inner_hint(&self.inner_back);
        // The outer iterator `Ok` case could be (0, None) as we don't know its size_hint yet.
        let outer = match self.iter.size_hint() {
            (0, Some(0)) => (0, Some(0)),
            _ => (0, None),
        };

        size_hint::add(size_hint::add(inner_front, inner_back), outer)
    }
}

impl<I, T, E> DoubleEndedIterator for FlattenOk<I, T, E>
where
    I: DoubleEndedIterator<Item = Result<T, E>>,
    T: IntoIterator,
    T::IntoIter: DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            // Handle the back inner iterator.
            if let Some(inner) = &mut self.inner_back {
                if let Some(item) = inner.next_back() {
                    return Some(Ok(item));
                }

                // This is necessary for the iterator to implement `FusedIterator`
                // with only the original iterator being fused.
                self.inner_back = None;
            }

            match self.iter.next_back() {
                Some(Ok(ok)) => self.inner_back = Some(ok.into_iter()),
                Some(Err(e)) => return Some(Err(e)),
                None => {
                    // Handle the front inner iterator.
                    if let Some(inner) = &mut self.inner_front {
                        if let Some(item) = inner.next_back() {
                            return Some(Ok(item));
                        }

                        // This is necessary for the iterator to implement `FusedIterator`
                        // with only the original iterator being fused.
                        self.inner_front = None;
                    } else {
                        return None;
                    }
                }
            }
        }
    }
}

impl<I, T, E> Clone for FlattenOk<I, T, E>
where
    I: Iterator<Item = Result<T, E>> + Clone,
    T: IntoIterator,
    T::IntoIter: Clone,
{
    clone_fields!(iter, inner_front, inner_back);
}

impl<I, T, E> fmt::Debug for FlattenOk<I, T, E>
where
    I: Iterator<Item = Result<T, E>> + fmt::Debug,
    T: IntoIterator,
    T::IntoIter: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlattenOk")
            .field("iter", &self.iter)
            .field("inner_front", &self.inner_front)
            .field("inner_back", &self.inner_back)
            .finish()
    }
}

/// Only the iterator being flattened needs to implement [`FusedIterator`].
impl<I, T, E> FusedIterator for FlattenOk<I, T, E>
where
    I: FusedIterator<Item = Result<T, E>>,
    T: IntoIterator,
{
}
