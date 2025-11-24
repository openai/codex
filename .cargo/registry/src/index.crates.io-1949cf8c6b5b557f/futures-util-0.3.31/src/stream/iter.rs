use super::assert_stream;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};

/// Stream for the [`iter`] function.
#[derive(Debug, Clone)]
#[must_use = "streams do nothing unless polled"]
pub struct Iter<I> {
    iter: I,
}

impl<I> Iter<I> {
    /// Acquires a reference to the underlying iterator that this stream is pulling from.
    pub fn get_ref(&self) -> &I {
        &self.iter
    }

    /// Acquires a mutable reference to the underlying iterator that this stream is pulling from.
    pub fn get_mut(&mut self) -> &mut I {
        &mut self.iter
    }

    /// Consumes this stream, returning the underlying iterator.
    pub fn into_inner(self) -> I {
        self.iter
    }
}

impl<I> Unpin for Iter<I> {}

/// Converts an `Iterator` into a `Stream` which is always ready
/// to yield the next value.
///
/// Iterators in Rust don't express the ability to block, so this adapter
/// simply always calls `iter.next()` and returns that.
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
///
/// let stream = stream::iter(vec![17, 19]);
/// assert_eq!(vec![17, 19], stream.collect::<Vec<i32>>().await);
/// # });
/// ```
pub fn iter<I>(i: I) -> Iter<I::IntoIter>
where
    I: IntoIterator,
{
    assert_stream::<I::Item, _>(Iter { iter: i.into_iter() })
}

impl<I> Stream for Iter<I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<I::Item>> {
        Poll::Ready(self.iter.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
